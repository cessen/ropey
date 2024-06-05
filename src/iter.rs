//! Iterators over a `Rope`'s data.
//!
//! The iterators in Ropey can be created from both `Rope`s and `RopeSlice`s.
//! When created from a `RopeSlice`, they iterate over only the data that the
//! `RopeSlice` refers to.  For the `Lines` and `Chunks` iterators, the data
//! of the first and last yielded item will be correctly truncated to match
//! the bounds of the `RopeSlice`.
//!
//! # Reverse iteration
//!
//! All iterators in Ropey can move both forwards and backwards over its
//! contents.  This can be accomplished via the `next()` and `prev()` methods on
//! each iterator, or by using the `reversed()` method to change the iterator's
//! direction.
//!
//! Conceptually, an iterator in Ropey is always positioned *on* the element it
//! most recently yielded, and returns an element when it jumps onto it via the
//! `next()` or `prev()` methods.
//!
//! For example, given the text `"abc"` and a `Chars` iterator starting at the
//! beginning of the text, you would get the following sequence of states and
//! return values by repeatedly calling `next()` (the vertical bar/hat represents
//! the position of the iterator):
//!
//! 0. `|abc`
//! 1. `âbc` -> `Some('a')`
//! 2. `ab̂c` -> `Some('b')`
//! 3. `abĉ` -> `Some('c')`
//! 4. `abc|` -> `None`
//!
//! The `prev()` method operates identically, except moving in the opposite
//! direction.  And `reverse()` simply swaps the behavior of `prev()` and
//! `next()`.
//!
//! # Creating iterators at any position
//!
//! Iterators in Ropey can be created starting at any position in the text.
//! This is accomplished with the various `bytes_at()`, `chars_at()`, etc.
//! methods of `Rope` and `RopeSlice`.
//!
//! When an iterator is created this way, it is positioned such that a call to
//! `next()` will return the specified element.  (One potentially weird effect
//! of the "on item" model of these iterators is that a `prev()` call will then
//! return the item *two items* before the specified element.)
//!
//! Importantly, iterators created this way still have access to the entire
//! contents of the `Rope`/`RopeSlice` they were created from&mdash;the
//! contents before the specified position is not truncated.  For example, you
//! can create a `Chars` iterator starting at the end of a `Rope`, and then
//! use the `prev()` method to iterate backwards over all of that `Rope`'s
//! chars.
//!
//! # A possible point of confusion
//!
//! The Rust standard library has an iterator trait `DoubleEndedIterator` with
//! a method `rev()`.  While the *name* is very similar to Ropey's `reversed()`
//! method, its behavior is very different.
//!
//! `DoubleEndedIterator` actually provides two iterators: one starting at each
//! end of the collection, moving in opposite directions towards each other.
//! Calling `rev()` switches between those two iterators, changing not only the
//! direction of iteration but also its current position in the collection.
//!
//! The `reversed()` method on Ropey's iterators, on the other hand, reverses
//! the direction of the iterator in-place, without changing its position in
//! the text.

use crate::{tree::Node, ChunkCursor};

//=============================================================

/// An iterator over a `Rope`'s contiguous `str` chunks.
///
/// Internally, each `Rope` stores text as a segemented collection of utf8
/// strings. This iterator iterates over those segments, returning a
/// `&str` slice for each one.  It is useful for situations such as:
///
/// - Writing a rope's utf8 text data to disk (but see
///   [`write_to()`](crate::Rope::write_to) for a convenience function that does
///   this for casual use cases).
/// - Streaming a rope's text data somewhere.
/// - Saving a rope to a non-utf8 encoding, doing the encoding conversion
///   incrementally as you go.
/// - Writing custom iterators over a rope's text data.
///
/// There are precisely two guarantee about the yielded chunks:
///
/// - All non-empty chunks are yielded.
/// - And they are yielded in order.
///
/// There are no guarantees about the size of yielded chunks, and except for
/// being valid `str` slices there are no guarantees about where the chunks are
/// split.  For example, they may be zero-sized, they don't necessarily align
/// with line breaks, they may split graphemes like CRLF, etc.
#[derive(Debug, Clone)]
pub struct Chunks<'a> {
    cursor: ChunkCursor<'a>,
    at_end: bool,
    is_reversed: bool,
}

