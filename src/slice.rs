use std::ops::RangeBounds;
use std::sync::Arc;

use crate::iter::{Bytes, Chars, Chunks, Lines};
use crate::rope::Rope;
use crate::str_utils::{
    byte_to_char_idx, byte_to_line_idx, byte_to_utf16_surrogate_idx, char_to_byte_idx,
    char_to_line_idx, count_chars, count_line_breaks, count_utf16_surrogates, line_to_byte_idx,
    line_to_char_idx, utf16_code_unit_to_char_idx,
};
use crate::tree::{Count, Node, TextInfo};
use crate::{end_bound_to_num, start_bound_to_num, Error, Result};

/// An immutable view into part of a `Rope`.
///
/// Just like standard `&str` slices, `RopeSlice`s behave as if the text in
/// their range is the only text that exists.  All indexing is relative to
/// the start of their range, and all iterators and methods that return text
/// truncate that text to the range of the slice.
///
/// In other words, the behavior of a `RopeSlice` is always identical to that
/// of a full `Rope` created from the same text range.  Nothing should be
/// surprising here.
#[derive(Copy, Clone)]
pub struct RopeSlice<'a>(pub(crate) RSEnum<'a>);

#[derive(Copy, Clone, Debug)]
pub(crate) enum RSEnum<'a> {
    Full {
        node: &'a Arc<Node>,
        start_info: TextInfo,
        end_info: TextInfo,
    },
    Light {
        text: &'a str,
        char_count: Count,
        utf16_surrogate_count: Count,
        line_break_count: Count,
    },
}

impl<'a> RopeSlice<'a> {
    /// Used for tests and debugging purposes.
    #[allow(dead_code)]
    pub(crate) fn is_light(&self) -> bool {
        matches!(&self.0, RSEnum::Light { .. })
    }

    pub(crate) fn new_with_range(node: &'a Arc<Node>, start: usize, end: usize) -> RopeSlice<'a> {
        assert!(start <= end);
        assert!(end <= node.text_info().chars as usize);

        // Early-out shortcut for taking a slice of the full thing.
        if start == 0 && end == node.char_count() {
            if node.is_leaf() {
                let text = node.leaf_text();
                return RopeSlice(RSEnum::Light {
                    text: text,
                    char_count: (end - start) as Count,
                    utf16_surrogate_count: count_utf16_surrogates(text) as Count,
                    line_break_count: count_line_breaks(text) as Count,
                });
            } else {
                return RopeSlice(RSEnum::Full {
                    node: node,
                    start_info: TextInfo {
                        bytes: 0,
                        chars: 0,
                        utf16_surrogates: 0,
                        line_breaks: 0,
                    },
                    end_info: TextInfo {
                        bytes: node.byte_count() as Count,
                        chars: node.char_count() as Count,
                        utf16_surrogates: node.utf16_surrogate_count() as Count,
                        line_breaks: node.line_break_count() as Count,
                    },
                });
            }
        }

        // Find the deepest node that still contains the full range given.
        let mut n_start = start;
        let mut n_end = end;
        let mut node = node;
        'outer: loop {
            match *(node as &Node) {
                // Early out if we reach a leaf, because we can do the
                // simpler lightweight slice then.
                Node::Leaf(ref text) => {
                    let start_byte = char_to_byte_idx(text, n_start);
                    let end_byte =
                        start_byte + char_to_byte_idx(&text[start_byte..], n_end - n_start);
                    return RopeSlice(RSEnum::Light {
                        text: &text[start_byte..end_byte],
                        char_count: (n_end - n_start) as Count,
                        utf16_surrogate_count: count_utf16_surrogates(&text[start_byte..end_byte])
                            as Count,
                        line_break_count: count_line_breaks(&text[start_byte..end_byte]) as Count,
                    });
                }

                Node::Internal(ref children) => {
                    let mut start_char = 0;
                    for (i, inf) in children.info().iter().enumerate() {
                        if n_start >= start_char && n_end <= (start_char + inf.chars as usize) {
                            n_start -= start_char;
                            n_end -= start_char;
                            node = &children.nodes()[i];
                            continue 'outer;
                        }
                        start_char += inf.chars as usize;
                    }
                    break;
                }
            }
        }

        // Create the slice
        RopeSlice(RSEnum::Full {
            node: node,
            start_info: node.char_to_text_info(n_start),
            end_info: {
                #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
                {
                    let mut info = node.char_to_text_info(n_end);
                    if node.is_crlf_split(n_end) {
                        info.line_breaks += 1;
                    }
                    info
                }
                #[cfg(not(any(feature = "cr_lines", feature = "unicode_lines")))]
                node.char_to_text_info(n_end)
            },
        })
    }

    pub(crate) fn new_with_byte_range(
        node: &'a Arc<Node>,
        start: usize,
        end: usize,
    ) -> Result<RopeSlice<'a>> {
        assert!(start <= end);
        assert!(end <= node.text_info().bytes as usize);

        // Early-out shortcut for taking a slice of the full thing.
        if start == 0 && end == node.byte_count() {
            if node.is_leaf() {
                let text = node.leaf_text();
                return Ok(RopeSlice(RSEnum::Light {
                    text,
                    char_count: count_chars(text) as Count,
                    utf16_surrogate_count: count_utf16_surrogates(text) as Count,
                    line_break_count: count_line_breaks(text) as Count,
                }));
            } else {
                return Ok(RopeSlice(RSEnum::Full {
                    node,
                    start_info: TextInfo {
                        bytes: 0,
                        chars: 0,
                        utf16_surrogates: 0,
                        line_breaks: 0,
                    },
                    end_info: TextInfo {
                        bytes: node.byte_count() as Count,
                        chars: node.char_count() as Count,
                        utf16_surrogates: node.utf16_surrogate_count() as Count,
                        line_breaks: node.line_break_count() as Count,
                    },
                }));
            }
        }