impl<'a> Chunks<'a> {
    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&'a str> {
        if self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    /// Advances the iterator backward and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn prev(&mut self) -> Option<&'a str> {
        if self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    /// Reverses the direction of iteration.
    ///
    /// NOTE: this is distinct from the standard library's `rev()` method for
    /// `DoubleEndedIterator`.  Unlike that method, this reverses the direction
    /// of the iterator without changing its position in the stream.
    #[inline(always)]
    #[must_use]
    pub fn reversed(mut self) -> Chunks<'a> {
        self.is_reversed = !self.is_reversed;
        self
    }

    //---------------------------------------------------------

    /// Returns the Chunks iterator as well as the actual start byte of the
    /// first chunk, from the start of Node's contents.
    ///
    /// Note that all parameters are relative to the entire contents of `node`.
    /// In particular, `at_byte_idx` is NOT relative to `byte_range`, it is an
    /// offset from the start of the full contents of `node`.
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> (Chunks, usize) {
        let cursor = ChunkCursor::new(node, byte_range, at_byte_idx);
        let byte_offset = byte_range[0] + cursor.byte_offset();

        let chunks = Chunks {
            cursor: cursor,
            at_end: at_byte_idx == byte_range[1],
            is_reversed: false,
        };

        (chunks, byte_offset)
    }

    fn next_impl(&mut self) -> Option<&'a str> {
        loop {
            if self.at_end {
                return None;
            } else {
                let chunk = self.cursor.chunk();
                self.at_end = !self.cursor.next();

                if !chunk.is_empty() {
                    return Some(chunk);
                }
            }
        }
    }

    fn prev_impl(&mut self) -> Option<&'a str> {
        loop {
            if self.at_end {
                self.at_end = false;
            } else if !self.cursor.prev() {
                return None;
            }

            let chunk = self.cursor.chunk();
            if !chunk.is_empty() {
                return Some(chunk);
            }
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<&'a str> {
        Chunks::next(self)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // For the `Chunks` iterator we only provide a minimum, since we don't
        // have enough information to provide a guaranteed maximum.  The minimum
        // we provide is a conservative fudged approximation of the number of
        // chunks it would take to store all the bytes remaining in the iterator
        // if all the chunks were absolutely fully packed with data.

        use crate::tree::MAX_TEXT_SIZE;

        let byte_len = if self.is_reversed {
            if self.at_end {
                self.cursor.byte_offset() + self.cursor.chunk().len()
            } else {
                self.cursor.byte_offset()
            }
        } else {
            if self.at_end {
                0
            } else {
                self.cursor.byte_offset_from_end()
            }
        };

        let min = (byte_len + MAX_TEXT_SIZE - 1) / MAX_TEXT_SIZE;
        (min, None)
    }
}

//=============================================================

/// An iterator over a `Rope`'s bytes.
#[derive(Debug, Clone)]
pub struct Bytes<'a> {
    cursor: ChunkCursor<'a>,
    current_chunk: &'a [u8],
    chunk_byte_idx: usize, // Byte index of the start of the current chunk.
    byte_idx_in_chunk: usize,
    at_start_sentinel: bool,
    is_reversed: bool,
}

impl<'a> Bytes<'a> {
    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<u8> {
        if self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    /// Advances the iterator backward and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline]
    pub fn prev(&mut self) -> Option<u8> {
        if self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    /// Reverses the direction of iteration.
    ///
    /// NOTE: this is distinct from the standard library's `rev()` method for
    /// `DoubleEndedIterator`.  Unlike that method, this reverses the direction
    /// of the iterator without changing its position in the stream.
    #[inline(always)]
    #[must_use]
    pub fn reversed(mut self) -> Bytes<'a> {
        self.is_reversed = !self.is_reversed;
        self
    }

    //---------------------------------------------------------

    #[inline]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> Bytes {
        let cursor = ChunkCursor::new(node, byte_range, at_byte_idx);
        let chunk = cursor.chunk();
        let byte_offset = cursor.byte_offset();

        let mut bytes = Bytes {
            cursor: cursor,
            current_chunk: chunk.as_bytes(),
            chunk_byte_idx: byte_offset,
            byte_idx_in_chunk: at_byte_idx - byte_range[0] - byte_offset,
            at_start_sentinel: false,
            is_reversed: false,
        };

        // The above code puts us at the specified byte, but we want to be just
        // before it so that `next()` yields it.
        bytes.prev_impl();

        bytes
    }

    #[inline(always)]
    fn next_impl(&mut self) -> Option<u8> {
        if self.at_start_sentinel {
            self.at_start_sentinel = false;
        } else {
            self.byte_idx_in_chunk += 1;
        }

        while self.byte_idx_in_chunk >= self.current_chunk.len() {
            if self.cursor.next() {
                self.chunk_byte_idx += self.current_chunk.len();
                self.byte_idx_in_chunk -= self.current_chunk.len();
                self.current_chunk = self.cursor.chunk().as_bytes();
            } else {
                return None;
            }
        }

        let byte = self.current_chunk[self.byte_idx_in_chunk];
        Some(byte)
    }

    #[inline(always)]
    fn prev_impl(&mut self) -> Option<u8> {
        while self.byte_idx_in_chunk == 0 {
            if self.cursor.prev() {
                self.current_chunk = self.cursor.chunk().as_bytes();
                self.chunk_byte_idx -= self.current_chunk.len();
                self.byte_idx_in_chunk += self.current_chunk.len();
            } else {
                self.at_start_sentinel = true;
                return None;
            }
        }

        self.byte_idx_in_chunk -= 1;
        Some(self.current_chunk[self.byte_idx_in_chunk])
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<u8> {
        Bytes::next(self)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let byte_len = if self.is_reversed {
            self.cursor.byte_offset() + self.byte_idx_in_chunk
        } else {
            (self.cursor.byte_offset_from_end() - self.byte_idx_in_chunk
                + (self.at_start_sentinel as usize))
                .saturating_sub(1)
        };

        (byte_len, Some(byte_len))
    }
}

impl<'a> ExactSizeIterator for Bytes<'a> {}

//=============================================================

/// An iterator over a `Rope`'s `char`s.
#[derive(Debug, Clone)]
pub struct Chars<'a> {
    cursor: ChunkCursor<'a>,
    current_chunk: &'a str,
    chunk_byte_idx: usize, // Byte index of the start of the current chunk.
    byte_idx_in_chunk: usize,
    at_start_sentinel: bool,
    is_reversed: bool,
}

impl<'a> Chars<'a> {
    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<char> {
        if self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    /// Advances the iterator backward and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline]
    pub fn prev(&mut self) -> Option<char> {
        if self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    /// Reverses the direction of iteration.
    ///
    /// NOTE: this is distinct from the standard library's `rev()` method for
    /// `DoubleEndedIterator`.  Unlike that method, this reverses the direction
    /// of the iterator without changing its position in the stream.
    #[inline(always)]
    #[must_use]
    pub fn reversed(mut self) -> Chars<'a> {
        self.is_reversed = !self.is_reversed;
        self
    }

    //---------------------------------------------------------

    #[inline]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> Chars {
        let cursor = ChunkCursor::new(node, byte_range, at_byte_idx);
        let chunk = cursor.chunk();
        let byte_offset = cursor.byte_offset();

        assert!(chunk.is_char_boundary(at_byte_idx - byte_range[0] - byte_offset));

        let mut chars = Chars {
            cursor: cursor,
            current_chunk: chunk,
            chunk_byte_idx: byte_offset,
            byte_idx_in_chunk: at_byte_idx - byte_range[0] - byte_offset,
            at_start_sentinel: false,
            is_reversed: false,
        };

        // The above code puts us at the char of the specified byte, but we want
        // to be one char before it so that `next()` yields it.
        chars.prev_impl();

        chars
    }

    #[inline(always)]
    fn next_impl(&mut self) -> Option<char> {
        if self.at_start_sentinel {
            self.at_start_sentinel = false;
        } else {
            self.byte_idx_in_chunk = crate::ceil_char_boundary(
                self.byte_idx_in_chunk + 1,
                self.current_chunk.as_bytes(),
            );
        }

        while self.byte_idx_in_chunk >= self.current_chunk.len() {
            if self.cursor.next() {
                self.chunk_byte_idx += self.current_chunk.len();
                self.byte_idx_in_chunk -= self.current_chunk.len();
                self.current_chunk = self.cursor.chunk();
            } else {
                return None;
            }
        }

        // TODO: do this in a more efficient way than constructing a temporary
        // iterator.
        let char = self.current_chunk[self.byte_idx_in_chunk..]
            .chars()
            .next()
            .unwrap();
        Some(char)
    }

    #[inline(always)]
    fn prev_impl(&mut self) -> Option<char> {
        while self.byte_idx_in_chunk == 0 {
            if self.cursor.prev() {
                self.current_chunk = self.cursor.chunk();
                self.chunk_byte_idx -= self.current_chunk.len();
                self.byte_idx_in_chunk += self.current_chunk.len();
            } else {
                self.at_start_sentinel = true;
                return None;
            }
        }

        self.byte_idx_in_chunk =
            crate::floor_char_boundary(self.byte_idx_in_chunk - 1, self.current_chunk.as_bytes());

        // TODO: do this in a more efficient way than constructing a temporary
        // iterator.
        let char = self.current_chunk[self.byte_idx_in_chunk..]
            .chars()
            .next()
            .unwrap();
        Some(char)
    }
}

impl<'a> Iterator for Chars<'a> {
    type Item = char;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<char> {
        Chars::next(self)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // We give a min/max based on the smallest and largest possible code
        // points in UTF8.  Smallest is 1 byte, largest is 4 bytes.
        //
        // Note: if the `metric_chars` feature is enabled, we could go to the
        // trouble of computing the exact length in chars.  However, that would
        // involve some complications that probably aren't worth it.  And in any
        // case it would make this behave differently depending on that feature,
        // and this iterator isn't actually supposed to have anything to do with
        // that feature.

        let byte_len = if self.is_reversed {
            self.cursor.byte_offset() + self.byte_idx_in_chunk
        } else {
            // The use of 4 here is to be conservative, since that's the size of
            // the largest possible UTF8 code point.  We could instead be exact
            // and find the actual size of the next code point, but given that
            // this is just an estimate anyway it doesn't seem worth it.
            (self.cursor.byte_offset_from_end() - self.byte_idx_in_chunk
                + (self.at_start_sentinel as usize * 4))
                .saturating_sub(4)
        };

        let min = (byte_len + 3) / 4;
        let max = byte_len;
        (min, Some(max))
    }
}

//=============================================================

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
mod lines {
    use crate::{
        tree::{Node, TextInfo},
        LineType, RopeSlice,
    };