        // Find the deepest node that still contains the full range given.
        let mut n_start = start;
        let mut n_end = end;
        let mut node = node;
        'outer: loop {
            match *(node as &Node) {
                // Early out if we reach a leaf, because we can do the
                // simpler lightweight slice then.
                Node::Leaf(ref text) => {
                    let start_byte = n_start;
                    let end_byte = n_end;
                    if !text.is_char_boundary(start_byte) || !text.is_char_boundary(end_byte) {
                        return Err(Error::ByteRangeNotCharBoundary(Some(start), Some(end)));
                    }
                    return Ok(RopeSlice(RSEnum::Light {
                        text: &text[start_byte..end_byte],
                        char_count: count_chars(&text[start_byte..end_byte]) as Count,
                        utf16_surrogate_count: count_utf16_surrogates(&text[start_byte..end_byte])
                            as Count,
                        line_break_count: count_line_breaks(&text[start_byte..end_byte]) as Count,
                    }));
                }

                Node::Internal(ref children) => {
                    let mut start_byte = 0;
                    for (i, inf) in children.info().iter().enumerate() {
                        if n_start >= start_byte && n_end <= (start_byte + inf.bytes as usize) {
                            n_start -= start_byte;
                            n_end -= start_byte;
                            node = &children.nodes()[i];
                            continue 'outer;
                        }
                        start_byte += inf.bytes as usize;
                    }
                    break;
                }
            }
        }

        // Make sure the bytes indices are valid char boundaries.
        if !node.is_char_boundary(n_start) || !node.is_char_boundary(n_end) {
            return Err(Error::ByteRangeNotCharBoundary(Some(start), Some(end)));
        }

        // Create the slice
        Ok(RopeSlice(RSEnum::Full {
            node,
            start_info: node.byte_to_text_info(n_start),
            end_info: node.byte_to_text_info(n_end),
        }))
    }

    //-----------------------------------------------------------------------
    // Informational methods

    /// Total number of bytes in the `RopeSlice`.
    ///
    /// Runs in O(1) time.
    #[inline]
    pub fn len_bytes(&self) -> usize {
        match *self {
            RopeSlice(RSEnum::Full {
                end_info,
                start_info,
                ..
            }) => (end_info.bytes - start_info.bytes) as usize,
            RopeSlice(RSEnum::Light { text, .. }) => text.len(),
        }
    }

    /// Total number of chars in the `RopeSlice`.
    ///
    /// Runs in O(1) time.
    #[inline]
    pub fn len_chars(&self) -> usize {
        match *self {
            RopeSlice(RSEnum::Full {
                end_info,
                start_info,
                ..
            }) => (end_info.chars - start_info.chars) as usize,
            RopeSlice(RSEnum::Light { char_count, .. }) => char_count as usize,
        }
    }

    /// Total number of lines in the `RopeSlice`.
    ///
    /// Runs in O(1) time.
    #[inline]
    pub fn len_lines(&self) -> usize {
        match *self {
            RopeSlice(RSEnum::Full {
                end_info,
                start_info,
                ..
            }) => (end_info.line_breaks - start_info.line_breaks) as usize + 1,
            RopeSlice(RSEnum::Light {
                line_break_count, ..
            }) => line_break_count as usize + 1,
        }
    }

    /// Total number of utf16 code units that would be in the `RopeSlice` if
    /// it were encoded as utf16.
    ///
    /// Ropey stores text internally as utf8, but sometimes it is necessary
    /// to interact with external APIs that still use utf16.  This function is
    /// primarily intended for such situations, and is otherwise not very
    /// useful.
    ///
    /// Runs in O(1) time.
    #[inline]
    pub fn len_utf16_cu(&self) -> usize {
        match *self {
            RopeSlice(RSEnum::Full {
                end_info,
                start_info,
                ..
            }) => {
                ((end_info.chars + end_info.utf16_surrogates)
                    - (start_info.chars + start_info.utf16_surrogates)) as usize
            }
            RopeSlice(RSEnum::Light {
                char_count,
                utf16_surrogate_count,
                ..
            }) => (char_count + utf16_surrogate_count) as usize,
        }
    }

    //-----------------------------------------------------------------------
    // Index conversion methods

    /// Returns the char index of the given byte.
    ///
    /// Notes:
    ///
    /// - If the byte is in the middle of a multi-byte char, returns the
    ///   index of the char that the byte belongs to.
    /// - `byte_idx` can be one-past-the-end, which will return one-past-the-end
    ///   char index.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    #[inline]
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.try_byte_to_char(byte_idx).unwrap()
    }

    /// Returns the line index of the given byte.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.  This is functionally equivalent to
    ///   counting the line endings before the specified byte.
    /// - `byte_idx` can be one-past-the-end, which will return the
    ///   last line index.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    #[inline]
    pub fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.try_byte_to_line(byte_idx).unwrap()
    }

    /// Returns the byte index of the given char.
    ///
    /// Notes:
    ///
    /// - `char_idx` can be one-past-the-end, which will return
    ///   one-past-the-end byte index.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    #[inline]
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.try_char_to_byte(char_idx).unwrap()
    }

    /// Returns the line index of the given char.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.  This is functionally equivalent to
    ///   counting the line endings before the specified char.
    /// - `char_idx` can be one-past-the-end, which will return the
    ///   last line index.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    #[inline]
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.try_char_to_line(char_idx).unwrap()
    }

    /// Returns the utf16 code unit index of the given char.
    ///
    /// Ropey stores text internally as utf8, but sometimes it is necessary
    /// to interact with external APIs that still use utf16.  This function is
    /// primarily intended for such situations, and is otherwise not very
    /// useful.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    #[inline]
    pub fn char_to_utf16_cu(&self, char_idx: usize) -> usize {
        self.try_char_to_utf16_cu(char_idx).unwrap()
    }

    /// Returns the char index of the given utf16 code unit.
    ///
    /// Ropey stores text internally as utf8, but sometimes it is necessary
    /// to interact with external APIs that still use utf16.  This function is
    /// primarily intended for such situations, and is otherwise not very
    /// useful.
    ///
    /// Note: if the utf16 code unit is in the middle of a char, returns the
    /// index of the char that it belongs to.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `utf16_cu_idx` is out of bounds
    /// (i.e. `utf16_cu_idx > len_utf16_cu()`).
    #[inline]
    pub fn utf16_cu_to_char(&self, utf16_cu_idx: usize) -> usize {
        self.try_utf16_cu_to_char(utf16_cu_idx).unwrap()
    }

    /// Returns the byte index of the start of the given line.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.
    /// - `line_idx` can be one-past-the-end, which will return
    ///   one-past-the-end byte index.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx` is out of bounds (i.e. `line_idx > len_lines()`).
    #[inline]
    pub fn line_to_byte(&self, line_idx: usize) -> usize {
        self.try_line_to_byte(line_idx).unwrap()
    }

    /// Returns the char index of the start of the given line.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.
    /// - `line_idx` can be one-past-the-end, which will return
    ///   one-past-the-end char index.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx` is out of bounds (i.e. `line_idx > len_lines()`).
    #[inline]
    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.try_line_to_char(line_idx).unwrap()
    }

    //-----------------------------------------------------------------------
    // Fetch methods

    /// Returns the byte at `byte_idx`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx >= len_bytes()`).
    #[inline]
    pub fn byte(&self, byte_idx: usize) -> u8 {
        // Bounds check
        if let Some(out) = self.get_byte(byte_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of slice: byte index {}, slice byte length {}",
                byte_idx,
                self.len_bytes()
            );
        }
    }

    /// Returns the char at `char_idx`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx >= len_chars()`).
    #[inline]
    pub fn char(&self, char_idx: usize) -> char {
        if let Some(out) = self.get_char(char_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of slice: char index {}, slice char length {}",
                char_idx,
                self.len_chars()
            );
        }
    }

    /// Returns the line at `line_idx`.
    ///
    /// Note: lines are zero-indexed.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx` is out of bounds (i.e. `line_idx >= len_lines()`).
    #[inline]
    pub fn line(&self, line_idx: usize) -> RopeSlice<'a> {
        if let Some(out) = self.get_line(line_idx) {
            out
        } else {
            let len_lines = self.len_lines();
            panic!(
                "Attempt to index past end of slice: line index {}, slice line length {}",
                line_idx, len_lines
            );
        }
    }

    /// Returns the chunk containing the given byte index.
    ///
    /// Also returns the byte and char indices of the beginning of the chunk
    /// and the index of the line that the chunk starts on.
    ///
    /// Note: for convenience, a one-past-the-end `byte_idx` returns the last
    /// chunk of the `RopeSlice`.
    ///
    /// The return value is organized as
    /// `(chunk, chunk_byte_idx, chunk_char_idx, chunk_line_idx)`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    pub fn chunk_at_byte(&self, byte_idx: usize) -> (&'a str, usize, usize, usize) {
        self.try_chunk_at_byte(byte_idx).unwrap()
    }

    /// Returns the chunk containing the given char index.
    ///
    /// Also returns the byte and char indices of the beginning of the chunk
    /// and the index of the line that the chunk starts on.
    ///
    /// Note: for convenience, a one-past-the-end `char_idx` returns the last
    /// chunk of the `RopeSlice`.
    ///
    /// The return value is organized as
    /// `(chunk, chunk_byte_idx, chunk_char_idx, chunk_line_idx)`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn chunk_at_char(&self, char_idx: usize) -> (&'a str, usize, usize, usize) {
        if let Some(out) = self.get_chunk_at_char(char_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of slice: char index {}, slice char length {}",
                char_idx,
                self.len_chars()
            );
        }
    }

    /// Returns the chunk containing the given line break.
    ///
    /// Also returns the byte and char indices of the beginning of the chunk
    /// and the index of the line that the chunk starts on.
    ///
    /// Note: for convenience, both the beginning and end of the slice are
    /// considered line breaks for the purposes of indexing.  For example, in
    /// the string `"Hello \n world!"` 0 would give the first chunk, 1 would
    /// give the chunk containing the newline character, and 2 would give the
    /// last chunk.
    ///
    /// The return value is organized as
    /// `(chunk, chunk_byte_idx, chunk_char_idx, chunk_line_idx)`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `line_break_idx` is out of bounds (i.e. `line_break_idx > len_lines()`).
    pub fn chunk_at_line_break(&self, line_break_idx: usize) -> (&'a str, usize, usize, usize) {
        if let Some(out) = self.get_chunk_at_line_break(line_break_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of Rope: line break index {}, max index {}",
                line_break_idx,
                self.len_lines()
            );
        }
    }

    /// Returns the entire contents of the `RopeSlice` as a `&str` if
    /// possible.
    ///
    /// This is useful for optimizing cases where the slice is only a few
    /// characters or words, and therefore has a high chance of being
    /// contiguous in memory.
    ///
    /// For large slices this method will typically fail and return `None`
    /// because large slices usually cross chunk boundaries in the rope.
    ///
    /// (Also see the `From` impl for converting to a `Cow<str>`.)
    ///
    /// Runs in O(1) time.
    #[inline]
    pub fn as_str(&self) -> Option<&'a str> {
        match *self {
            RopeSlice(RSEnum::Full { .. }) => None,
            RopeSlice(RSEnum::Light { text, .. }) => Some(text),
        }
    }

    //-----------------------------------------------------------------------
    // Slice creation

    /// Returns a sub-slice of the `RopeSlice` in the given char index range.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if the start of the range is greater than the end, or the end
    /// is out of bounds (i.e. `end > len_chars()`).
    pub fn slice<R>(&self, char_range: R) -> RopeSlice<'a>
    where
        R: RangeBounds<usize>,
    {
        let (start, end) = {
            let start_range = start_bound_to_num(char_range.start_bound());
            let end_range = end_bound_to_num(char_range.end_bound());

            // Early-out shortcut for taking a slice of the full thing.
            if start_range == None && end_range == None {
                return *self;
            }

            (
                start_range.unwrap_or(0),
                end_range.unwrap_or_else(|| self.len_chars()),
            )
        };

        // Bounds check
        assert!(start <= end);
        assert!(
            end <= self.len_chars(),
            "Attempt to slice past end of RopeSlice: slice end {}, RopeSlice length {}",
            end,
            self.len_chars()
        );

        match *self {
            RopeSlice(RSEnum::Full {
                node, start_info, ..
            }) => RopeSlice::new_with_range(
                node,
                start_info.chars as usize + start,
                start_info.chars as usize + end,
            ),
            RopeSlice(RSEnum::Light { text, .. }) => {
                let start_byte = char_to_byte_idx(text, start);
                let end_byte = char_to_byte_idx(text, end);
                let new_text = &text[start_byte..end_byte];
                RopeSlice(RSEnum::Light {
                    text: new_text,
                    char_count: (end - start) as Count,
                    utf16_surrogate_count: count_utf16_surrogates(new_text) as Count,
                    line_break_count: count_line_breaks(new_text) as Count,
                })
            }
        }
    }

    /// Returns a sub-slice of the `RopeSlice` in the given byte index range.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The start of the range is greater than the end.
    /// - The end is out of bounds (i.e. `end > len_bytes()`).
    /// - The range doesn't align with char boundaries.
    pub fn byte_slice<R>(&self, byte_range: R) -> RopeSlice<'a>
    where
        R: RangeBounds<usize>,
    {
        match self.get_byte_slice_impl(byte_range) {
            Ok(s) => return s,
            Err(e) => panic!("byte_slice(): {}", e),
        }
    }

    //-----------------------------------------------------------------------
    // Iterator methods

    /// Creates an iterator over the bytes of the `RopeSlice`.
    ///
    /// Runs in O(log N) time.
    #[inline]
    pub fn bytes(&self) -> Bytes<'a> {
        match *self {
            RopeSlice(RSEnum::Full {
                node,
                start_info,
                end_info,
            }) => Bytes::new_with_range(
                node,
                (start_info.bytes as usize, end_info.bytes as usize),
                (start_info.chars as usize, end_info.chars as usize),
                (
                    start_info.line_breaks as usize,
                    end_info.line_breaks as usize + 1,
                ),
            ),
            RopeSlice(RSEnum::Light { text, .. }) => Bytes::from_str(text),
        }
    }

    /// Creates an iterator over the bytes of the `RopeSlice`, starting at
    /// byte `byte_idx`.
    ///
    /// If `byte_idx == len_bytes()` then an iterator at the end of the
    /// `RopeSlice` is created (i.e. `next()` will return `None`).
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    #[inline]
    pub fn bytes_at(&self, byte_idx: usize) -> Bytes<'a> {
        if let Some(out) = self.get_bytes_at(byte_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of RopeSlice: byte index {}, RopeSlice byte length {}",
                byte_idx,
                self.len_bytes()
            );
        }
    }

    /// Creates an iterator over the chars of the `RopeSlice`.
    ///
    /// Runs in O(log N) time.
    #[inline]
    pub fn chars(&self) -> Chars<'a> {
        match *self {
            RopeSlice(RSEnum::Full {
                node,
                start_info,
                end_info,
            }) => Chars::new_with_range(
                node,
                (start_info.bytes as usize, end_info.bytes as usize),
                (start_info.chars as usize, end_info.chars as usize),
                (
                    start_info.line_breaks as usize,
                    end_info.line_breaks as usize + 1,
                ),
            ),
            RopeSlice(RSEnum::Light { text, .. }) => Chars::from_str(text),
        }
    }

    /// Creates an iterator over the chars of the `RopeSlice`, starting at
    /// char `char_idx`.
    ///
    /// If `char_idx == len_chars()` then an iterator at the end of the
    /// `RopeSlice` is created (i.e. `next()` will return `None`).
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    #[inline]
    pub fn chars_at(&self, char_idx: usize) -> Chars<'a> {
        if let Some(out) = self.get_chars_at(char_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of RopeSlice: char index {}, RopeSlice char length {}",
                char_idx,
                self.len_chars()
            );
        }
    }

    /// Creates an iterator over the lines of the `RopeSlice`.
    ///
    /// Runs in O(log N) time.
    #[inline]
    pub fn lines(&self) -> Lines<'a> {
        match *self {
            RopeSlice(RSEnum::Full {
                node,
                start_info,
                end_info,
            }) => Lines::new_with_range(
                node,
                (start_info.bytes as usize, end_info.bytes as usize),
                (
                    start_info.line_breaks as usize,
                    end_info.line_breaks as usize + 1,
                ),
            ),
            RopeSlice(RSEnum::Light {
                text,
                line_break_count,
                ..
            }) => Lines::from_str(text, line_break_count as usize + 1),
        }
    }

    /// Creates an iterator over the lines of the `RopeSlice`, starting at
    /// line `line_idx`.
    ///
    /// If `line_idx == len_lines()` then an iterator at the end of the
    /// `RopeSlice` is created (i.e. `next()` will return `None`).
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx` is out of bounds (i.e. `line_idx > len_lines()`).
    #[inline]
    pub fn lines_at(&self, line_idx: usize) -> Lines<'a> {
        if let Some(out) = self.get_lines_at(line_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of RopeSlice: line index {}, RopeSlice line length {}",
                line_idx,
                self.len_lines()
            );
        }
    }

    /// Creates an iterator over the chunks of the `RopeSlice`.
    ///
    /// Runs in O(log N) time.
    #[inline]
    pub fn chunks(&self) -> Chunks<'a> {
        match *self {
            RopeSlice(RSEnum::Full {
                node,
                start_info,
                end_info,
            }) => Chunks::new_with_range(
                node,
                (start_info.bytes as usize, end_info.bytes as usize),
                (start_info.chars as usize, end_info.chars as usize),
                (
                    start_info.line_breaks as usize,
                    end_info.line_breaks as usize + 1,
                ),
            ),
            RopeSlice(RSEnum::Light { text, .. }) => Chunks::from_str(text, false),
        }
    }

    /// Creates an iterator over the chunks of the `RopeSlice`, with the
    /// iterator starting at the byte containing `byte_idx`.
    ///
    /// Also returns the byte and char indices of the beginning of the first
    /// chunk to be yielded, and the index of the line that chunk starts on.
    ///
    /// If `byte_idx == len_bytes()` an iterator at the end of the `RopeSlice`
    /// (yielding `None` on a call to `next()`) is created.
    ///
    /// The return value is organized as
    /// `(iterator, chunk_byte_idx, chunk_char_idx, chunk_line_idx)`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    #[inline]
    pub fn chunks_at_byte(&self, byte_idx: usize) -> (Chunks<'a>, usize, usize, usize) {
        if let Some(out) = self.get_chunks_at_byte(byte_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of RopeSlice: byte index {}, RopeSlice byte length {}",
                byte_idx,
                self.len_bytes()
            );
        }
    }

    /// Creates an iterator over the chunks of the `RopeSlice`, with the
    /// iterator starting on the chunk containing `char_idx`.
    ///
    /// Also returns the byte and char indices of the beginning of the first
    /// chunk to be yielded, and the index of the line that chunk starts on.
    ///
    /// If `char_idx == len_chars()` an iterator at the end of the `RopeSlice`
    /// (yielding `None` on a call to `next()`) is created.
    ///
    /// The return value is organized as
    /// `(iterator, chunk_byte_idx, chunk_char_idx, chunk_line_idx)`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    #[inline]
    pub fn chunks_at_char(&self, char_idx: usize) -> (Chunks<'a>, usize, usize, usize) {
        if let Some(out) = self.get_chunks_at_char(char_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of RopeSlice: char index {}, RopeSlice char length {}",
                char_idx,
                self.len_chars()
            );
        }
    }

    /// Creates an iterator over the chunks of the `RopeSlice`, with the
    /// iterator starting at the chunk containing `line_break_idx`.
    ///
    /// Also returns the byte and char indices of the beginning of the first
    /// chunk to be yielded, and the index of the line that chunk starts on.
    ///
    /// Note: for convenience, both the beginning and end of the `RopeSlice` are
    /// considered line breaks for the purposes of indexing.  For example, in
    /// the string `"Hello \n world!"` 0 would create an iterator starting on
    /// the first chunk, 1 would create an iterator starting on the chunk
    /// containing the newline character, and 2 would create an iterator at
    /// the end of the `RopeSlice` (yielding `None` on a call to `next()`).
    ///
    /// The return value is organized as
    /// `(iterator, chunk_byte_idx, chunk_char_idx, chunk_line_idx)`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `line_break_idx` is out of bounds (i.e. `line_break_idx > len_lines()`).
    #[inline]
    pub fn chunks_at_line_break(&self, line_break_idx: usize) -> (Chunks<'a>, usize, usize, usize) {
        if let Some(out) = self.get_chunks_at_line_break(line_break_idx) {
            out
        } else {
            panic!(
                "Attempt to index past end of RopeSlice: line break index {}, RopeSlice line break max index {}",
                line_break_idx,
                self.len_lines()
            );
        }
    }
}

/// # Non-Panicking
///
/// The methods in this impl block provide non-panicking versions of
/// `RopeSlice`'s panicking methods.  They return either `Option::None` or
/// `Result::Err()` when their panicking counterparts would have panicked.
impl<'a> RopeSlice<'a> {
    /// Non-panicking version of [`byte_to_char()`](RopeSlice::byte_to_char).
    #[inline]
    pub fn try_byte_to_char(&self, byte_idx: usize) -> Result<usize> {
        // Bounds check
        if byte_idx <= self.len_bytes() {
            let (chunk, b, c, _) = self.chunk_at_byte(byte_idx);
            Ok(c + byte_to_char_idx(chunk, byte_idx - b))
        } else {
            Err(Error::ByteIndexOutOfBounds(byte_idx, self.len_bytes()))
        }
    }

    /// Non-panicking version of [`byte_to_line()`](RopeSlice::byte_to_line).
    #[inline]
    pub fn try_byte_to_line(&self, byte_idx: usize) -> Result<usize> {
        // Bounds check
        if byte_idx <= self.len_bytes() {
            let (chunk, b, _, l) = self.chunk_at_byte(byte_idx);
            Ok(l + byte_to_line_idx(chunk, byte_idx - b))
        } else {
            Err(Error::ByteIndexOutOfBounds(byte_idx, self.len_bytes()))
        }
    }

    /// Non-panicking version of [`char_to_byte()`](RopeSlice::char_to_byte).
    #[inline]
    pub fn try_char_to_byte(&self, char_idx: usize) -> Result<usize> {
        // Bounds check
        if char_idx <= self.len_chars() {
            let (chunk, b, c, _) = self.chunk_at_char(char_idx);
            Ok(b + char_to_byte_idx(chunk, char_idx - c))
        } else {
            Err(Error::CharIndexOutOfBounds(char_idx, self.len_chars()))
        }
    }

    /// Non-panicking version of [`char_to_line()`](RopeSlice::char_to_line).
    #[inline]
    pub fn try_char_to_line(&self, char_idx: usize) -> Result<usize> {
        // Bounds check
        if char_idx <= self.len_chars() {
            let (chunk, _, c, l) = self.chunk_at_char(char_idx);
            Ok(l + char_to_line_idx(chunk, char_idx - c))
        } else {
            Err(Error::CharIndexOutOfBounds(char_idx, self.len_chars()))
        }
    }

    /// Non-panicking version of [`char_to_utf16_cu()`](RopeSlice::char_to_utf16_cu).
    #[inline]
    pub fn try_char_to_utf16_cu(&self, char_idx: usize) -> Result<usize> {
        // Bounds check
        if char_idx <= self.len_chars() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node, start_info, ..
                }) => {
                    let char_idx = char_idx + start_info.chars as usize;

                    let (chunk, chunk_start_info) = node.get_chunk_at_char(char_idx);
                    let chunk_byte_idx =
                        char_to_byte_idx(chunk, char_idx - chunk_start_info.chars as usize);
                    let surrogate_count = byte_to_utf16_surrogate_idx(chunk, chunk_byte_idx);

                    Ok(
                        char_idx + chunk_start_info.utf16_surrogates as usize + surrogate_count
                            - start_info.chars as usize
                            - start_info.utf16_surrogates as usize,
                    )
                }

                RopeSlice(RSEnum::Light { text, .. }) => {
                    let byte_idx = char_to_byte_idx(text, char_idx);
                    let surrogate_count = byte_to_utf16_surrogate_idx(text, byte_idx);
                    Ok(char_idx + surrogate_count)
                }
            }
        } else {
            Err(Error::CharIndexOutOfBounds(char_idx, self.len_chars()))
        }
    }

    /// Non-panicking version of [`utf16_cu_to_char()`](RopeSlice::utf16_cu_to_char).
    #[inline]
    pub fn try_utf16_cu_to_char(&self, utf16_cu_idx: usize) -> Result<usize> {
        // Bounds check
        if utf16_cu_idx <= self.len_utf16_cu() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node, start_info, ..
                }) => {
                    let utf16_cu_idx =
                        utf16_cu_idx + (start_info.chars + start_info.utf16_surrogates) as usize;

                    let (chunk, chunk_start_info) = node.get_chunk_at_utf16_code_unit(utf16_cu_idx);
                    let chunk_utf16_cu_idx = utf16_cu_idx
                        - (chunk_start_info.chars + chunk_start_info.utf16_surrogates) as usize;
                    let chunk_char_idx = utf16_code_unit_to_char_idx(chunk, chunk_utf16_cu_idx);

                    Ok(
                        chunk_start_info.chars as usize + chunk_char_idx
                            - start_info.chars as usize,
                    )
                }

                RopeSlice(RSEnum::Light { text, .. }) => {
                    Ok(utf16_code_unit_to_char_idx(text, utf16_cu_idx))
                }
            }
        } else {
            Err(Error::Utf16IndexOutOfBounds(
                utf16_cu_idx,
                self.len_utf16_cu(),
            ))
        }
    }

    /// Non-panicking version of [`line_to_byte()`](RopeSlice::line_to_byte).
    #[inline]
    pub fn try_line_to_byte(&self, line_idx: usize) -> Result<usize> {
        // Bounds check
        if line_idx <= self.len_lines() {
            if line_idx == self.len_lines() {
                Ok(self.len_bytes())
            } else {
                let (chunk, b, _, l) = self.chunk_at_line_break(line_idx);
                Ok(b + line_to_byte_idx(chunk, line_idx - l))
            }
        } else {
            Err(Error::LineIndexOutOfBounds(line_idx, self.len_lines()))
        }
    }

    /// Non-panicking version of [`line_to_char()`](RopeSlice::line_to_char).
    #[inline]
    pub fn try_line_to_char(&self, line_idx: usize) -> Result<usize> {
        // Bounds check
        if line_idx <= self.len_lines() {
            if line_idx == self.len_lines() {
                Ok(self.len_chars())
            } else {
                let (chunk, _, c, l) = self.chunk_at_line_break(line_idx);
                Ok(c + line_to_char_idx(chunk, line_idx - l))
            }
        } else {
            Err(Error::LineIndexOutOfBounds(line_idx, self.len_lines()))
        }
    }

    /// Non-panicking version of [`get_byte()`](RopeSlice::get_byte).
    #[inline]
    pub fn get_byte(&self, byte_idx: usize) -> Option<u8> {
        // Bounds check
        if byte_idx < self.len_bytes() {
            let (chunk, chunk_byte_idx, _, _) = self.chunk_at_byte(byte_idx);
            let chunk_rel_byte_idx = byte_idx - chunk_byte_idx;
            Some(chunk.as_bytes()[chunk_rel_byte_idx])
        } else {
            None
        }
    }

    /// Non-panicking version of [`char()`](RopeSlice::char).
    #[inline]
    pub fn get_char(&self, char_idx: usize) -> Option<char> {
        // Bounds check
        if char_idx < self.len_chars() {
            let (chunk, _, chunk_char_idx, _) = self.chunk_at_char(char_idx);
            let byte_idx = char_to_byte_idx(chunk, char_idx - chunk_char_idx);
            Some(chunk[byte_idx..].chars().next().unwrap())
        } else {
            None
        }
    }

    /// Non-panicking version of [`line()`](RopeSlice::line).
    #[inline]
    pub fn get_line(&self, line_idx: usize) -> Option<RopeSlice<'a>> {
        let len_lines = self.len_lines();
        // Bounds check
        if line_idx < len_lines {
            let (chunk_1, _, c1, l1) = self.chunk_at_line_break(line_idx);
            let (chunk_2, _, c2, l2) = self.chunk_at_line_break(line_idx + 1);
            if c1 == c2 {
                let text1 = &chunk_1[line_to_byte_idx(chunk_1, line_idx - l1)..];
                let text2 = &text1[..line_to_byte_idx(text1, 1)];
                Some(RopeSlice(RSEnum::Light {
                    text: text2,
                    char_count: count_chars(text2) as Count,
                    utf16_surrogate_count: count_utf16_surrogates(text2) as Count,
                    line_break_count: if line_idx == (len_lines - 1) { 0 } else { 1 },
                }))
            } else {
                let start = c1 + line_to_char_idx(chunk_1, line_idx - l1);
                let end = c2 + line_to_char_idx(chunk_2, line_idx + 1 - l2);
                Some(self.slice(start..end))
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`chunk_at_byte()`](RopeSlice::chunk_at_byte).
    pub fn try_chunk_at_byte(&self, byte_idx: usize) -> Result<(&'a str, usize, usize, usize)> {
        // Bounds check
        if byte_idx <= self.len_bytes() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => {
                    // Get the chunk.
                    let (chunk, chunk_start_info) =
                        node.get_chunk_at_byte(byte_idx + start_info.bytes as usize);

                    // Calculate clipped start/end byte indices within the chunk.
                    let chunk_start_byte_idx =
                        start_info.bytes.saturating_sub(chunk_start_info.bytes);
                    let chunk_end_byte_idx =
                        (chunk.len() as Count).min(end_info.bytes - chunk_start_info.bytes);

                    // Return the clipped chunk and byte offset.
                    Ok((
                        &chunk[chunk_start_byte_idx as usize..chunk_end_byte_idx as usize],
                        chunk_start_info.bytes.saturating_sub(start_info.bytes) as usize,
                        chunk_start_info.chars.saturating_sub(start_info.chars) as usize,
                        chunk_start_info
                            .line_breaks
                            .saturating_sub(start_info.line_breaks)
                            as usize,
                    ))
                }
                RopeSlice(RSEnum::Light { text, .. }) => Ok((text, 0, 0, 0)),
            }
        } else {
            Err(Error::ByteIndexOutOfBounds(byte_idx, self.len_bytes()))
        }
    }

    /// Non-panicking version of [`chunk_at_char()`](RopeSlice::chunk_at_char).
    pub fn get_chunk_at_char(&self, char_idx: usize) -> Option<(&'a str, usize, usize, usize)> {
        // Bounds check
        if char_idx <= self.len_chars() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => {
                    // Get the chunk.
                    let (chunk, chunk_start_info) =
                        node.get_chunk_at_char(char_idx + start_info.chars as usize);

                    // Calculate clipped start/end byte indices within the chunk.
                    let chunk_start_byte_idx =
                        start_info.bytes.saturating_sub(chunk_start_info.bytes);
                    let chunk_end_byte_idx =
                        (chunk.len() as Count).min(end_info.bytes - chunk_start_info.bytes);

                    // Return the clipped chunk and byte offset.
                    Some((
                        &chunk[chunk_start_byte_idx as usize..chunk_end_byte_idx as usize],
                        chunk_start_info.bytes.saturating_sub(start_info.bytes) as usize,
                        chunk_start_info.chars.saturating_sub(start_info.chars) as usize,
                        chunk_start_info
                            .line_breaks
                            .saturating_sub(start_info.line_breaks)
                            as usize,
                    ))
                }
                RopeSlice(RSEnum::Light { text, .. }) => Some((text, 0, 0, 0)),
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`chunk_at_line_break()`](RopeSlice::chunk_at_line_break).
    pub fn get_chunk_at_line_break(
        &self,
        line_break_idx: usize,
    ) -> Option<(&'a str, usize, usize, usize)> {
        // Bounds check
        if line_break_idx <= self.len_lines() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => {
                    // Get the chunk.
                    let (chunk, chunk_start_info) = if line_break_idx == 0 {
                        node.get_chunk_at_byte(start_info.bytes as usize)
                    } else if line_break_idx == self.len_lines() {
                        node.get_chunk_at_byte(end_info.bytes as usize)
                    } else {
                        node.get_chunk_at_line_break(
                            line_break_idx + start_info.line_breaks as usize,
                        )
                    };

                    // Calculate clipped start/end byte indices within the chunk.
                    let chunk_start_byte_idx =
                        start_info.bytes.saturating_sub(chunk_start_info.bytes);
                    let chunk_end_byte_idx =
                        (chunk.len() as Count).min(end_info.bytes - chunk_start_info.bytes);

                    // Return the clipped chunk and byte offset.
                    Some((
                        &chunk[chunk_start_byte_idx as usize..chunk_end_byte_idx as usize],
                        chunk_start_info.bytes.saturating_sub(start_info.bytes) as usize,
                        chunk_start_info.chars.saturating_sub(start_info.chars) as usize,
                        chunk_start_info
                            .line_breaks
                            .saturating_sub(start_info.line_breaks)
                            as usize,
                    ))
                }
                RopeSlice(RSEnum::Light { text, .. }) => Some((text, 0, 0, 0)),
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`slice()`](RopeSlice::slice).
    pub fn get_slice<R>(&self, char_range: R) -> Option<RopeSlice<'a>>
    where
        R: RangeBounds<usize>,
    {
        let (start, end) = {
            let start_range = start_bound_to_num(char_range.start_bound());
            let end_range = end_bound_to_num(char_range.end_bound());

            // Early-out shortcut for taking a slice of the full thing.
            if start_range == None && end_range == None {
                return Some(*self);
            }

            (
                start_range.unwrap_or(0),
                end_range.unwrap_or_else(|| self.len_chars()),
            )
        };

        // Bounds check
        if start <= end && end <= self.len_chars() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node, start_info, ..
                }) => Some(RopeSlice::new_with_range(
                    node,
                    start_info.chars as usize + start,
                    start_info.chars as usize + end,
                )),
                RopeSlice(RSEnum::Light { text, .. }) => {
                    let start_byte = char_to_byte_idx(text, start);
                    let end_byte = char_to_byte_idx(text, end);
                    let new_text = &text[start_byte..end_byte];
                    Some(RopeSlice(RSEnum::Light {
                        text: new_text,
                        char_count: (end - start) as Count,
                        utf16_surrogate_count: count_utf16_surrogates(new_text) as Count,
                        line_break_count: count_line_breaks(new_text) as Count,
                    }))
                }
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`byte_slice()`](RopeSlice::byte_slice).
    pub fn get_byte_slice<R>(&self, byte_range: R) -> Option<RopeSlice<'a>>
    where
        R: RangeBounds<usize>,
    {
        self.get_byte_slice_impl(byte_range).ok()
    }

    pub(crate) fn get_byte_slice_impl<R>(&self, byte_range: R) -> Result<RopeSlice<'a>>
    where
        R: RangeBounds<usize>,
    {
        let start_range = start_bound_to_num(byte_range.start_bound());
        let end_range = end_bound_to_num(byte_range.end_bound());

        // Bounds checks.
        match (start_range, end_range) {
            (None, None) => {
                // Early-out shortcut for taking a slice of the full thing.
                return Ok(*self);
            }
            (Some(s), Some(e)) => {
                if s > e {
                    return Err(Error::ByteRangeInvalid(s, e));
                } else if e > self.len_bytes() {
                    return Err(Error::ByteRangeOutOfBounds(
                        start_range,
                        end_range,
                        self.len_bytes(),
                    ));
                }
            }
            (Some(s), None) => {
                if s > self.len_bytes() {
                    return Err(Error::ByteRangeOutOfBounds(
                        start_range,
                        end_range,
                        self.len_bytes(),
                    ));
                }
            }
            (None, Some(e)) => {
                if e > self.len_bytes() {
                    return Err(Error::ByteRangeOutOfBounds(
                        start_range,
                        end_range,
                        self.len_bytes(),
                    ));
                }
            }
        }

        let (start, end) = (
            start_range.unwrap_or(0),
            end_range.unwrap_or_else(|| self.len_bytes()),
        );

        match *self {
            RopeSlice(RSEnum::Full {
                node, start_info, ..
            }) => RopeSlice::new_with_byte_range(
                node,
                start_info.bytes as usize + start,
                start_info.bytes as usize + end,
            )
            .map_err(|e| {
                if let Error::ByteRangeNotCharBoundary(_, _) = e {
                    Error::ByteRangeNotCharBoundary(start_range, end_range)
                } else {
                    e
                }
            }),
            RopeSlice(RSEnum::Light { text, .. }) => {
                if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                    return Err(Error::ByteRangeNotCharBoundary(start_range, end_range));
                }
                let new_text = &text[start..end];
                Ok(RopeSlice(RSEnum::Light {
                    text: new_text,
                    char_count: count_chars(new_text) as Count,
                    utf16_surrogate_count: count_utf16_surrogates(new_text) as Count,
                    line_break_count: count_line_breaks(new_text) as Count,
                }))
            }
        }
    }

    /// Non-panicking version of [`bytes_at()`](RopeSlice::bytes_at).
    #[inline]
    pub fn get_bytes_at(&self, byte_idx: usize) -> Option<Bytes<'a>> {
        // Bounds check
        if byte_idx <= self.len_bytes() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => Some(Bytes::new_with_range_at(
                    node,
                    start_info.bytes as usize + byte_idx,
                    (start_info.bytes as usize, end_info.bytes as usize),
                    (start_info.chars as usize, end_info.chars as usize),
                    (
                        start_info.line_breaks as usize,
                        end_info.line_breaks as usize + 1,
                    ),
                )),
                RopeSlice(RSEnum::Light { text, .. }) => Some(Bytes::from_str_at(text, byte_idx)),
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`chars_at()`](RopeSlice::chars_at).
    #[inline]
    pub fn get_chars_at(&self, char_idx: usize) -> Option<Chars<'a>> {
        // Bounds check
        if char_idx <= self.len_chars() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => Some(Chars::new_with_range_at(
                    node,
                    start_info.chars as usize + char_idx,
                    (start_info.bytes as usize, end_info.bytes as usize),
                    (start_info.chars as usize, end_info.chars as usize),
                    (
                        start_info.line_breaks as usize,
                        end_info.line_breaks as usize + 1,
                    ),
                )),
                RopeSlice(RSEnum::Light { text, .. }) => Some(Chars::from_str_at(text, char_idx)),
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`lines_at()`](RopeSlice::lines_at).
    #[inline]
    pub fn get_lines_at(&self, line_idx: usize) -> Option<Lines<'a>> {
        // Bounds check
        if line_idx <= self.len_lines() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => Some(Lines::new_with_range_at(
                    node,
                    start_info.line_breaks as usize + line_idx,
                    (start_info.bytes as usize, end_info.bytes as usize),
                    (
                        start_info.line_breaks as usize,
                        end_info.line_breaks as usize + 1,
                    ),
                )),
                RopeSlice(RSEnum::Light {
                    text,
                    line_break_count,
                    ..
                }) => Some(Lines::from_str_at(
                    text,
                    line_idx,
                    line_break_count as usize + 1,
                )),
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`chunks_at_byte()`](RopeSlice::chunks_at_byte).
    #[inline]
    pub fn get_chunks_at_byte(&self, byte_idx: usize) -> Option<(Chunks<'a>, usize, usize, usize)> {
        // Bounds check
        if byte_idx <= self.len_bytes() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => {
                    let (chunks, chunk_byte_idx, chunk_char_idx, chunk_line_idx) =
                        Chunks::new_with_range_at_byte(
                            node,
                            byte_idx + start_info.bytes as usize,
                            (start_info.bytes as usize, end_info.bytes as usize),
                            (start_info.chars as usize, end_info.chars as usize),
                            (
                                start_info.line_breaks as usize,
                                end_info.line_breaks as usize + 1,
                            ),
                        );

                    Some((
                        chunks,
                        chunk_byte_idx.saturating_sub(start_info.bytes as usize),
                        chunk_char_idx.saturating_sub(start_info.chars as usize),
                        chunk_line_idx.saturating_sub(start_info.line_breaks as usize),
                    ))
                }
                RopeSlice(RSEnum::Light {
                    text,
                    char_count,
                    line_break_count,
                    ..
                }) => {
                    let chunks = Chunks::from_str(text, byte_idx == text.len());

                    if byte_idx == text.len() {
                        Some((
                            chunks,
                            text.len(),
                            char_count as usize,
                            line_break_count as usize,
                        ))
                    } else {
                        Some((chunks, 0, 0, 0))
                    }
                }
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`chunks_at_char()`](RopeSlice::chunks_at_char).
    #[inline]
    pub fn get_chunks_at_char(&self, char_idx: usize) -> Option<(Chunks<'a>, usize, usize, usize)> {
        // Bounds check
        if char_idx <= self.len_chars() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => {
                    let (chunks, chunk_byte_idx, chunk_char_idx, chunk_line_idx) =
                        Chunks::new_with_range_at_char(
                            node,
                            char_idx + start_info.chars as usize,
                            (start_info.bytes as usize, end_info.bytes as usize),
                            (start_info.chars as usize, end_info.chars as usize),
                            (
                                start_info.line_breaks as usize,
                                end_info.line_breaks as usize + 1,
                            ),
                        );

                    Some((
                        chunks,
                        chunk_byte_idx.saturating_sub(start_info.bytes as usize),
                        chunk_char_idx.saturating_sub(start_info.chars as usize),
                        chunk_line_idx.saturating_sub(start_info.line_breaks as usize),
                    ))
                }
                RopeSlice(RSEnum::Light {
                    text,
                    char_count,
                    line_break_count,
                    ..
                }) => {
                    let chunks = Chunks::from_str(text, char_idx == char_count as usize);

                    if char_idx == char_count as usize {
                        Some((
                            chunks,
                            text.len(),
                            char_count as usize,
                            line_break_count as usize,
                        ))
                    } else {
                        Some((chunks, 0, 0, 0))
                    }
                }
            }
        } else {
            None
        }
    }

    /// Non-panicking version of [`chunks_at_line_break()`](RopeSlice::chunks_at_line_break).
    #[inline]
    pub fn get_chunks_at_line_break(
        &self,
        line_break_idx: usize,
    ) -> Option<(Chunks<'a>, usize, usize, usize)> {
        // Bounds check
        if line_break_idx <= self.len_lines() {
            match *self {
                RopeSlice(RSEnum::Full {
                    node,
                    start_info,
                    end_info,
                }) => {
                    // Get the chunk.
                    let (chunks, chunk_byte_idx, chunk_char_idx, chunk_line_idx) =
                        if line_break_idx == 0 {
                            Chunks::new_with_range_at_byte(
                                node,
                                start_info.bytes as usize,
                                (start_info.bytes as usize, end_info.bytes as usize),
                                (start_info.chars as usize, end_info.chars as usize),
                                (
                                    start_info.line_breaks as usize,
                                    end_info.line_breaks as usize + 1,
                                ),
                            )
                        } else if line_break_idx == self.len_lines() {
                            Chunks::new_with_range_at_byte(
                                node,
                                end_info.bytes as usize,
                                (start_info.bytes as usize, end_info.bytes as usize),
                                (start_info.chars as usize, end_info.chars as usize),
                                (
                                    start_info.line_breaks as usize,
                                    end_info.line_breaks as usize + 1,
                                ),
                            )
                        } else {
                            Chunks::new_with_range_at_line_break(
                                node,
                                line_break_idx + start_info.line_breaks as usize,
                                (start_info.bytes as usize, end_info.bytes as usize),
                                (start_info.chars as usize, end_info.chars as usize),
                                (
                                    start_info.line_breaks as usize,
                                    end_info.line_breaks as usize + 1,
                                ),
                            )
                        };
                    Some((
                        chunks,
                        chunk_byte_idx.saturating_sub(start_info.bytes as usize),
                        chunk_char_idx.saturating_sub(start_info.chars as usize),
                        chunk_line_idx.saturating_sub(start_info.line_breaks as usize),
                    ))
                }
                RopeSlice(RSEnum::Light {
                    text,
                    char_count,
                    line_break_count,
                    ..
                }) => {
                    let chunks =
                        Chunks::from_str(text, line_break_idx == line_break_count as usize);

                    if line_break_idx == line_break_count as usize {
                        Some((
                            chunks,
                            text.len(),
                            char_count as usize,
                            line_break_count as usize,
                        ))
                    } else {
                        Some((chunks, 0, 0, 0))
                    }
                }
            }
        } else {
            None
        }
    }
}

//==============================================================
// Conversion impls

/// Creates a `RopeSlice` directly from a string slice.
///
/// The useful applications of this are actually somewhat narrow.  It is
/// intended primarily as an aid when implementing additional functionality
/// on top of Ropey, where you may already have access to a rope chunk and
/// want to directly create a `RopeSlice` from it, avoiding the overhead of
/// going through the slicing APIs.
///
/// Although it is possible to use this to create `RopeSlice`s from
/// arbitrary strings, doing so is not especially useful.  For example,
/// `Rope`s and `RopeSlice`s can already be directly compared for
/// equality with strings and string slices.
///
/// Runs in O(N) time, where N is the length of the string slice.
impl<'a> From<&'a str> for RopeSlice<'a> {
    #[inline]
    fn from(text: &'a str) -> Self {
        RopeSlice(RSEnum::Light {
            text: text,
            char_count: count_chars(text) as Count,
            utf16_surrogate_count: count_utf16_surrogates(text) as Count,
            line_break_count: count_line_breaks(text) as Count,
        })
    }
}

impl<'a> From<RopeSlice<'a>> for String {
    #[inline]
    fn from(s: RopeSlice<'a>) -> Self {
        let mut text = String::with_capacity(s.len_bytes());
        text.extend(s.chunks());
        text
    }
}

/// Attempts to borrow the contents of the slice, but will convert to an
/// owned string if the contents is not contiguous in memory.
///
/// Runs in best case O(1), worst case O(N).
impl<'a> From<RopeSlice<'a>> for std::borrow::Cow<'a, str> {
    #[inline]
    fn from(s: RopeSlice<'a>) -> Self {
        if let Some(text) = s.as_str() {
            std::borrow::Cow::Borrowed(text)
        } else {
            std::borrow::Cow::Owned(String::from(s))
        }
    }
}

//==============================================================
// Other impls

impl<'a> std::fmt::Debug for RopeSlice<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list().entries(self.chunks()).finish()
    }
}

impl<'a> std::fmt::Display for RopeSlice<'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for chunk in self.chunks() {
            write!(f, "{}", chunk)?
        }
        Ok(())
    }
}

impl<'a> std::cmp::Eq for RopeSlice<'a> {}

impl<'a, 'b> std::cmp::PartialEq<RopeSlice<'b>> for RopeSlice<'a> {
    fn eq(&self, other: &RopeSlice<'b>) -> bool {
        if self.len_bytes() != other.len_bytes() {
            return false;
        }

        let mut chunk_itr_1 = self.chunks();
        let mut chunk_itr_2 = other.chunks();
        let mut chunk1 = chunk_itr_1.next().unwrap_or("").as_bytes();
        let mut chunk2 = chunk_itr_2.next().unwrap_or("").as_bytes();

        loop {
            if chunk1.len() > chunk2.len() {
                if &chunk1[..chunk2.len()] != chunk2 {
                    return false;
                } else {
                    chunk1 = &chunk1[chunk2.len()..];
                    chunk2 = &[];
                }
            } else if &chunk2[..chunk1.len()] != chunk1 {
                return false;
            } else {
                chunk2 = &chunk2[chunk1.len()..];
                chunk1 = &[];
            }

            if chunk1.is_empty() {
                if let Some(chunk) = chunk_itr_1.next() {
                    chunk1 = chunk.as_bytes();
                } else {
                    break;
                }
            }

            if chunk2.is_empty() {
                if let Some(chunk) = chunk_itr_2.next() {
                    chunk2 = chunk.as_bytes();
                } else {
                    break;
                }
            }
        }

        return true;
    }
}

impl<'a, 'b> std::cmp::PartialEq<&'b str> for RopeSlice<'a> {
    #[inline]
    fn eq(&self, other: &&'b str) -> bool {
        match *self {
            RopeSlice(RSEnum::Full { .. }) => {
                if self.len_bytes() != other.len() {
                    return false;
                }
                let other = other.as_bytes();

                let mut idx = 0;
                for chunk in self.chunks() {
                    let chunk = chunk.as_bytes();
                    if chunk != &other[idx..(idx + chunk.len())] {
                        return false;
                    }
                    idx += chunk.len();
                }

                return true;
            }
            RopeSlice(RSEnum::Light { text, .. }) => {
                return text == *other;
            }
        }
    }
}

impl<'a, 'b> std::cmp::PartialEq<RopeSlice<'a>> for &'b str {
    #[inline]
    fn eq(&self, other: &RopeSlice<'a>) -> bool {
        other == self
    }
}

impl<'a> std::cmp::PartialEq<str> for RopeSlice<'a> {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        std::cmp::PartialEq::<&str>::eq(self, &other)
    }
}

impl<'a> std::cmp::PartialEq<RopeSlice<'a>> for str {
    #[inline]
    fn eq(&self, other: &RopeSlice<'a>) -> bool {
        std::cmp::PartialEq::<&str>::eq(other, &self)
    }
}

impl<'a> std::cmp::PartialEq<String> for RopeSlice<'a> {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

impl<'a> std::cmp::PartialEq<RopeSlice<'a>> for String {
    #[inline]
    fn eq(&self, other: &RopeSlice<'a>) -> bool {
        self.as_str() == other
    }
}

impl<'a, 'b> std::cmp::PartialEq<std::borrow::Cow<'b, str>> for RopeSlice<'a> {
    #[inline]
    fn eq(&self, other: &std::borrow::Cow<'b, str>) -> bool {
        *self == **other
    }
}

impl<'a, 'b> std::cmp::PartialEq<RopeSlice<'a>> for std::borrow::Cow<'b, str> {
    #[inline]
    fn eq(&self, other: &RopeSlice<'a>) -> bool {
        **self == *other
    }
}

impl<'a> std::cmp::PartialEq<Rope> for RopeSlice<'a> {
    #[inline]
    fn eq(&self, other: &Rope) -> bool {
        *self == other.slice(..)
    }
}

impl<'a> std::cmp::PartialEq<RopeSlice<'a>> for Rope {
    #[inline]
    fn eq(&self, other: &RopeSlice<'a>) -> bool {
        self.slice(..) == *other
    }
}

impl<'a> std::cmp::Ord for RopeSlice<'a> {
    #[allow(clippy::op_ref)] // Erroneously thinks with can directly use a slice.
    fn cmp(&self, other: &RopeSlice<'a>) -> std::cmp::Ordering {
        let mut chunk_itr_1 = self.chunks();
        let mut chunk_itr_2 = other.chunks();
        let mut chunk1 = chunk_itr_1.next().unwrap_or("").as_bytes();
        let mut chunk2 = chunk_itr_2.next().unwrap_or("").as_bytes();

        loop {
            if chunk1.len() >= chunk2.len() {
                let compared = chunk1[..chunk2.len()].cmp(chunk2);
                if compared != std::cmp::Ordering::Equal {
                    return compared;
                }

                chunk1 = &chunk1[chunk2.len()..];
                chunk2 = &[];
            } else {
                let compared = chunk1.cmp(&chunk2[..chunk1.len()]);
                if compared != std::cmp::Ordering::Equal {
                    return compared;
                }

                chunk1 = &[];
                chunk2 = &chunk2[chunk1.len()..];
            }

            if chunk1.is_empty() {
                if let Some(chunk) = chunk_itr_1.next() {
                    chunk1 = chunk.as_bytes();
                } else {
                    break;
                }
            }

            if chunk2.is_empty() {
                if let Some(chunk) = chunk_itr_2.next() {
                    chunk2 = chunk.as_bytes();
                } else {
                    break;
                }
            }
        }

        self.len_bytes().cmp(&other.len_bytes())
    }
}

impl<'a, 'b> std::cmp::PartialOrd<RopeSlice<'b>> for RopeSlice<'a> {
    #[inline]
    fn partial_cmp(&self, other: &RopeSlice<'b>) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> std::hash::Hash for RopeSlice<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // `std::hash::Hasher` only guarantees the same hash output for
        // exactly the same calls to `Hasher::write()`.  Just submitting
        // the same data in the same order isn't enough--it also has to
        // be split the same between calls.  So we go to some effort here
        // to ensure that we always submit the text data in the same
        // fixed-size blocks, even if those blocks don't align with chunk
        // boundaries at all.
        //
        // The naive approach is to always copy to a fixed-size buffer
        // and submit the buffer whenever it fills up.  We conceptually
        // follow that approach here, but we do a little better by
        // skipping the buffer and directly passing the data without
        // copying when possible.
        const BLOCK_SIZE: usize = 256;

        let mut buffer = [0u8; BLOCK_SIZE];
        let mut buffer_len = 0;

        for chunk in self.chunks() {
            let mut data = chunk.as_bytes();

            while !data.is_empty() {
                if buffer_len == 0 && data.len() >= BLOCK_SIZE {
                    // Process data directly, skipping the buffer.
                    let (head, tail) = data.split_at(BLOCK_SIZE);
                    state.write(head);
                    data = tail;
                } else if buffer_len == BLOCK_SIZE {
                    // Process the filled buffer.
                    state.write(&buffer[..]);
                    buffer_len = 0;
                } else {
                    // Append to the buffer.
                    let n = (BLOCK_SIZE - buffer_len).min(data.len());
                    let (head, tail) = data.split_at(n);
                    (&mut buffer[buffer_len..(buffer_len + n)]).copy_from_slice(head);
                    buffer_len += n;
                    data = tail;
                }
            }
        }

        // Write any remaining unprocessed data in the buffer.
        if buffer_len > 0 {
            state.write(&buffer[..buffer_len]);
        }

        // Same strategy as `&str` in stdlib, so that e.g. two adjacent
        // fields in a `#[derive(Hash)]` struct with "Hi " and "there"
        // vs "Hi t" and "here" give the struct a different hash.
        state.write_u8(0xff)
    }
}

//===========================================================

#[cfg(test)]
mod tests {
    use crate::str_utils::{
        byte_to_char_idx, byte_to_line_idx, char_to_byte_idx, char_to_line_idx,
    };
    use crate::Rope;
    use std::hash::{Hash, Hasher};

    // 127 bytes, 103 chars, 1 line
    const TEXT: &str = "Hello there!  How're you doing?  It's \
                        a fine day, isn't it?  Aren't you glad \
                        we're alive?  こんにちは、みんなさん！";
    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";
    // 127 bytes, 107 chars, 111 utf16 code units, 1 line
    const TEXT_EMOJI: &str = "Hello there!🐸  How're you doing?🐸  It's \
                              a fine day, isn't it?🐸  Aren't you glad \
                              we're alive?🐸  こんにちは、みんなさん！";

    #[test]
    fn len_bytes_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(7..98);
        assert_eq!(s.len_bytes(), 105);
    }

    #[test]
    fn len_bytes_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);
        assert_eq!(s.len_bytes(), 0);
    }

    #[test]
    fn len_chars_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(7..98);
        assert_eq!(s.len_chars(), 91);
    }

    #[test]
    fn len_chars_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);
        assert_eq!(s.len_chars(), 0);
    }

    #[test]
    fn len_lines_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..98);
        assert_eq!(s.len_lines(), 3);
    }

    #[test]
    fn len_lines_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);
        assert_eq!(s.len_lines(), 1);
    }

    #[test]
    fn len_lines_03() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
        for i in 0..r.len_chars() {
            if cfg!(any(feature = "cr_lines", feature = "unicode_lines")) {
                assert_eq!(r.slice(..i).len_lines(), 1 + ((i + 1) / 2));
            } else {
                assert_eq!(r.slice(..i).len_lines(), 1 + (i / 2));
            }
        }
    }

    #[test]
    fn len_lines_04() {
        // Make sure splitting CRLF pairs at the start works properly.
        let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
        for i in 0..r.len_chars() {
            assert_eq!(r.slice(i..).len_lines(), 16 - (i / 2));
        }
    }

    #[test]
    fn len_utf16_cu_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(..);
        assert_eq!(s.len_utf16_cu(), 103);
    }

    #[test]
    fn len_utf16_cu_02() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);
        assert_eq!(s.len_utf16_cu(), 111);
    }

    #[test]
    fn len_utf16_cu_03() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(13..33);
        assert_eq!(s.len_utf16_cu(), 21);
    }

    #[test]
    fn len_utf16_cu_04() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        assert_eq!(s.len_utf16_cu(), 2);
    }

    #[test]
    fn len_utf16_cu_05() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        assert_eq!(s.len_utf16_cu(), 0);
    }

    #[test]
    fn byte_to_char_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(88..102);

        // ?  こんにちは、みんなさん

        assert_eq!(0, s.byte_to_char(0));
        assert_eq!(1, s.byte_to_char(1));
        assert_eq!(2, s.byte_to_char(2));

        assert_eq!(3, s.byte_to_char(3));
        assert_eq!(3, s.byte_to_char(4));
        assert_eq!(3, s.byte_to_char(5));

        assert_eq!(4, s.byte_to_char(6));
        assert_eq!(4, s.byte_to_char(7));
        assert_eq!(4, s.byte_to_char(8));

        assert_eq!(13, s.byte_to_char(33));
        assert_eq!(13, s.byte_to_char(34));
        assert_eq!(13, s.byte_to_char(35));
        assert_eq!(14, s.byte_to_char(36));
    }

    #[test]
    fn byte_to_line_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        // 's a fine day, isn't it?\nAren't you glad \
        // we're alive?\nこんにちは、みん

        assert_eq!(0, s.byte_to_line(0));
        assert_eq!(0, s.byte_to_line(1));

        assert_eq!(0, s.byte_to_line(24));
        assert_eq!(1, s.byte_to_line(25));
        assert_eq!(1, s.byte_to_line(26));

        assert_eq!(1, s.byte_to_line(53));
        assert_eq!(2, s.byte_to_line(54));
        assert_eq!(2, s.byte_to_line(57));

        assert_eq!(2, s.byte_to_line(78));
    }

    #[test]
    fn byte_to_line_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(50..50);
        assert_eq!(0, s.byte_to_line(0));
    }

    #[test]
    fn byte_to_line_03() {
        let r = Rope::from_str("Hi there\nstranger!");
        let s = r.slice(0..9);
        assert_eq!(0, s.byte_to_line(0));
        assert_eq!(0, s.byte_to_line(8));
        assert_eq!(1, s.byte_to_line(9));
    }

    #[test]
    #[should_panic]
    fn byte_to_line_04() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);
        s.byte_to_line(79);
    }

    #[test]
    fn byte_to_line_05() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
        for i in 0..r.len_bytes() {
            if cfg!(any(feature = "cr_lines", feature = "unicode_lines")) {
                assert_eq!(r.byte_slice(..i).byte_to_line(i), (i + 1) / 2);
            } else {
                assert_eq!(r.byte_slice(..i).byte_to_line(i), i / 2);
            }
        }
    }

    #[test]
    fn char_to_byte_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(88..102);

        // ?  こんにちは、みんなさん

        assert_eq!(0, s.char_to_byte(0));
        assert_eq!(1, s.char_to_byte(1));
        assert_eq!(2, s.char_to_byte(2));

        assert_eq!(3, s.char_to_byte(3));
        assert_eq!(6, s.char_to_byte(4));
        assert_eq!(33, s.char_to_byte(13));
        assert_eq!(36, s.char_to_byte(14));
    }

    #[test]
    fn char_to_line_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        // 's a fine day, isn't it?\nAren't you glad \
        // we're alive?\nこんにちは、みん

        assert_eq!(0, s.char_to_line(0));
        assert_eq!(0, s.char_to_line(1));

        assert_eq!(0, s.char_to_line(24));
        assert_eq!(1, s.char_to_line(25));
        assert_eq!(1, s.char_to_line(26));

        assert_eq!(1, s.char_to_line(53));
        assert_eq!(2, s.char_to_line(54));
        assert_eq!(2, s.char_to_line(55));

        assert_eq!(2, s.char_to_line(62));
    }

    #[test]
    fn char_to_line_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);

        assert_eq!(0, s.char_to_line(0));
    }

    #[test]
    fn char_to_line_03() {
        let r = Rope::from_str("Hi there\nstranger!");
        let s = r.slice(0..9);
        assert_eq!(0, s.char_to_line(0));
        assert_eq!(0, s.char_to_line(8));
        assert_eq!(1, s.char_to_line(9));
    }

    #[test]
    #[should_panic]
    fn char_to_line_04() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        s.char_to_line(63);
    }

    #[test]
    fn char_to_line_05() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
        for i in 0..r.len_chars() {
            if cfg!(any(feature = "cr_lines", feature = "unicode_lines")) {
                assert_eq!(r.slice(..i).char_to_line(i), (i + 1) / 2);
            } else {
                assert_eq!(r.slice(..i).char_to_line(i), i / 2);
            }
        }
    }

    #[test]
    fn line_to_byte_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        // 's a fine day, isn't it?\nAren't you glad \
        // we're alive?\nこんにちは、みん

        assert_eq!(0, s.line_to_byte(0));
        assert_eq!(25, s.line_to_byte(1));
        assert_eq!(54, s.line_to_byte(2));
        assert_eq!(78, s.line_to_byte(3));
    }

    #[test]
    fn line_to_byte_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);

        assert_eq!(0, s.line_to_byte(0));
        assert_eq!(0, s.line_to_byte(1));
    }

    #[test]
    #[should_panic]
    fn line_to_byte_03() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        s.line_to_byte(4);
    }

    #[test]
    fn line_to_byte_04() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
        for i in 0..r.len_bytes() {
            assert_eq!(r.byte_slice(..i).line_to_byte((i + 1) / 2), i);
        }
    }

    #[test]
    fn line_to_char_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        assert_eq!(0, s.line_to_char(0));
        assert_eq!(25, s.line_to_char(1));
        assert_eq!(54, s.line_to_char(2));
        assert_eq!(62, s.line_to_char(3));
    }

    #[test]
    fn line_to_char_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);

        assert_eq!(0, s.line_to_char(0));
        assert_eq!(0, s.line_to_char(1));
    }

    #[test]
    #[should_panic]
    fn line_to_char_03() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        s.line_to_char(4);
    }

    #[test]
    fn line_to_char_04() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
        for i in 0..r.len_chars() {
            assert_eq!(r.slice(..i).line_to_char((i + 1) / 2), i);
        }
    }

    #[test]
    fn char_to_utf16_cu_01() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        assert_eq!(0, s.char_to_utf16_cu(0));
    }

    #[test]
    #[should_panic]
    fn char_to_utf16_cu_02() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        s.char_to_utf16_cu(1);
    }

    #[test]
    fn char_to_utf16_cu_03() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        assert_eq!(0, s.char_to_utf16_cu(0));
        assert_eq!(2, s.char_to_utf16_cu(1));
    }

    #[test]
    #[should_panic]
    fn char_to_utf16_cu_04() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        s.char_to_utf16_cu(2);
    }

    #[test]
    fn char_to_utf16_cu_05() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);

        assert_eq!(0, s.char_to_utf16_cu(0));

        assert_eq!(12, s.char_to_utf16_cu(12));
        assert_eq!(14, s.char_to_utf16_cu(13));

        assert_eq!(33, s.char_to_utf16_cu(32));
        assert_eq!(35, s.char_to_utf16_cu(33));

        assert_eq!(63, s.char_to_utf16_cu(61));
        assert_eq!(65, s.char_to_utf16_cu(62));

        assert_eq!(95, s.char_to_utf16_cu(92));
        assert_eq!(97, s.char_to_utf16_cu(93));

        assert_eq!(111, s.char_to_utf16_cu(107));
    }

    #[test]
    #[should_panic]
    fn char_to_utf16_cu_06() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);
        s.char_to_utf16_cu(108);
    }

    #[test]
    fn char_to_utf16_cu_07() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..106);

        assert_eq!(0, s.char_to_utf16_cu(0));

        assert_eq!(11, s.char_to_utf16_cu(11));
        assert_eq!(13, s.char_to_utf16_cu(12));

        assert_eq!(32, s.char_to_utf16_cu(31));
        assert_eq!(34, s.char_to_utf16_cu(32));

        assert_eq!(62, s.char_to_utf16_cu(60));
        assert_eq!(64, s.char_to_utf16_cu(61));

        assert_eq!(94, s.char_to_utf16_cu(91));
        assert_eq!(96, s.char_to_utf16_cu(92));

        assert_eq!(109, s.char_to_utf16_cu(105));
    }

    #[test]
    #[should_panic]
    fn char_to_utf16_cu_08() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..106);
        s.char_to_utf16_cu(106);
    }

    #[test]
    fn utf16_cu_to_char_01() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        assert_eq!(0, s.utf16_cu_to_char(0));
    }

    #[test]
    #[should_panic]
    fn utf16_cu_to_char_02() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        s.utf16_cu_to_char(1);
    }

    #[test]
    fn utf16_cu_to_char_03() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        assert_eq!(0, s.utf16_cu_to_char(0));
        assert_eq!(0, s.utf16_cu_to_char(1));
        assert_eq!(1, s.utf16_cu_to_char(2));
    }

    #[test]
    #[should_panic]
    fn utf16_cu_to_char_04() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        s.utf16_cu_to_char(3);
    }

    #[test]
    fn utf16_cu_to_char_05() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);

        assert_eq!(0, s.utf16_cu_to_char(0));

        assert_eq!(12, s.utf16_cu_to_char(12));
        assert_eq!(12, s.utf16_cu_to_char(13));
        assert_eq!(13, s.utf16_cu_to_char(14));

        assert_eq!(32, s.utf16_cu_to_char(33));
        assert_eq!(32, s.utf16_cu_to_char(34));
        assert_eq!(33, s.utf16_cu_to_char(35));

        assert_eq!(61, s.utf16_cu_to_char(63));
        assert_eq!(61, s.utf16_cu_to_char(64));
        assert_eq!(62, s.utf16_cu_to_char(65));

        assert_eq!(92, s.utf16_cu_to_char(95));
        assert_eq!(92, s.utf16_cu_to_char(96));
        assert_eq!(93, s.utf16_cu_to_char(97));

        assert_eq!(107, s.utf16_cu_to_char(111));
    }

    #[test]
    #[should_panic]
    fn utf16_cu_to_char_06() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);
        s.utf16_cu_to_char(112);
    }

    #[test]
    fn utf16_cu_to_char_07() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..106);

        assert_eq!(0, s.utf16_cu_to_char(0));

        assert_eq!(11, s.utf16_cu_to_char(11));
        assert_eq!(11, s.utf16_cu_to_char(12));
        assert_eq!(12, s.utf16_cu_to_char(13));

        assert_eq!(31, s.utf16_cu_to_char(32));
        assert_eq!(31, s.utf16_cu_to_char(33));
        assert_eq!(32, s.utf16_cu_to_char(34));

        assert_eq!(60, s.utf16_cu_to_char(62));
        assert_eq!(60, s.utf16_cu_to_char(63));
        assert_eq!(61, s.utf16_cu_to_char(64));

        assert_eq!(91, s.utf16_cu_to_char(94));
        assert_eq!(91, s.utf16_cu_to_char(95));
        assert_eq!(92, s.utf16_cu_to_char(96));

        assert_eq!(105, s.utf16_cu_to_char(109));
    }

    #[test]
    #[should_panic]
    fn utf16_cu_to_char_08() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..106);
        s.utf16_cu_to_char(110);
    }

    #[test]
    fn byte_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..100);

        assert_eq!(s.byte(0), b't');
        assert_eq!(s.byte(10), b' ');

        // UTF-8 encoding of 'な'.
        assert_eq!(s.byte(s.len_bytes() - 3), 0xE3);
        assert_eq!(s.byte(s.len_bytes() - 2), 0x81);
        assert_eq!(s.byte(s.len_bytes() - 1), 0xAA);
    }

    #[test]
    #[should_panic]
    fn byte_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..100);
        s.byte(s.len_bytes());
    }

    #[test]
    #[should_panic]
    fn byte_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(42..42);
        s.byte(0);
    }

    #[test]
    fn char_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..100);

        // t's \
        // a fine day, isn't it?  Aren't you glad \
        // we're alive?  こんにちは、みんな

        assert_eq!(s.char(0), 't');
        assert_eq!(s.char(10), ' ');
        assert_eq!(s.char(18), 'n');
        assert_eq!(s.char(65), 'な');
    }

    #[test]
    #[should_panic]
    fn char_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..100);
        s.char(66);
    }

    #[test]
    #[should_panic]
    fn char_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);
        s.char(0);
    }

    #[test]
    fn line_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);
        // "'s a fine day, isn't it?\nAren't you glad \
        //  we're alive?\nこんにちは、みん"

        let l0 = s.line(0);
        assert_eq!(l0, "'s a fine day, isn't it?\n");
        assert_eq!(l0.len_bytes(), 25);
        assert_eq!(l0.len_chars(), 25);
        assert_eq!(l0.len_lines(), 2);

        let l1 = s.line(1);
        assert_eq!(l1, "Aren't you glad we're alive?\n");
        assert_eq!(l1.len_bytes(), 29);
        assert_eq!(l1.len_chars(), 29);
        assert_eq!(l1.len_lines(), 2);

        let l2 = s.line(2);
        assert_eq!(l2, "こんにちは、みん");
        assert_eq!(l2.len_bytes(), 24);
        assert_eq!(l2.len_chars(), 8);
        assert_eq!(l2.len_lines(), 1);
    }

    #[test]
    fn line_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..59);
        // "'s a fine day, isn't it?\n"

        assert_eq!(s.line(0), "'s a fine day, isn't it?\n");
        assert_eq!(s.line(1), "");
    }

    #[test]
    fn line_03() {
        let r = Rope::from_str("Hi\nHi\nHi\nHi\nHi\nHi\n");
        let s = r.slice(1..17);

        assert_eq!(s.line(0), "i\n");
        assert_eq!(s.line(1), "Hi\n");
        assert_eq!(s.line(2), "Hi\n");
        assert_eq!(s.line(3), "Hi\n");
        assert_eq!(s.line(4), "Hi\n");
        assert_eq!(s.line(5), "Hi");
    }

    #[test]
    fn line_04() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);

        assert_eq!(s.line(0), "");
    }

    #[test]
    #[should_panic]
    fn line_05() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);
        s.line(3);
    }

    #[test]
    fn line_06() {
        let r = Rope::from_str("1\n2\n3\n4\n5\n6\n7\n8");
        let s = r.slice(1..11);
        // "\n2\n3\n4\n5\n6"

        assert_eq!(s.line(0).len_lines(), 2);
        assert_eq!(s.line(1).len_lines(), 2);
        assert_eq!(s.line(2).len_lines(), 2);
        assert_eq!(s.line(3).len_lines(), 2);
        assert_eq!(s.line(4).len_lines(), 2);
        assert_eq!(s.line(5).len_lines(), 1);
    }

    #[test]
    fn chunk_at_byte() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);
        let text = &TEXT_LINES[34..112];
        // "'s a fine day, isn't it?\nAren't you glad \
        //  we're alive?\nこんにちは、みん"

        let mut t = text;
        let mut prev_chunk = "";
        for i in 0..s.len_bytes() {
            let (chunk, b, c, l) = s.chunk_at_byte(i);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
            if chunk != prev_chunk {
                assert_eq!(chunk, &t[..chunk.len()]);
                t = &t[chunk.len()..];
                prev_chunk = chunk;
            }

            let c1 = {
                let i2 = byte_to_char_idx(text, i);
                text.chars().nth(i2).unwrap()
            };
            let c2 = {
                let i2 = i - b;
                let i3 = byte_to_char_idx(chunk, i2);
                chunk.chars().nth(i3).unwrap()
            };
            assert_eq!(c1, c2);
        }

        assert_eq!(t.len(), 0);
    }

    #[test]
    fn chunk_at_char() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);
        let text = &TEXT_LINES[34..112];
        // "'s a fine day, isn't it?\nAren't you glad \
        //  we're alive?\nこんにちは、みん"

        let mut t = text;
        let mut prev_chunk = "";
        for i in 0..s.len_chars() {
            let (chunk, b, c, l) = s.chunk_at_char(i);
            assert_eq!(b, char_to_byte_idx(text, c));
            assert_eq!(l, char_to_line_idx(text, c));
            if chunk != prev_chunk {
                assert_eq!(chunk, &t[..chunk.len()]);
                t = &t[chunk.len()..];
                prev_chunk = chunk;
            }

            let c1 = text.chars().nth(i).unwrap();
            let c2 = {
                let i2 = i - c;
                chunk.chars().nth(i2).unwrap()
            };
            assert_eq!(c1, c2);
        }
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn chunk_at_line_break() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);
        let text = &TEXT_LINES[34..112];
        // "'s a fine day, isn't it?\nAren't you glad \
        //  we're alive?\nこんにちは、みん"

        // First chunk
        {
            let (chunk, b, c, l) = s.chunk_at_line_break(0);
            assert_eq!(chunk, &text[..chunk.len()]);
            assert_eq!(b, 0);
            assert_eq!(c, 0);
            assert_eq!(l, 0);
        }

        // Middle chunks
        for i in 1..s.len_lines() {
            let (chunk, b, c, l) = s.chunk_at_line_break(i);
            assert_eq!(chunk, &text[b..(b + chunk.len())]);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
            assert!(l < i);
            assert!(i <= byte_to_line_idx(text, b + chunk.len()));
        }

        // Last chunk
        {
            let (chunk, b, c, l) = s.chunk_at_line_break(s.len_lines());
            assert_eq!(chunk, &text[(text.len() - chunk.len())..]);
            assert_eq!(chunk, &text[b..]);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
        }
    }

    #[test]
    fn slice_01() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(..);

        let s2 = s1.slice(..);

        assert_eq!(TEXT, s2);
    }

    #[test]
    fn slice_02() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(5..43);

        let s2 = s1.slice(3..25);

        assert_eq!(&TEXT[8..30], s2);
    }

    #[test]
    fn slice_03() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(31..97);

        let s2 = s1.slice(7..64);

        assert_eq!(&TEXT[38..103], s2);
    }

    #[test]
    fn slice_04() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(5..43);

        let s2 = s1.slice(21..21);

        assert!(s2.is_light());
        assert_eq!("", s2);
    }

    #[test]
    fn slice_05() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(5..98);
        for i in 0..(s1.len_chars() - 1) {
            let s2 = s1.slice(i..(i + 1));
            assert!(s2.is_light());
        }
    }

    #[test]
    #[should_panic]
    fn slice_06() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..43);

        #[allow(clippy::reversed_empty_ranges)]
        s.slice(21..20); // Wrong ordering on purpose.
    }

    #[test]
    #[should_panic]
    fn slice_07() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..43);

        s.slice(37..39);
    }

    #[test]
    fn byte_slice_01() {
        let r = Rope::from_str(TEXT);
        let s1 = r.byte_slice(..);

        let s2 = s1.byte_slice(..);

        assert_eq!(TEXT, s2);
    }

    #[test]
    fn byte_slice_02() {
        let r = Rope::from_str(TEXT);
        let s1 = r.byte_slice(50..118);

        let s2 = s1.byte_slice(3..25);

        assert_eq!(&TEXT[53..75], s2);
    }

    #[test]
    fn byte_slice_03() {
        let r = Rope::from_str(TEXT);
        let s1 = r.byte_slice(50..118);

        let s2 = s1.byte_slice(7..65);

        assert_eq!(&TEXT[57..115], s2);
    }

    #[test]
    fn byte_slice_04() {
        let r = Rope::from_str(TEXT);
        let s1 = r.byte_slice(50..118);

        let s2 = s1.byte_slice(21..21);

        assert!(s2.is_light());
        assert_eq!("", s2);
    }

    #[test]
    fn byte_slice_05() {
        let r = Rope::from_str(TEXT);
        let s1 = r.byte_slice(4..86);
        for i in 0..(s1.len_bytes() - 1) {
            let s2 = s1.byte_slice(i..(i + 1));
            assert!(s2.is_light());
        }
    }

    #[test]
    #[should_panic]
    fn byte_slice_06() {
        let r = Rope::from_str(TEXT);
        let s = r.byte_slice(50..118);

        #[allow(clippy::reversed_empty_ranges)]
        s.byte_slice(21..20); // Wrong ordering on purpose.
    }

    #[test]
    #[should_panic]
    fn byte_slice_07() {
        let r = Rope::from_str(TEXT);
        let s = r.byte_slice(50..85);

        s.byte_slice(35..36);
    }

    #[test]
    #[should_panic]
    fn byte_slice_08() {
        let r = Rope::from_str(TEXT);
        let s = r.byte_slice(50..118);

        // Not a char boundary.
        s.byte_slice(..43);
    }

    #[test]
    #[should_panic]
    fn byte_slice_09() {
        let r = Rope::from_str(TEXT);
        let s = r.byte_slice(50..118);

        // Not a char boundary.
        s.byte_slice(43..);
    }

    #[test]
    fn eq_str_01() {
        let r = Rope::from_str(TEXT);
        let slice = r.slice(..);

        assert_eq!(slice, TEXT);
        assert_eq!(TEXT, slice);
    }

    #[test]
    fn eq_str_02() {
        let r = Rope::from_str(TEXT);
        let slice = r.slice(0..20);

        assert_ne!(slice, TEXT);
        assert_ne!(TEXT, slice);
    }

    #[test]
    fn eq_str_03() {
        let mut r = Rope::from_str(TEXT);
        r.remove(20..21);
        r.insert(20, "z");
        let slice = r.slice(..);

        assert_ne!(slice, TEXT);
        assert_ne!(TEXT, slice);
    }

    #[test]
    fn eq_str_04() {
        let r = Rope::from_str(TEXT);
        let slice = r.slice(..);
        let s: String = TEXT.into();

        assert_eq!(slice, s);
        assert_eq!(s, slice);
    }

    #[test]
    fn eq_rope_slice_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);

        assert_eq!(s, s);
    }

    #[test]
    fn eq_rope_slice_02() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(43..97);
        let s2 = r.slice(43..97);

        assert_eq!(s1, s2);
    }

    #[test]
    fn eq_rope_slice_03() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(43..43);
        let s2 = r.slice(43..45);

        assert_ne!(s1, s2);
    }

    #[test]
    fn eq_rope_slice_04() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(43..45);
        let s2 = r.slice(43..43);

        assert_ne!(s1, s2);
    }

    #[test]
    fn eq_rope_slice_05() {
        let r = Rope::from_str("");
        let s = r.slice(0..0);

        assert_eq!(s, s);
    }

    #[test]
    fn cmp_rope_slice_01() {
        let r1 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
        let r2 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
        let s1 = r1.slice(..);
        let s2 = r2.slice(..);

        assert_eq!(s1.cmp(&s2), std::cmp::Ordering::Equal);
        assert_eq!(s1.slice(..24).cmp(&s2), std::cmp::Ordering::Less);
        assert_eq!(s1.cmp(&s2.slice(..24)), std::cmp::Ordering::Greater);
    }

    #[test]
    fn cmp_rope_slice_02() {
        let r1 = Rope::from_str("abcdefghijklmnzpqrstuvwxyz");
        let r2 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
        let s1 = r1.slice(..);
        let s2 = r2.slice(..);

        assert_eq!(s1.cmp(&s2), std::cmp::Ordering::Greater);
        assert_eq!(s2.cmp(&s1), std::cmp::Ordering::Less);
    }

    #[test]
    fn to_string_01() {
        let r = Rope::from_str(TEXT);
        let slc = r.slice(..);
        let s: String = slc.into();

        assert_eq!(r, s);
        assert_eq!(slc, s);
    }

    #[test]
    fn to_string_02() {
        let r = Rope::from_str(TEXT);
        let slc = r.slice(0..24);
        let s: String = slc.into();

        assert_eq!(slc, s);
    }

    #[test]
    fn to_string_03() {
        let r = Rope::from_str(TEXT);
        let slc = r.slice(13..89);
        let s: String = slc.into();

        assert_eq!(slc, s);
    }

    #[test]
    fn to_string_04() {
        let r = Rope::from_str(TEXT);
        let slc = r.slice(13..41);
        let s: String = slc.into();

        assert_eq!(slc, s);
    }

    #[test]
    fn to_cow_01() {
        use std::borrow::Cow;
        let r = Rope::from_str(TEXT);
        let s = r.slice(13..83);
        let cow: Cow<str> = s.into();

        assert_eq!(s, cow);
    }

    #[test]
    fn to_cow_02() {
        use std::borrow::Cow;
        let r = Rope::from_str(TEXT);
        let s = r.slice(13..14);
        let cow: Cow<str> = r.slice(13..14).into();

        // Make sure it's borrowed.
        if let Cow::Owned(_) = cow {
            panic!("Small Cow conversions should result in a borrow.");
        }

        assert_eq!(s, cow);
    }

    #[test]
    fn hash_01() {
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        let r = Rope::from_str("Hello there!");
        let s = r.slice(..);

        r.hash(&mut h1);
        s.hash(&mut h2);

        assert_eq!(h1.finish(), h2.finish());
    }

    // Iterator tests are in the iter module
}