    /// An iterator over a `Rope`'s lines.
    ///
    /// Notes:
    /// - What the iterator considers to be a line depends on the line type it
    ///   was created with.
    /// - The returned lines include the line break at the end, if any.
    ///
    /// The last line is returned even if blank, in which case it
    /// is returned as an empty slice.
    #[derive(Debug, Clone)]
    pub struct Lines<'a> {
        node: &'a Node,
        node_info: &'a TextInfo,
        byte_range: [usize; 2],
        line_range: [usize; 2],
        line_type: LineType,
        current_line_idx: usize,
        at_start_sentinel: bool,
        is_reversed: bool,
    }

    impl<'a> Lines<'a> {
        /// Advances the iterator forward and returns the next value.
        ///
        /// Runs in amortized O(1) time and worst-case O(log N) time.
        #[inline(always)]
        #[allow(clippy::should_implement_trait)]
        pub fn next(&mut self) -> Option<RopeSlice<'a>> {
            if self.is_reversed {
                self.prev_impl()
            } else {
                self.next_impl()
            }
        }

        /// Advances the iterator backward and returns the previous value.
        ///
        /// Runs in amortized O(1) time and worst-case O(log N) time.
        #[inline(always)]
        pub fn prev(&mut self) -> Option<RopeSlice<'a>> {
            if self.is_reversed {
                self.next_impl()
            } else {
                self.prev_impl()
            }
        }

        /// Reverses the direction of iteration.
        ///
        /// NOTE: this is distinct from the standard library's `rev()` method for
        /// `DoubleEndedIterator`.  Unlike that method, this reverses the direction
        /// of the iterator without changing its position in the stream.
        #[inline(always)]
        #[must_use]
        pub fn reversed(mut self) -> Lines<'a> {
            self.is_reversed = !self.is_reversed;
            self
        }

        //-----------------------------------------------------

        /// Note: unlike the other iterator constructors, this one takes
        /// `at_line_idx` relative to the slice defined by `byte_range`, not
        /// relative to the whole contents of `node`.
        pub(crate) fn new(
            node: &'a Node,
            node_info: &'a TextInfo,
            byte_range: [usize; 2],
            at_line_idx: usize,
            line_type: LineType,
        ) -> Lines<'a> {
            let start_line = {
                let (text, info) = node.get_text_at_byte(byte_range[0]);
                info.line_breaks(line_type)
                    + text.byte_to_line(byte_range[0] - info.bytes, line_type)
            };
            let end_line = {
                let (text, info) = node.get_text_at_byte(byte_range[1]);
                info.line_breaks(line_type)
                    + text.byte_to_line(byte_range[1] - info.bytes, line_type)
                    + 1
            };

            assert!(start_line + at_line_idx <= end_line);

            Lines {
                node: node,
                node_info: node_info,
                byte_range: byte_range,
                line_range: [start_line, end_line],
                line_type: line_type,
                current_line_idx: start_line + at_line_idx.saturating_sub(1),
                at_start_sentinel: at_line_idx == 0,
                is_reversed: false,
            }
        }

        fn current_line(&self) -> Option<RopeSlice<'a>> {
            if self.at_start_sentinel || self.current_line_idx >= self.line_range[1] {
                return None;
            }

            let start_byte = {
                let (text, start_info) = self
                    .node
                    .get_text_at_line_break(self.current_line_idx, self.line_type);

                start_info.bytes
                    + text.line_to_byte(
                        self.current_line_idx - start_info.line_breaks(self.line_type),
                        self.line_type,
                    )
            };
            let end_byte = {
                let (text, start_info) = self
                    .node
                    .get_text_at_line_break(self.current_line_idx + 1, self.line_type);

                start_info.bytes
                    + text.line_to_byte(
                        self.current_line_idx + 1 - start_info.line_breaks(self.line_type),
                        self.line_type,
                    )
            };

            Some(RopeSlice::new(
                self.node,
                self.node_info,
                [
                    start_byte.max(self.byte_range[0]),
                    end_byte.min(self.byte_range[1]),
                ],
            ))
        }

        fn next_impl(&mut self) -> Option<RopeSlice<'a>> {
            if self.current_line_idx >= self.line_range[1] {
                return None;
            }

            if !self.at_start_sentinel {
                self.current_line_idx += 1;
            } else {
                self.at_start_sentinel = false;
            }

            self.current_line()
        }

        fn prev_impl(&mut self) -> Option<RopeSlice<'a>> {
            if self.current_line_idx <= self.line_range[0] {
                self.at_start_sentinel = true;
                return None;
            }

            self.current_line_idx -= 1;
            self.current_line()
        }
    }

    impl<'a> Iterator for Lines<'a> {
        type Item = RopeSlice<'a>;

        /// Advances the iterator forward and returns the next value.
        ///
        /// Runs in amortized O(1) time and worst-case O(log N) time.
        #[inline(always)]
        fn next(&mut self) -> Option<RopeSlice<'a>> {
            Lines::next(self)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            let len = if self.is_reversed {
                self.current_line_idx - self.line_range[0]
            } else {
                (self.line_range[1] - self.current_line_idx).saturating_sub(1)
                    + (self.at_start_sentinel as usize)
            };
            (len, Some(len))
        }
    }
}

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
pub use lines::Lines;

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{rope_builder::RopeBuilder, Rope};

    #[cfg(feature = "metric_lines_lf_cr")]
    use crate::LineType;

    // 127 bytes, 103 chars, 1 line
    const TEXT: &str = "Hello there!  How're you doing?  It's \
                        a fine day, isn't it?  Aren't you glad \
                        we're alive?  こんにちは、みんなさん！";

    #[cfg(feature = "metric_lines_lf_cr")]
    fn lines_text() -> String {
        let mut text = String::new();
        text.push_str("\r\n");
        for _ in 0..16 {
            text.push_str(
                "Hello there!  How're you doing?  It's a fine day, \
                 isn't it?  Aren't you glad we're alive?\r\n\
                 こんにちは！元気ですか？日はいいですね。\
                 私たちが生きだって嬉しいではないか？\r\n",
            );
        }
        text
    }

    fn hello_world_repeat_rope() -> Rope {
        let mut rb = RopeBuilder::new();
        for _ in 0..4 {
            rb._append_chunk_as_leaf("Hello ");
            rb._append_chunk_as_leaf("world!");
        }
        rb.finish()
    }

    /// Note: ensures that the chunks as given become individual leaf nodes in
    /// the rope.
    fn make_rope_from_chunks(chunks: &[&str]) -> Rope {
        let mut rb = RopeBuilder::new();
        for chunk in chunks {
            rb._append_chunk_as_leaf(chunk);
        }
        rb.finish()
    }

    #[test]
    fn chunks_iter_01() {
        let r = Rope::from_str(TEXT);

        let mut text = TEXT;
        let mut chunks = r.chunks();
        let mut stack = Vec::new();

        // Forward.
        while let Some(chunk) = chunks.next() {
            assert_eq!(&text[..chunk.len()], chunk);
            stack.push(chunk);
            text = &text[chunk.len()..];
        }
        assert_eq!("", text);

        // Backward.
        while let Some(chunk) = chunks.prev() {
            assert_eq!(stack.pop().unwrap(), chunk);
        }
        assert_eq!(0, stack.len());
    }

    #[test]
    fn chunks_iter_02() {
        let r = hello_world_repeat_rope();

        let mut chunks = r.chunks();

        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(None, chunks.next());

        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_03() {
        let r = Rope::from_str("");

        let mut chunks = r.chunks();
        assert_eq!(None, chunks.next());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_04() {
        let r = hello_world_repeat_rope();
        let s = r.slice(3..45);

        let mut chunks = s.chunks();

        assert_eq!(Some("lo "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("wor"), chunks.next());
        assert_eq!(None, chunks.next());

        assert_eq!(Some("wor"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("lo "), chunks.prev());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_05() {
        let r = hello_world_repeat_rope();
        let s = r.slice(8..40);

        let mut chunks = s.chunks();

        assert_eq!(Some("rld!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hell"), chunks.next());
        assert_eq!(None, chunks.next());

        assert_eq!(Some("Hell"), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("rld!"), chunks.prev());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_06() {
        let r = hello_world_repeat_rope();
        let s = r.slice(14..14);

        let mut chunks = s.chunks();
        assert_eq!(None, chunks.next());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_07() {
        let r = Rope::from_str("A");
        let mut chunks = r.chunks();

        assert_eq!(Some("A"), chunks.next());
        assert_eq!(None, chunks.next());
        assert_eq!(Some("A"), chunks.prev());
        assert_eq!(None, chunks.prev());

        assert_eq!(Some("A"), chunks.next());
        assert_eq!(Some("A"), chunks.prev());
        assert_eq!(Some("A"), chunks.next());
    }

    #[test]
    fn chunks_iter_08() {
        let r =
            make_rope_from_chunks(&["ABC", "DEF", "GHI", "JKL", "MNO", "PQR", "STU", "VWX", "YZ"]);
        let mut chunks = r.chunks();

        assert_eq!(Some("ABC"), chunks.next());
        assert_eq!(Some("ABC"), chunks.prev());
        assert_eq!(None, chunks.prev());

        assert_eq!(Some("ABC"), chunks.next());
        assert_eq!(Some("DEF"), chunks.next());
        assert_eq!(Some("DEF"), chunks.prev());

        assert_eq!(Some("DEF"), chunks.next());
        assert_eq!(Some("GHI"), chunks.next());
        assert_eq!(Some("JKL"), chunks.next());
        assert_eq!(Some("JKL"), chunks.prev());

        assert_eq!(Some("JKL"), chunks.next());
        assert_eq!(Some("MNO"), chunks.next());
        assert_eq!(Some("PQR"), chunks.next());
        assert_eq!(Some("STU"), chunks.next());
        assert_eq!(Some("VWX"), chunks.next());
        assert_eq!(Some("VWX"), chunks.prev());

        assert_eq!(Some("VWX"), chunks.next());
        assert_eq!(Some("YZ"), chunks.next());
        assert_eq!(None, chunks.next());
        assert_eq!(Some("YZ"), chunks.prev());

        assert_eq!(Some("YZ"), chunks.next());
        assert_eq!(None, chunks.next());
    }

    #[test]
    fn chunks_at_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..TEXT.len() {
            let mut current_byte = r.chunk(i).1;

            for chunk1 in r.chunks_at(i) {
                let chunk2 = r.chunk(current_byte).0;
                assert_eq!(chunk2, chunk1);
                current_byte += chunk2.len();
            }
        }

        let mut chunks = r.chunks_at(TEXT.len());
        assert_eq!(None, chunks.next());
    }

    #[test]
    fn chunks_at_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let text = &TEXT[5..124];

        for i in 0..text.len() {
            let mut current_byte = s.chunk(i).1;

            for chunk1 in s.chunks_at(i) {
                let chunk2 = s.chunk(current_byte).0;
                assert_eq!(chunk2, chunk1);
                current_byte += chunk2.len();
            }
        }

        let mut chunks = s.chunks_at(text.len());
        assert_eq!(None, chunks.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_iter_size_hint_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);

        let mut chunks = s.chunks();

        // Forward.
        assert!(chunks.clone().count() >= chunks.size_hint().0);
        while let Some(_) = chunks.next() {
            assert!(chunks.clone().count() >= chunks.size_hint().0);
        }
        assert_eq!(0, chunks.size_hint().0);

        // Backward.
        chunks = chunks.reversed();
        assert!(chunks.clone().count() >= chunks.size_hint().0);
        while let Some(_) = chunks.next() {
            assert!(chunks.clone().count() >= chunks.size_hint().0);
        }
        assert_eq!(0, chunks.size_hint().0);
    }

    // NOTE: when you add support for starting iterators at specific indices,
    // ensure that the Bytes iterator can be created with a starting index that
    // splits a char.

    fn test_bytes_against_text(mut bytes: Bytes, text: &str) {
        // Forward.
        let mut iter_f = text.bytes();
        loop {
            let b1 = bytes.next();
            let b2 = iter_f.next();

            assert_eq!(b1, b2);

            if b1.is_none() && b2.is_none() {
                break;
            }
        }

        // Backward.
        let mut iter_b = text.bytes().rev();
        loop {
            let b1 = bytes.prev();
            let b2 = iter_b.next();

            assert_eq!(b1, b2);

            if b1.is_none() && b2.is_none() {
                break;
            }
        }
    }

    #[test]
    fn bytes_iter_01() {
        let r = Rope::from_str(TEXT);
        let iter = r.bytes();

        test_bytes_against_text(iter, TEXT);
    }

    #[test]
    fn bytes_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let iter = s.bytes();

        test_bytes_against_text(iter, &TEXT[5..124]);
    }

    #[test]
    fn bytes_iter_03() {
        let text = "abc";
        let r = Rope::from_str(text);
        let text = text.as_bytes();

        let mut bytes = r.bytes();

        assert_eq!(Some(text[0]), bytes.next());
        assert_eq!(None, bytes.prev());

        assert_eq!(Some(text[0]), bytes.next());
        assert_eq!(Some(text[1]), bytes.next());
        assert_eq!(Some(text[0]), bytes.prev());

        assert_eq!(Some(text[1]), bytes.next());
        assert_eq!(Some(text[2]), bytes.next());
        assert_eq!(Some(text[1]), bytes.prev());

        assert_eq!(Some(text[2]), bytes.next());
        assert_eq!(None, bytes.next());
        assert_eq!(Some(text[2]), bytes.prev());
    }

    #[test]
    fn bytes_iter_04() {
        let r = Rope::from_str("");

        assert_eq!(None, r.bytes().next());
        assert_eq!(None, r.bytes().prev());
    }

    #[test]
    fn bytes_at_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..TEXT.len() {
            let mut bytes = r.bytes_at(i);
            assert_eq!(TEXT.as_bytes()[i], bytes.next().unwrap());
        }

        let mut bytes = r.bytes_at(TEXT.len());
        assert_eq!(None, bytes.next());
    }

    #[test]
    fn bytes_at_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let text = &TEXT[5..124];

        for i in 0..text.len() {
            let mut bytes = s.bytes_at(i);
            assert_eq!(text.as_bytes()[i], bytes.next().unwrap());
        }

        let mut bytes = s.bytes_at(text.len());
        assert_eq!(None, bytes.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_iter_size_hint_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);

        let mut bytes = s.bytes();

        // Forward.
        assert_eq!(bytes.clone().count(), bytes.size_hint().0);
        while let Some(_) = bytes.next() {
            assert_eq!(bytes.clone().count(), bytes.size_hint().0);
        }
        assert_eq!(0, bytes.size_hint().0);

        // Backward.
        bytes = bytes.reversed();
        assert_eq!(bytes.clone().count(), bytes.size_hint().0);
        while let Some(_) = bytes.next() {
            assert_eq!(bytes.clone().count(), bytes.size_hint().0);
        }
        assert_eq!(0, bytes.size_hint().0);
    }

    fn test_chars_against_text(mut chars: Chars, text: &str) {
        // Forward.
        let mut iter_f = text.chars();
        loop {
            let c1 = chars.next();
            let c2 = iter_f.next();

            assert_eq!(c1, c2);

            if c1.is_none() && c2.is_none() {
                break;
            }
        }

        // Backward.
        let mut iter_b = text.chars().rev();
        loop {
            let c1 = chars.prev();
            let c2 = iter_b.next();

            assert_eq!(c1, c2);

            if c1.is_none() && c2.is_none() {
                break;
            }
        }
    }

    #[test]
    fn chars_iter_01() {
        let r = Rope::from_str(TEXT);
        let iter = r.chars();

        test_chars_against_text(iter, TEXT);
    }

    #[test]
    fn chars_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let iter = s.chars();

        test_chars_against_text(iter, &TEXT[5..124]);
    }

    #[test]
    fn chars_iter_03() {
        let text = "abc";
        let r = Rope::from_str(text);

        let mut chars = r.chars();

        assert_eq!(Some('a'), chars.next());
        assert_eq!(None, chars.prev());

        assert_eq!(Some('a'), chars.next());
        assert_eq!(Some('b'), chars.next());
        assert_eq!(Some('a'), chars.prev());

        assert_eq!(Some('b'), chars.next());
        assert_eq!(Some('c'), chars.next());
        assert_eq!(Some('b'), chars.prev());

        assert_eq!(Some('c'), chars.next());
        assert_eq!(None, chars.next());
        assert_eq!(Some('c'), chars.prev());
    }

    #[test]
    fn chars_iter_04() {
        let r = Rope::from_str("");

        assert_eq!(None, r.chars().next());
        assert_eq!(None, r.chars().prev());
    }

    #[test]
    fn chars_at_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..TEXT.len() {
            if !TEXT.is_char_boundary(i) {
                continue;
            }
            let mut chars = r.chars_at(i);
            assert_eq!(TEXT[i..].chars().next(), chars.next());
        }

        let mut chars = r.chars_at(TEXT.len());
        assert_eq!(None, chars.next());
    }

    #[test]
    fn chars_at_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let text = &TEXT[5..124];

        for i in 0..text.len() {
            if !text.is_char_boundary(i) {
                continue;
            }
            let mut chars = s.chars_at(i);
            assert_eq!(text[i..].chars().next(), chars.next());
        }

        let mut chars = s.chars_at(text.len());
        assert_eq!(None, chars.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_iter_size_hint_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);

        let mut chars = s.chars();

        // Forward.
        assert!(chars.clone().count() >= chars.size_hint().0);
        assert!(chars.clone().count() <= chars.size_hint().1.unwrap());
        while let Some(_) = chars.next() {
            assert!(chars.clone().count() >= chars.size_hint().0);
            assert!(chars.clone().count() <= chars.size_hint().1.unwrap());
        }
        assert_eq!(0, chars.size_hint().0);
        assert_eq!(0, chars.size_hint().1.unwrap());

        // Backward.
        chars = chars.reversed();
        assert!(chars.clone().count() >= chars.size_hint().0);
        assert!(chars.clone().count() <= chars.size_hint().1.unwrap());
        while let Some(_) = chars.next() {
            assert!(chars.clone().count() >= chars.size_hint().0);
            assert!(chars.clone().count() <= chars.size_hint().1.unwrap());
        }
        assert_eq!(0, chars.size_hint().0);
        assert_eq!(0, chars.size_hint().1.unwrap());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_01() {
        let r = Rope::from_str("hi\nyo\nbye");

        let mut lines = r.lines(LineType::LF_CR);

        assert_eq!("hi\n", lines.next().unwrap());
        assert_eq!(None, lines.prev());

        assert_eq!("hi\n", lines.next().unwrap());
        assert_eq!("yo\n", lines.next().unwrap());
        assert_eq!("hi\n", lines.prev().unwrap());

        assert_eq!("yo\n", lines.next().unwrap());
        assert_eq!("bye", lines.next().unwrap());
        assert_eq!(None, lines.next());
        assert_eq!("bye", lines.prev().unwrap());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_02() {
        let text = "Hello there!\nHow goes it?";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines(LineType::LF_CR).count());
        assert_eq!(2, s.lines(LineType::LF_CR).count());

        let mut lines = r.lines(LineType::LF_CR);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::LF_CR);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_03() {
        let text = "Hello there!\nHow goes it?\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(3, r.lines(LineType::LF_CR).count());
        assert_eq!(3, s.lines(LineType::LF_CR).count());

        let mut lines = r.lines(LineType::LF_CR);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::LF_CR);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_04() {
        let text = "Hello there!\nHow goes it?\nYeah!";
        let r = Rope::from_str(text);
        let s1 = r.slice(..25);
        let s2 = r.slice(..26);

        assert_eq!(2, s1.lines(LineType::LF_CR).count());
        assert_eq!(3, s2.lines(LineType::LF_CR).count());

        let mut lines = s1.lines(LineType::LF_CR);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s2.lines(LineType::LF_CR);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_05() {
        let text = "";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(1, r.lines(LineType::LF_CR).count());
        assert_eq!(1, s.lines(LineType::LF_CR).count());

        let mut lines = r.lines(LineType::LF_CR);
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::LF_CR);
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_06() {
        let text = "a";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(1, r.lines(LineType::LF_CR).count());
        assert_eq!(1, s.lines(LineType::LF_CR).count());

        let mut lines = r.lines(LineType::LF_CR);
        assert_eq!("a", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::LF_CR);
        assert_eq!("a", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_07() {
        let text = "a\nb";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines(LineType::LF_CR).count());
        assert_eq!(2, s.lines(LineType::LF_CR).count());

        let mut lines = r.lines(LineType::LF_CR);
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::LF_CR);
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_08() {
        let text = "\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines(LineType::LF_CR).count());
        assert_eq!(2, s.lines(LineType::LF_CR).count());

        let mut lines = r.lines(LineType::LF_CR);
        assert_eq!("\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::LF_CR);
        assert_eq!("\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_09() {
        let text = "a\nb\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(3, r.lines(LineType::LF_CR).count());
        assert_eq!(3, s.lines(LineType::LF_CR).count());

        let mut lines = r.lines(LineType::LF_CR);
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::LF_CR);
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_10() {
        let text = lines_text();
        let r = Rope::from_str(&text);

        let mut itr = r.lines(LineType::LF_CR);

        assert_eq!(None, itr.prev());
        assert_eq!(None, itr.prev());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_11() {
        let text = lines_text();
        let r = Rope::from_str(&text);

        let mut lines = Vec::new();
        let mut itr = r.lines(LineType::LF_CR);

        while let Some(line) = itr.next() {
            lines.push(line);
        }

        while let Some(line) = itr.prev() {
            assert_eq!(line, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_12() {
        let text = lines_text();
        let r = Rope::from_str(&text);
        let s = r.slice(34..2031);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::LF_CR);

        while let Some(line) = itr.next() {
            lines.push(line);
        }

        while let Some(line) = itr.prev() {
            assert_eq!(line, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_13() {
        let text = "";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::LF_CR);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_14() {
        let text = "a";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::LF_CR);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_15() {
        let text = "a\nb";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::LF_CR);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_16() {
        let text = "\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::LF_CR);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_17() {
        let text = "a\nb\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::LF_CR);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_18() {
        let text = lines_text();
        let r = Rope::from_str(&text);
        let s = r.slice(..);

        assert_eq!(34, r.lines(LineType::LF_CR).count());
        assert_eq!(34, s.lines(LineType::LF_CR).count());

        // Rope
        let mut lines = r.lines(LineType::LF_CR);
        assert_eq!("\r\n", lines.next().unwrap());
        for _ in 0..16 {
            assert_eq!(
                "Hello there!  How're you doing?  It's a fine day, \
                 isn't it?  Aren't you glad we're alive?\r\n",
                lines.next().unwrap()
            );
            assert_eq!(
                "こんにちは！元気ですか？日はいいですね。\
                 私たちが生きだって嬉しいではないか？\r\n",
                lines.next().unwrap()
            );
        }
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        // Slice
        let mut lines = s.lines(LineType::LF_CR);
        assert_eq!("\r\n", lines.next().unwrap());
        for _ in 0..16 {
            assert_eq!(
                "Hello there!  How're you doing?  It's a fine day, \
                 isn't it?  Aren't you glad we're alive?\r\n",
                lines.next().unwrap()
            );
            assert_eq!(
                "こんにちは！元気ですか？日はいいですね。\
                 私たちが生きだって嬉しいではないか？\r\n",
                lines.next().unwrap()
            );
        }
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_19() {
    //     let r = Rope::from_str("a\nb\nc\nd\ne\nf\ng\nh\n");
    //     for (line, c) in r.lines(LineType::LF_CR).zip('a'..='h') {
    //         assert_eq!(line, format!("{c}\n"))
    //     }
    //     for (line, c) in r
    //         .lines_at(r.len_lines() - 1)
    //         .reversed()
    //         .zip(('a'..='h').rev())
    //     {
    //         assert_eq!(line, format!("{c}\n"))
    //     }

    //     let r = Rope::from_str("ab\nc\nd\ne\nf\ng\nh\n");
    //     for (line, c) in r.slice(1..).lines(LineType::LF_CR).zip('b'..='h') {
    //         assert_eq!(line, format!("{c}\n"))
    //     }
    // }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_01() {
        let text = lines_text();
        let r = Rope::from_str(&text);

        for i in 0..r.len_lines(LineType::LF_CR) {
            let line = r.line(i, LineType::LF_CR);
            let mut lines = r.lines_at(i, LineType::LF_CR);
            assert_eq!(Some(line), lines.next());
        }

        let mut lines = r.lines_at(r.len_lines(LineType::LF_CR), LineType::LF_CR);
        assert_eq!(None, lines.next());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_02() {
        let text = lines_text();
        let r = Rope::from_str(&text);
        let s = r.slice(34..2031);

        for i in 0..s.len_lines(LineType::LF_CR) {
            let line = s.line(i, LineType::LF_CR);
            let mut lines = s.lines_at(i, LineType::LF_CR);
            assert_eq!(Some(line), lines.next());
        }

        let mut lines = s.lines_at(s.len_lines(LineType::LF_CR), LineType::LF_CR);
        assert_eq!(None, lines.next());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_03() {
        let text = lines_text();
        let r = Rope::from_str(&text);
        let s = r.slice(34..34);

        let mut lines = s.lines_at(0, LineType::LF_CR);
        assert_eq!("", lines.next().unwrap());

        let mut lines = s.lines_at(1, LineType::LF_CR);
        assert_eq!(None, lines.next());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_iter_size_hint_01() {
        let text = lines_text();
        let r = Rope::from_str(&text);
        let s = r.slice(34..2031);

        let mut lines = s.lines(LineType::LF_CR);
        let mut line_count = lines.clone().count();

        // Forward.
        assert_eq!(line_count, lines.size_hint().0);
        while let Some(_) = lines.next() {
            line_count -= 1;
            assert_eq!(line_count, lines.size_hint().0);
        }
        assert_eq!(line_count, 0);
        assert_eq!(line_count, lines.size_hint().0);

        // Backward.
        lines = lines.reversed();
        line_count = lines.clone().count();
        assert_eq!(line_count, lines.size_hint().0);
        while let Some(_) = lines.next() {
            line_count -= 1;
            assert_eq!(line_count, lines.size_hint().0);
        }
        assert_eq!(line_count, 0);
        assert_eq!(line_count, lines.size_hint().0);
    }
}
