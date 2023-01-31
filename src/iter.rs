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
//! All iterators in Ropey operate as a cursor that can move both forwards
//! and backwards over its contents.  This can be accomplished via the
//! `next()` and `prev()` methods on each iterator, or by using the `reverse()`
//! or `reversed()` methods to change the iterator's direction.
//!
//! Conceptually, an iterator in Ropey is always positioned *between* the
//! elements it iterates over, and returns an element when it jumps over it
//! via the `next()` or `prev()` methods.
//!
//! For example, given the text `"abc"` and a `Chars` iterator starting at the
//! beginning of the text, you would get the following sequence of states and
//! return values by repeatedly calling `next()` (the vertical bar represents
//! the position of the iterator):
//!
//! 0. `|abc`
//! 1. `a|bc` -> `Some('a')`
//! 2. `ab|c` -> `Some('b')`
//! 3. `abc|` -> `Some('c')`
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
//! `next()` will return the specified element, and a call to `prev()` will
//! return the element just before the specified one.
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
//! a method `rev()`.  While this method's name is very similar to Ropey's
//! `reverse()` method, its behavior is very different.
//!
//! `DoubleEndedIterator` actually provides two iterators: one starting at each
//! end of the collection, moving in opposite directions towards each other.
//! Calling `rev()` switches between those two iterators, changing not only the
//! direction of iteration but also its current position in the collection.
//!
//! The `reverse()` method on Ropey's iterators, on the other hand, reverses
//! the direction of the iterator in-place, without changing its position in
//! the text.

use std::str;
use std::sync::Arc;

use crate::slice::{RSEnum, RopeSlice};
use crate::str_utils::{
    byte_to_line_idx, char_to_byte_idx, count_chars, count_utf16_surrogates, ends_with_line_break,
    last_line_start_byte_idx, line_to_byte_idx, trim_line_break,
};
use crate::tree::{Count, Node, TextInfo};

//==========================================================

/// An iterator over a `Rope`'s bytes.
#[derive(Debug, Clone)]
pub struct Bytes<'a> {
    chunk_iter: Chunks<'a>,
    cur_chunk: &'a [u8],
    byte_idx: usize,
    last_call_was_prev_impl: bool,
    bytes_total: usize,
    bytes_remaining: usize,
    is_reversed: bool,
}

impl<'a> Bytes<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Bytes {
        let mut chunk_iter = Chunks::new(node);
        let cur_chunk = if let Some(chunk) = chunk_iter.next() {
            chunk
        } else {
            ""
        };
        Bytes {
            chunk_iter: chunk_iter,
            cur_chunk: cur_chunk.as_bytes(),
            byte_idx: 0,
            last_call_was_prev_impl: false,
            bytes_total: node.text_info().bytes as usize,
            bytes_remaining: node.text_info().bytes as usize,
            is_reversed: false,
        }
    }

    #[inline(always)]
    pub(crate) fn new_with_range(
        node: &Arc<Node>,
        byte_idx_range: (usize, usize),
        char_idx_range: (usize, usize),
        line_break_idx_range: (usize, usize),
    ) -> Bytes {
        Bytes::new_with_range_at(
            node,
            byte_idx_range.0,
            byte_idx_range,
            char_idx_range,
            line_break_idx_range,
        )
    }

    pub(crate) fn new_with_range_at(
        node: &Arc<Node>,
        at_byte: usize,
        byte_idx_range: (usize, usize),
        char_idx_range: (usize, usize),
        line_break_idx_range: (usize, usize),
    ) -> Bytes {
        let (mut chunk_iter, mut chunk_byte_start, _, _) = Chunks::new_with_range_at_byte(
            node,
            at_byte,
            byte_idx_range,
            char_idx_range,
            line_break_idx_range,
        );

        let cur_chunk = if byte_idx_range.0 == byte_idx_range.1 {
            ""
        } else if at_byte < byte_idx_range.1 {
            chunk_iter.next().unwrap()
        } else {
            let chunk = chunk_iter.prev().unwrap();
            chunk_iter.next();
            chunk_byte_start -= chunk.len();
            chunk
        };

        Bytes {
            chunk_iter: chunk_iter,
            cur_chunk: cur_chunk.as_bytes(),
            byte_idx: at_byte - chunk_byte_start,
            last_call_was_prev_impl: false,
            bytes_total: byte_idx_range.1 - byte_idx_range.0,
            bytes_remaining: byte_idx_range.1 - at_byte,
            is_reversed: false,
        }
    }

    #[inline(always)]
    pub(crate) fn from_str(text: &str) -> Bytes {
        Bytes::from_str_at(text, 0)
    }

    pub(crate) fn from_str_at(text: &str, byte_idx: usize) -> Bytes {
        let mut chunk_iter = Chunks::from_str(text, false);
        let cur_chunk = if let Some(chunk) = chunk_iter.next() {
            chunk
        } else {
            ""
        };
        Bytes {
            chunk_iter: chunk_iter,
            cur_chunk: cur_chunk.as_bytes(),
            byte_idx: byte_idx,
            last_call_was_prev_impl: false,
            bytes_total: text.len(),
            bytes_remaining: text.len() - byte_idx,
            is_reversed: false,
        }
    }

    /// Reverses the direction of the iterator in-place.
    ///
    /// In other words, swaps the behavior of [`prev()`](Bytes::prev())
    /// and [`next()`](Bytes::next()).
    #[inline]
    pub fn reverse(&mut self) {
        self.is_reversed = !self.is_reversed;
    }

    /// Same as `reverse()`, but returns itself.
    ///
    /// This is useful when chaining iterator methods:
    ///
    /// ```rust
    /// # use ropey::Rope;
    /// # let rope = Rope::from_str("Hello there\n world!\n");
    /// // Enumerate the rope's bytes in reverse, starting from the end.
    /// for (i, b) in rope.bytes_at(rope.len_bytes()).reversed().enumerate() {
    ///     println!("{} {}", i, b);
    /// #   assert_eq!(b, rope.byte(rope.len_bytes() - i - 1));
    /// }
    #[inline]
    #[must_use]
    pub fn reversed(mut self) -> Bytes<'a> {
        self.reverse();
        self
    }

    /// Advances the iterator backwards and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn prev(&mut self) -> Option<u8> {
        if !self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    #[inline]
    fn prev_impl(&mut self) -> Option<u8> {
        // Put us back into a "prev" progression.
        if !self.last_call_was_prev_impl {
            self.chunk_iter.prev();
            self.last_call_was_prev_impl = true;
        }

        // Progress the chunks iterator back if needed.
        if self.byte_idx == 0 {
            if let Some(chunk) = self.chunk_iter.prev() {
                self.cur_chunk = chunk.as_bytes();
                self.byte_idx = self.cur_chunk.len();
            } else {
                return None;
            }
        }

        // Progress the byte counts and return the previous byte.
        self.byte_idx -= 1;
        self.bytes_remaining += 1;
        return Some(self.cur_chunk[self.byte_idx]);
    }

    #[inline]
    fn next_impl(&mut self) -> Option<u8> {
        // Put us back into a "next" progression.
        if self.last_call_was_prev_impl {
            self.chunk_iter.next();
            self.last_call_was_prev_impl = false;
        }

        // Progress the chunks iterator forward if needed.
        if self.byte_idx >= self.cur_chunk.len() {
            if let Some(chunk) = self.chunk_iter.next() {
                self.cur_chunk = chunk.as_bytes();
                self.byte_idx = 0;
            } else {
                return None;
            }
        }

        // Progress the byte counts and return the next byte.
        let byte = self.cur_chunk[self.byte_idx];
        self.byte_idx += 1;
        self.bytes_remaining -= 1;
        return Some(byte);
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<u8> {
        if !self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = if !self.is_reversed {
            self.bytes_remaining
        } else {
            self.bytes_total - self.bytes_remaining
        };
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for Bytes<'a> {}

//==========================================================

/// An iterator over a `Rope`'s chars.
#[derive(Debug, Clone)]
pub struct Chars<'a> {
    chunk_iter: Chunks<'a>,
    cur_chunk: &'a str,
    byte_idx: usize,
    last_call_was_prev_impl: bool,
    chars_total: usize,
    chars_remaining: usize,
    is_reversed: bool,
}

impl<'a> Chars<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Chars {
        let mut chunk_iter = Chunks::new(node);
        let cur_chunk = if let Some(chunk) = chunk_iter.next() {
            chunk
        } else {
            ""
        };
        Chars {
            chunk_iter: chunk_iter,
            cur_chunk: cur_chunk,
            byte_idx: 0,
            last_call_was_prev_impl: false,
            chars_total: node.text_info().chars as usize,
            chars_remaining: node.text_info().chars as usize,
            is_reversed: false,
        }
    }

    #[inline(always)]
    pub(crate) fn new_with_range(
        node: &Arc<Node>,
        byte_idx_range: (usize, usize),
        char_idx_range: (usize, usize),
        line_break_idx_range: (usize, usize),
    ) -> Chars {
        Chars::new_with_range_at(
            node,
            char_idx_range.0,
            byte_idx_range,
            char_idx_range,
            line_break_idx_range,
        )
    }

    pub(crate) fn new_with_range_at(
        node: &Arc<Node>,
        at_char: usize,
        byte_idx_range: (usize, usize),
        char_idx_range: (usize, usize),
        line_break_idx_range: (usize, usize),
    ) -> Chars {
        let (mut chunk_iter, _, mut chunk_char_start, _) = Chunks::new_with_range_at_char(
            node,
            at_char,
            byte_idx_range,
            char_idx_range,
            line_break_idx_range,
        );

        let cur_chunk = if char_idx_range.0 == char_idx_range.1 {
            ""
        } else if at_char < char_idx_range.1 {
            chunk_iter.next().unwrap()
        } else {
            let chunk = chunk_iter.prev().unwrap();
            chunk_iter.next();
            chunk_char_start =
                (node.get_chunk_at_char(at_char - 1).1.chars as usize).max(char_idx_range.0);
            chunk
        };

        Chars {
            chunk_iter: chunk_iter,
            cur_chunk: cur_chunk,
            byte_idx: char_to_byte_idx(cur_chunk, at_char - chunk_char_start),
            last_call_was_prev_impl: false,
            chars_total: char_idx_range.1 - char_idx_range.0,
            chars_remaining: char_idx_range.1 - at_char,
            is_reversed: false,
        }
    }

    #[inline(always)]
    pub(crate) fn from_str(text: &str) -> Chars {
        Chars::from_str_at(text, 0)
    }

    pub(crate) fn from_str_at(text: &str, char_idx: usize) -> Chars {
        let mut chunk_iter = Chunks::from_str(text, false);
        let cur_chunk = if let Some(chunk) = chunk_iter.next() {
            chunk
        } else {
            ""
        };
        let start_byte_idx = char_to_byte_idx(text, char_idx);
        let chars_remaining = count_chars(&text[start_byte_idx..]);

        Chars {
            chunk_iter: chunk_iter,
            cur_chunk: cur_chunk,
            byte_idx: start_byte_idx,
            last_call_was_prev_impl: false,
            chars_total: chars_remaining + count_chars(&text[..start_byte_idx]),
            chars_remaining: chars_remaining,
            is_reversed: false,
        }
    }

    /// Reverses the direction of the iterator in-place.
    ///
    /// In other words, swaps the behavior of [`prev()`](Chars::prev())
    /// and [`next()`](Chars::next()).
    #[inline]
    pub fn reverse(&mut self) {
        self.is_reversed = !self.is_reversed;
    }

    /// Same as `reverse()`, but returns itself.
    ///
    /// This is useful when chaining iterator methods:
    ///
    /// ```rust
    /// # use ropey::Rope;
    /// # let rope = Rope::from_str("Hello there\n world!\n");
    /// // Enumerate the rope's chars in reverse, starting from the end.
    /// for (i, ch) in rope.chars_at(rope.len_chars()).reversed().enumerate() {
    ///     println!("{} {}", i, ch);
    /// #   assert_eq!(ch, rope.char(rope.len_chars() - i - 1));
    /// }
    #[inline]
    #[must_use]
    pub fn reversed(mut self) -> Chars<'a> {
        self.reverse();
        self
    }

    /// Advances the iterator backwards and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn prev(&mut self) -> Option<char> {
        if !self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    #[inline]
    fn prev_impl(&mut self) -> Option<char> {
        // Put us back into a "prev" progression.
        if !self.last_call_was_prev_impl {
            self.chunk_iter.prev();
            self.last_call_was_prev_impl = true;
        }

        // Progress the chunks iterator back if needed.
        if self.byte_idx == 0 {
            if let Some(chunk) = self.chunk_iter.prev() {
                self.cur_chunk = chunk;
                self.byte_idx = self.cur_chunk.len();
            } else {
                return None;
            }
        }

        // Find the previous char boundary, updating counters as needed, and
        // return the previous char.
        self.byte_idx -= 1;
        while !self.cur_chunk.is_char_boundary(self.byte_idx) {
            self.byte_idx -= 1;
        }
        self.chars_remaining += 1;
        return (&self.cur_chunk[self.byte_idx..]).chars().next();
    }

    #[inline]
    fn next_impl(&mut self) -> Option<char> {
        // Put us back into a "next" progression.
        if self.last_call_was_prev_impl {
            self.chunk_iter.next();
            self.last_call_was_prev_impl = false;
        }

        // Progress the chunks iterator forward if needed.
        if self.byte_idx >= self.cur_chunk.len() {
            if let Some(chunk) = self.chunk_iter.next() {
                self.cur_chunk = chunk;
                self.byte_idx = 0;
            } else {
                return None;
            }
        }

        // Find the next char boundary, updating counters as needed, and
        // return the next char.
        let start = self.byte_idx;
        self.byte_idx += 1;
        while !self.cur_chunk.is_char_boundary(self.byte_idx) {
            self.byte_idx += 1;
        }
        self.chars_remaining -= 1;
        return (&self.cur_chunk[start..]).chars().next();
    }
}

impl<'a> Iterator for Chars<'a> {
    type Item = char;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<char> {
        if !self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = if !self.is_reversed {
            self.chars_remaining
        } else {
            self.chars_total - self.chars_remaining
        };
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for Chars<'a> {}

//==========================================================

/// An iterator over a `Rope`'s lines.
///
/// The returned lines include the line break at the end, if any.
///
/// The last line is returned even if blank, in which case it
/// is returned as an empty slice.
#[derive(Debug, Clone)]
pub struct Lines<'a> {
    iter: LinesEnum<'a>,
    is_reversed: bool,
    /// The content of the current tree leaf.
    text: &'a str,
    /// The total byte position of the iterator.
    byte_idx: usize,
    at_end: bool,
    line_idx: usize,
    total_lines: usize,
}

#[derive(Debug, Clone)]
enum LinesEnum<'a> {
    Full {
        /// A stack of nodes that represents the current tree position.
        /// This stack contains only internal nodes, the leaf text is
        /// stored in the `Lines::text` field instead.
        /// Each entry contains a tree node and the index of the current
        /// child that is stored next on the stack (or the index of the
        /// leaf) for the last node.
        node_stack: Vec<(&'a Arc<Node>, usize)>,
        /// The position within the current leaf (`Lines::text`).
        leaf_byte_idx: u32,
        /// The total number of bytes this iterator can traverse.
        total_bytes: usize,
    },
    Light,
}

impl<'a> Lines<'a> {
    #[inline(always)]
    pub(crate) fn new(node: &Arc<Node>) -> Lines {
        let info = node.text_info();
        Lines::new_with_range_at(
            node,
            0,
            (0, info.bytes as usize),
            (0, info.line_breaks as usize + 1),
        )
    }

    #[inline(always)]
    pub(crate) fn new_with_range(
        node: &Arc<Node>,
        byte_idx_range: (usize, usize),
        line_idx_range: (usize, usize),
    ) -> Lines {
        Lines::new_with_range_at(node, line_idx_range.0, byte_idx_range, line_idx_range)
    }

    pub(crate) fn new_with_range_at(
        node: &Arc<Node>,
        line: usize,
        byte_idx_range: (usize, usize),
        line_idx_range: (usize, usize),
    ) -> Lines {
        debug_assert!(node.is_char_boundary(byte_idx_range.0));
        debug_assert!(node.is_char_boundary(byte_idx_range.1));
        debug_assert!(line >= line_idx_range.0);

        // For convenience/readability.
        let total_lines = line_idx_range.1 - line_idx_range.0;

        // Special-case: empty slice/rope.
        if byte_idx_range.0 == byte_idx_range.1 {
            return Lines {
                iter: LinesEnum::Light,
                text: "",
                at_end: false,
                is_reversed: false,
                byte_idx: 0,
                line_idx: 0,
                total_lines: 1,
            };
        }

        // Special-case: root is a leaf.  Return light version of the iterator.
        if node.is_leaf() {
            let text = &node.leaf_text()[byte_idx_range.0..byte_idx_range.1];
            return Lines::from_str_at(text, line - line_idx_range.0, total_lines);
        }

        // Common case.  Traverse into the tree to build the iterator.
        let mut start_byte_idx = byte_idx_range.0;
        let mut end_byte_idx = byte_idx_range.1;
        let mut line_idx = line;
        let mut chunk_byte_start = 0;
        let mut node_stack: Vec<(&Arc<Node>, usize)> = Vec::new();
        let mut node_ref = node;
        loop {
            match **node_ref {
                // Recursively traverse into whichever child has the target line break,
                // bounded by the start and end bytes of the slice/rope.
                Node::Internal(ref children) => {
                    // Find the appropriate child.
                    let (child_i, acc) = children.search_by(|_, end_info| {
                        if (end_info.bytes as usize) >= end_byte_idx {
                            true
                        } else if line_idx <= (end_info.line_breaks as usize) {
                            (end_info.bytes as usize) > start_byte_idx
                        } else {
                            false
                        }
                    });

                    // Update tracking info.
                    start_byte_idx = start_byte_idx.saturating_sub(acc.bytes as usize);
                    end_byte_idx -= acc.bytes as usize;
                    line_idx -= acc.line_breaks as usize;
                    chunk_byte_start += acc.bytes as usize;

                    // Add to the node stack.
                    node_stack.push((node_ref, child_i));
                    node_ref = &children.nodes()[child_i];
                }

                // Create the iterator.
                Node::Leaf(ref text) => {
                    let leaf_byte_idx = line_to_byte_idx(text, line_idx)
                        .max(start_byte_idx)
                        .min(end_byte_idx);

                    let res = Lines {
                        iter: LinesEnum::Full {
                            node_stack,
                            leaf_byte_idx: leaf_byte_idx as u32,
                            total_bytes: byte_idx_range.1 - byte_idx_range.0,
                        },
                        is_reversed: false,
                        text,
                        byte_idx: chunk_byte_start + leaf_byte_idx - byte_idx_range.0,
                        at_end: leaf_byte_idx == end_byte_idx
                            && line_idx > byte_to_line_idx(&text[..end_byte_idx], end_byte_idx),
                        line_idx: line - line_idx_range.0,
                        total_lines,
                    };

                    return res;
                }
            }
        }
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!).
    ///
    /// This is only exposed publicly for use in property testing.
    #[doc(hidden)]
    pub fn from_str_pt(text: &str) -> Lines {
        let line_count = byte_to_line_idx(text, text.len()) + 1;
        Lines::from_str(text, line_count)
    }

    pub(crate) fn from_str(text: &str, lines: usize) -> Lines {
        Lines {
            iter: LinesEnum::Light,
            is_reversed: false,
            text: text,
            byte_idx: 0,
            at_end: false,
            line_idx: 0,
            total_lines: lines,
        }
    }

    pub(crate) fn from_str_at(text: &str, line: usize, lines: usize) -> Lines {
        Lines {
            iter: LinesEnum::Light,
            is_reversed: false,
            text: text,
            byte_idx: line_to_byte_idx(text, line),
            at_end: line >= lines,
            line_idx: line.min(lines),
            total_lines: lines,
        }
    }
    /// Reverses the direction of the iterator in-place.
    ///
    /// In other words, swaps the behavior of [`prev()`](Lines::prev())
    /// and [`next()`](Lines::next()).
    #[inline]
    pub fn reverse(&mut self) {
        self.is_reversed = !self.is_reversed;
    }

    /// Same as `reverse()`, but returns itself.
    ///
    /// This is useful when chaining iterator methods:
    ///
    /// ```rust
    /// # use ropey::Rope;
    /// # let rope = Rope::from_str("Hello there\n world!\n");
    /// // Enumerate the rope's lines in reverse, starting from the end.
    /// for (i, line) in rope.lines_at(rope.len_lines()).reversed().enumerate() {
    ///     println!("{} {}", i, line);
    /// #   assert_eq!(line, rope.line(rope.len_lines() - i - 1));
    /// }
    #[inline]
    #[must_use]
    pub fn reversed(mut self) -> Lines<'a> {
        self.reverse();
        self
    }

    /// Advances the iterator backwards and returns the previous value.
    ///
    /// Runs in O(1) time with respect to rope length and O(N) time with
    /// respect to line length.
    #[inline(always)]
    pub fn prev(&mut self) -> Option<RopeSlice<'a>> {
        if self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    fn prev_impl(&mut self) -> Option<RopeSlice<'a>> {
        match *self {
            Lines {
                iter:
                    LinesEnum::Full {
                        ref mut node_stack,
                        ref mut leaf_byte_idx,
                        ..
                    },
                ref mut byte_idx,
                ref mut text,
                ref mut at_end,
                ref mut line_idx,
                ..
            } => {
                let tail = &text[..*leaf_byte_idx as usize];

                // The only line yielded by this iterator that doesn't
                // end with a line break is the very last line. As the
                // very last line requires a special conditon here
                // anyway we can save the result so we don't have to
                // count newlines later.
                let ends_with_line_break = if std::mem::take(at_end) {
                    if ends_with_line_break(tail) {
                        *line_idx -= 1;
                        return Some(RopeSlice(RSEnum::Light {
                            text: "",
                            char_count: 0,
                            utf16_surrogate_count: 0,
                            line_break_count: 0,
                        }));
                    }
                    false
                } else if *byte_idx == 0 {
                    return None;
                } else {
                    true
                };

                *line_idx -= 1;

                // Get the byte index of the start of the line within the chunk,
                // and whether we know if the line is contained entirely within
                // the chunk or not.
                let (line_start_idx, line_inside_chunk) = {
                    let line_start = last_line_start_byte_idx(trim_line_break(tail));
                    let line_len = *leaf_byte_idx as usize - line_start;
                    if line_len >= *byte_idx {
                        (*leaf_byte_idx as usize - *byte_idx, true)
                    } else {
                        (line_start, line_start > 0)
                    }
                };

                let chunk_line = &tail[line_start_idx..];
                *byte_idx -= chunk_line.len();

                // If the line is contained entirely within the current chunk, return it.
                if line_inside_chunk {
                    *leaf_byte_idx = line_start_idx as u32;
                    return Some(RopeSlice(RSEnum::Light {
                        text: chunk_line,
                        char_count: count_chars(chunk_line) as Count,
                        utf16_surrogate_count: count_utf16_surrogates(chunk_line) as Count,
                        line_break_count: ends_with_line_break as Count,
                    }));
                }

                // We need to advance to the next (preceding) chunk that contains
                // a line break.  As the line might span across multiple chunks we
                // track the closest common parent node (and position within that
                // node) during traversal to avoid expensive `RopeSlice` construction
                // later.
                let mut shared_parent = node_stack.len() - 1;
                let mut pos_in_shared_parent = {
                    let (parent, child_i) = *node_stack.last().unwrap();
                    parent.children().info()[..child_i]
                        .iter()
                        .fold(TextInfo::new(), |res, it| res + *it)
                };
                let mut len = TextInfo::from_str(tail);
                pos_in_shared_parent += len;

                // If the line starts exactly at the start of the chunk
                // then it might not actually span multiple chunks.
                let mut multi_chunk_slice = !tail.is_empty();
                let head_start = loop {
                    let mut stack_idx = node_stack.len() - 1;
                    let (_, child_i) = node_stack.last_mut().unwrap();

                    // If the iterator has reached the start of the parent node, advance
                    // to the previous parent.
                    if *child_i == 0 {
                        // Find how high up the stack we need to go to advance to
                        // the previous chunk.
                        while node_stack[stack_idx].1 == 0 {
                            debug_assert_ne!(stack_idx, 0, "iterated past the first leaf");
                            stack_idx -= 1;
                        }

                        // If we've reached a new high position in the stack, accumulate its
                        // TextInfo for `RopeSlice` construction later.
                        if stack_idx < shared_parent {
                            for (node, child_i) in &node_stack[stack_idx..shared_parent] {
                                for &child_pos in &node.children().info()[..*child_i] {
                                    pos_in_shared_parent += child_pos;
                                }
                            }
                            shared_parent = stack_idx;
                        }

                        // Advance to the previous chunk.
                        node_stack[stack_idx].1 -= 1;
                        while stack_idx < (node_stack.len() - 1) {
                            let child_i = node_stack[stack_idx].1;
                            let node = &node_stack[stack_idx].0.children().nodes()[child_i];
                            node_stack[stack_idx + 1] = (node, node.child_count() - 1);
                            stack_idx += 1;
                        }
                    } else {
                        // Advance to the previous sibling chunk.
                        *child_i -= 1;
                    }

                    let (node, child_i) = *node_stack.last().unwrap();
                    let info = node.children().info()[child_i];
                    let available_bytes = *byte_idx;

                    if info.line_breaks != 0 {
                        // This chunk contains a line break so it will contain the start of our line.
                        *text = node.children().nodes()[child_i].leaf_text();
                        // Find the start of the line within the chunk.
                        // The function used here is slightly different because it must
                        // not skip the first line break.
                        // A line break at the end of the chunk is already the line break
                        // we are looking for.  The line break belonging to this line is
                        // always contained in the chunk we started this iteration at.
                        let mut line_start = last_line_start_byte_idx(text);
                        // Cut off the line at the start of the iterator.
                        let line_len = text.len() - line_start;
                        if line_len >= available_bytes {
                            line_start = text.len() - available_bytes;
                        }
                        break line_start;
                    }

                    if info.bytes as usize >= available_bytes {
                        // This chunk does not contain a line break but the current
                        // line still ends here because the iterator is exhaused.
                        *text = node.children().nodes()[child_i].leaf_text();
                        break text.len() - *byte_idx;
                    }

                    len += info;
                    *byte_idx -= info.bytes as usize;
                    multi_chunk_slice = true;
                };
                let head = &text[head_start..];

                // Book keeping.
                *byte_idx -= head.len();
                *leaf_byte_idx = head_start as u32;

                // Construct the `RopeSlice` containing the line.
                // Note that `head` never contains any line breaks because the
                // iterator stops at the first line break (see comment above).
                let head_chars = count_chars(head) as Count;
                let head_surrogates = count_utf16_surrogates(head) as Count;
                let line = if multi_chunk_slice {
                    RSEnum::Full {
                        node: node_stack[shared_parent].0,
                        start_info: pos_in_shared_parent
                            - TextInfo {
                                bytes: head.len() as Count,
                                chars: head_chars,
                                utf16_surrogates: head_surrogates,
                                line_breaks: 0,
                            }
                            - len,
                        end_info: pos_in_shared_parent,
                    }
                } else {
                    RSEnum::Light {
                        text: head,
                        char_count: head_chars,
                        utf16_surrogate_count: head_surrogates,
                        line_break_count: 0,
                    }
                };
                let line = RopeSlice(line);

                Some(line)
            }

            Lines {
                iter: LinesEnum::Light,
                ref mut text,
                ref mut byte_idx,
                ref mut at_end,
                ref mut line_idx,
                ..
            } => {
                if std::mem::take(at_end) {
                    if text.is_empty() || ends_with_line_break(text) {
                        *line_idx -= 1;
                        return Some("".into());
                    }
                } else if *byte_idx == 0 {
                    return None;
                }

                let end_idx = *byte_idx;
                let start_idx = last_line_start_byte_idx(trim_line_break(&text[..end_idx]));
                *byte_idx = start_idx;
                *line_idx -= 1;
                let line = &text[start_idx..end_idx];

                return Some(RopeSlice(RSEnum::Light {
                    text: line,
                    char_count: count_chars(line) as Count,
                    utf16_surrogate_count: count_utf16_surrogates(line) as Count,
                    line_break_count: 1,
                }));
            }
        }
    }

    fn next_impl(&mut self) -> Option<RopeSlice<'a>> {
        match *self {
            Lines {
                iter:
                    LinesEnum::Full {
                        ref mut node_stack,
                        ref mut leaf_byte_idx,
                        total_bytes,
                    },
                ref mut byte_idx,
                ref mut text,
                ref mut at_end,
                ref mut line_idx,
                ..
            } => {
                if *at_end {
                    return None;
                } else if *byte_idx == total_bytes {
                    *at_end = true;
                    *line_idx += 1;
                    return Some(RopeSlice(RSEnum::Light {
                        text: "",
                        char_count: 0,
                        utf16_surrogate_count: 0,
                        line_break_count: 0,
                    }));
                }
                *line_idx += 1;

                let head = &text[*leaf_byte_idx as usize..];
                let mut line_len = line_to_byte_idx(head, 1);

                // Check if the iterators needs to advance to the next chunk.
                // During this check the number of newline (0 or 1) is yielded
                // for free so save that aswell.
                let available_bytes = total_bytes - *byte_idx;
                let (line_inside_chunk, line_break_count) = if line_len >= available_bytes {
                    // If the iterator is exhausted we don't need to switch chunks.
                    // Check if the last line has a line break to decide whether
                    // we still need to yield an empty line later.
                    line_len = available_bytes;
                    let ends_with_line_break = ends_with_line_break(&head[..line_len]);
                    *at_end = !ends_with_line_break;
                    // Reached end of the text, so no need to advance.
                    (true, ends_with_line_break as u64)
                } else {
                    // Iterator is not yet exhausted, so advance to the next chunk
                    // if we've reached the chunk boundary and the last character
                    // is not a line break.  If the iterator is not exhausted, a
                    // line always ends with a line break.
                    (line_len != head.len() || ends_with_line_break(head), 1)
                };

                // Yield the current line if it is contained within the current chunk.
                if line_inside_chunk {
                    let line = &head[..line_len];
                    *byte_idx += line_len;
                    *leaf_byte_idx += line_len as u32;

                    return Some(RopeSlice(RSEnum::Light {
                        text: line,
                        char_count: count_chars(line) as Count,
                        utf16_surrogate_count: count_utf16_surrogates(line) as Count,
                        line_break_count,
                    }));
                }

                *byte_idx += head.len();

                // We need to advance to the next chunk that contains
                // a line break.  As the line might span across multiple chunks we
                // track the closest common parent node (and position within that
                // node) during traversal to avoid expensive `RopeSlice` construction
                // later.
                let mut shared_parent = node_stack.len() - 1;
                let mut pos_in_shared_parent = {
                    let (parent, child_i) = *node_stack.last().unwrap();
                    parent.children().info()[..=child_i]
                        .iter()
                        .fold(TextInfo::new(), |res, it| res + *it)
                };
                let mut len = TextInfo::from_str(head);
                pos_in_shared_parent -= len;

                // If the line starts exactly at the start of the next chunk
                // then it might not actually span multiple chunks.
                let mut multi_chunk_slice = !head.is_empty();
                let (tail_len, tail_ends_with_newline) = loop {
                    let mut stack_idx = node_stack.len() - 1;

                    // Advance to the next sibling chunk.
                    let (_, child_i) = node_stack.last_mut().unwrap();
                    *child_i += 1;

                    // If the iterator has reached the end of the parent node, advance
                    // to the next parent.
                    if *child_i >= node_stack[stack_idx].0.child_count() {
                        // Find how high up the stack we need to go to advance to
                        // the next chunk.
                        while node_stack[stack_idx].1 >= (node_stack[stack_idx].0.child_count() - 1)
                        {
                            debug_assert_ne!(stack_idx, 0, "iterated past the last leaf");
                            stack_idx -= 1;
                        }

                        // If we've reached a new high position in the stack, accumulate its
                        // TextInfo for `RopeSlice` construction later.
                        if stack_idx < shared_parent {
                            for (node, child_i) in &node_stack[stack_idx..shared_parent] {
                                for &child_pos in &node.children().info()[..*child_i] {
                                    pos_in_shared_parent += child_pos;
                                }
                            }
                            shared_parent = stack_idx;
                        }

                        // Advance to the next chunk.
                        node_stack[stack_idx].1 += 1;
                        while stack_idx < (node_stack.len() - 1) {
                            let child_i = node_stack[stack_idx].1;
                            let node = &node_stack[stack_idx].0.children().nodes()[child_i];
                            node_stack[stack_idx + 1] = (node, 0);
                            stack_idx += 1;
                        }
                    }

                    let (node, child_i) = *node_stack.last().unwrap();
                    let info = node.children().info()[child_i];
                    let available_bytes = total_bytes - *byte_idx;

                    if info.line_breaks != 0 {
                        // This chunk contains a line break so it will contain the start of our line.
                        *text = node.children().nodes()[child_i].leaf_text();
                        // Find the end of the line within the chunk.
                        let mut line_end = line_to_byte_idx(text, 1);
                        // Check if the iterator was exhausted.
                        let ends_with_newline = if line_end >= available_bytes {
                            // Handle terminating lines without a line break properly.
                            line_end = available_bytes;
                            let ends_with_newline = ends_with_line_break(&text[..line_end]);
                            *at_end = !ends_with_newline;
                            ends_with_newline
                        } else {
                            true
                        };
                        break (line_end, ends_with_newline);
                    }

                    if info.bytes as usize >= available_bytes {
                        // This chunk does not contain a line break but the current
                        // line still ends here because the iterator is exhausted.
                        *at_end = true;
                        *text = node.children().nodes()[child_i].leaf_text();
                        break (available_bytes, false);
                    }

                    len += info;
                    *byte_idx += info.bytes as usize;
                    multi_chunk_slice = true;
                };

                // Book keeping.
                *byte_idx += tail_len;
                *leaf_byte_idx = tail_len as u32;

                // Construct the `RopeSlice` containing the line.
                let line_tail = &text[..tail_len];
                let line_tail_chars = count_chars(line_tail) as Count;
                let line_tail_surrogates = count_utf16_surrogates(line_tail) as Count;
                let line = if multi_chunk_slice {
                    RSEnum::Full {
                        node: node_stack[shared_parent].0,
                        start_info: pos_in_shared_parent,
                        end_info: pos_in_shared_parent
                            + len
                            + TextInfo {
                                bytes: tail_len as Count,
                                chars: line_tail_chars,
                                utf16_surrogates: line_tail_surrogates,
                                line_breaks: tail_ends_with_newline as Count,
                            },
                    }
                } else {
                    RSEnum::Light {
                        text: line_tail,
                        char_count: line_tail_chars,
                        utf16_surrogate_count: line_tail_surrogates,
                        line_break_count: tail_ends_with_newline as Count,
                    }
                };

                Some(RopeSlice(line))
            }

            Lines {
                iter: LinesEnum::Light,
                text,
                ref mut byte_idx,
                ref mut at_end,
                ref mut line_idx,
                ..
            } => {
                if *at_end {
                    return None;
                } else if *byte_idx == text.len() {
                    *at_end = true;
                    *line_idx += 1;
                    return Some("".into());
                }

                let start_idx = *byte_idx;
                let end_idx = line_to_byte_idx(&text[start_idx..], 1) + start_idx;
                *byte_idx = end_idx;
                *line_idx += 1;

                if end_idx == text.len() {
                    *at_end = !ends_with_line_break(text);
                }

                return Some((&text[start_idx..end_idx]).into());
            }
        }
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = RopeSlice<'a>;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in O(1) time with respect to rope length and O(N) time with
    /// respect to line length.
    #[inline(always)]
    fn next(&mut self) -> Option<RopeSlice<'a>> {
        if self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_reversed {
            (self.line_idx, Some(self.line_idx))
        } else {
            (
                self.total_lines - self.line_idx,
                Some(self.total_lines - self.line_idx),
            )
        }
    }
}

impl ExactSizeIterator for Lines<'_> {}

//==========================================================

/// An iterator over a `Rope`'s contiguous `str` chunks.
///
/// Internally, each `Rope` stores text as a segemented collection of utf8
/// strings. This iterator iterates over those segments, returning a
/// `&str` slice for each one.  It is useful for situations such as:
///
/// - Writing a rope's utf8 text data to disk (but see
///   [`write_to()`](crate::rope::Rope::write_to) for a convenience function that does this
///   for casual use-cases).
/// - Streaming a rope's text data somewhere.
/// - Saving a rope to a non-utf8 encoding, doing the encoding conversion
///   incrementally as you go.
/// - Writing custom iterators over a rope's text data.
///
/// There are precisely two guarantees about the yielded chunks:
///
/// - All non-empty chunks are yielded, and they are yielded in order.
/// - CRLF pairs are never split across chunks.
///
/// There are no guarantees about the size of yielded chunks, and except for
/// CRLF pairs and being valid `str` slices there are no guarantees about
/// where the chunks are split.  For example, they may be zero-sized, they
/// don't necessarily align with line breaks, etc.
#[derive(Debug, Clone)]
pub struct Chunks<'a> {
    iter: ChunksEnum<'a>,
    is_reversed: bool,
}

#[derive(Debug, Clone)]
enum ChunksEnum<'a> {
    Full {
        node_stack: Vec<(&'a Arc<Node>, usize)>, // (node ref, index of current child)
        total_bytes: usize,                      // Total bytes in the data range of the iterator.
        byte_idx: isize, // The index of the current byte relative to the data range start.
    },
    Light {
        text: &'a str,
        is_end: bool,
    },
}

impl<'a> Chunks<'a> {
    #[inline(always)]
    pub(crate) fn new(node: &Arc<Node>) -> Chunks {
        let info = node.text_info();
        Chunks::new_with_range_at_byte(
            node,
            0,
            (0, info.bytes as usize),
            (0, info.chars as usize),
            (0, info.line_breaks as usize + 1),
        )
        .0
    }

    #[inline(always)]
    pub(crate) fn new_with_range(
        node: &Arc<Node>,
        byte_idx_range: (usize, usize),
        char_idx_range: (usize, usize),
        line_break_idx_range: (usize, usize),
    ) -> Chunks {
        Chunks::new_with_range_at_byte(
            node,
            byte_idx_range.0,
            byte_idx_range,
            char_idx_range,
            line_break_idx_range,
        )
        .0
    }

    /// The main workhorse function for creating new `Chunks` iterators.
    ///
    /// Creates a new `Chunks` iterator from the given node, starting the
    /// iterator at the chunk containing the `at_byte` byte index (i.e. the
    /// `next()` method will yield the chunk containing that byte).  The range
    /// of the iterator is bounded by `byte_idx_range`.
    ///
    /// Both `at_byte` and `byte_idx_range` are relative to the beginning of
    /// of the passed node.
    ///
    /// Passing an `at_byte` equal to the max of `byte_idx_range` creates an
    /// iterator at the end of forward iteration.
    ///
    /// Returns the iterator and the byte/char/line index of its start relative
    /// to the start of the node.
    pub(crate) fn new_with_range_at_byte(
        node: &Arc<Node>,
        at_byte: usize,
        byte_idx_range: (usize, usize),
        char_idx_range: (usize, usize),
        line_break_idx_range: (usize, usize),
    ) -> (Chunks, usize, usize, usize) {
        debug_assert!(at_byte >= byte_idx_range.0);
        debug_assert!(at_byte <= byte_idx_range.1);

        // For convenience/readability.
        let start_byte = byte_idx_range.0;
        let end_byte = byte_idx_range.1;

        // Special-case for empty text contents.
        if start_byte == end_byte {
            return (
                Chunks {
                    iter: ChunksEnum::Light {
                        text: "",
                        is_end: false,
                    },
                    is_reversed: false,
                },
                0,
                0,
                0,
            );
        }

        // If root is a leaf, return light version of the iter.
        if node.is_leaf() {
            let text = &node.leaf_text()[start_byte..end_byte];
            if at_byte == end_byte {
                return (
                    Chunks {
                        iter: ChunksEnum::Light {
                            text: text,
                            is_end: true,
                        },
                        is_reversed: false,
                    },
                    text.len(),
                    count_chars(text),
                    byte_to_line_idx(text, text.len()),
                );
            } else {
                return (
                    Chunks {
                        iter: ChunksEnum::Light {
                            text: text,
                            is_end: false,
                        },
                        is_reversed: false,
                    },
                    0,
                    0,
                    0,
                );
            }
        }

        // Create and populate the node stack, and determine the char index
        // within the first chunk, and byte index of the start of that chunk.
        let mut info = TextInfo::new();
        let mut byte_idx = at_byte as isize;
        let node_stack = {
            let mut node_stack: Vec<(&Arc<Node>, usize)> = Vec::new();
            let mut node_ref = node;
            loop {
                match **node_ref {
                    Node::Leaf(ref text) => {
                        if at_byte < end_byte || byte_idx == 0 {
                            byte_idx = info.bytes as isize - start_byte as isize;
                        } else {
                            byte_idx =
                                (info.bytes as isize + text.len() as isize) - start_byte as isize;
                            info = TextInfo {
                                bytes: byte_idx_range.1 as u64,
                                chars: char_idx_range.1 as u64,
                                utf16_surrogates: 0, // Bogus value, not needed
                                line_breaks: line_break_idx_range.1 as u64 - 1,
                            };
                            (*node_stack.last_mut().unwrap()).1 += 1;
                        }
                        break;
                    }
                    Node::Internal(ref children) => {
                        let (child_i, acc_info) = children.search_byte_idx(byte_idx as usize);
                        info += acc_info;
                        node_stack.push((node_ref, child_i));
                        node_ref = &children.nodes()[child_i];
                        byte_idx -= acc_info.bytes as isize;
                    }
                }
            }
            node_stack
        };

        // Create the iterator.
        (
            Chunks {
                iter: ChunksEnum::Full {
                    node_stack: node_stack,
                    total_bytes: end_byte - start_byte,
                    byte_idx: byte_idx,
                },
                is_reversed: false,
            },
            (info.bytes as usize).max(byte_idx_range.0),
            (info.chars as usize).max(char_idx_range.0),
            (info.line_breaks as usize).max(line_break_idx_range.0),
        )
    }

    #[inline(always)]
    pub(crate) fn new_with_range_at_char(
        node: &Arc<Node>,
        at_char: usize,
        byte_idx_range: (usize, usize),
        char_idx_range: (usize, usize),
        line_break_idx_range: (usize, usize),
    ) -> (Chunks, usize, usize, usize) {
        let at_byte = if at_char == char_idx_range.1 {
            byte_idx_range.1
        } else {
            (node.get_chunk_at_char(at_char).1.bytes as usize).max(byte_idx_range.0)
        };

        Chunks::new_with_range_at_byte(
            node,
            at_byte,
            byte_idx_range,
            char_idx_range,
            line_break_idx_range,
        )
    }

    #[inline(always)]
    pub(crate) fn new_with_range_at_line_break(
        node: &Arc<Node>,
        at_line_break: usize,
        byte_idx_range: (usize, usize),
        char_idx_range: (usize, usize),
        line_break_idx_range: (usize, usize),
    ) -> (Chunks, usize, usize, usize) {
        let at_byte = if at_line_break == line_break_idx_range.1 {
            byte_idx_range.1
        } else {
            (node.get_chunk_at_line_break(at_line_break).1.bytes as usize).max(byte_idx_range.0)
        };

        Chunks::new_with_range_at_byte(
            node,
            at_byte,
            byte_idx_range,
            char_idx_range,
            line_break_idx_range,
        )
    }

    pub(crate) fn from_str(text: &str, at_end: bool) -> Chunks {
        Chunks {
            iter: ChunksEnum::Light {
                text: text,
                is_end: at_end,
            },
            is_reversed: false,
        }
    }

    /// Reverses the direction of the iterator in-place.
    ///
    /// In other words, swaps the behavior of [`prev()`](Chunks::prev())
    /// and [`next()`](Chunks::next()).
    #[inline]
    pub fn reverse(&mut self) {
        self.is_reversed = !self.is_reversed;
    }

    /// Same as `reverse()`, but returns itself.
    ///
    /// This is useful when chaining iterator methods:
    ///
    /// ```rust
    /// # use ropey::Rope;
    /// # let rope = Rope::from_str("Hello there\n world!\n");
    /// // Enumerate the rope's chunks in reverse, starting from the end.
    /// for (i, chunk) in rope.chunks_at_byte(rope.len_bytes()).0.reversed().enumerate() {
    ///     println!("{} {}", i, chunk);
    /// #   assert_eq!(chunk, rope.chunks().nth(rope.chunks().count() - i - 1).unwrap());
    /// }
    #[inline]
    #[must_use]
    pub fn reversed(mut self) -> Chunks<'a> {
        self.reverse();
        self
    }

    /// Advances the iterator backwards and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn prev(&mut self) -> Option<&'a str> {
        if !self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    fn prev_impl(&mut self) -> Option<&'a str> {
        match *self {
            Chunks {
                iter:
                    ChunksEnum::Full {
                        ref mut node_stack,
                        total_bytes,
                        ref mut byte_idx,
                    },
                ..
            } => {
                if *byte_idx <= 0 {
                    return None;
                }

                // Progress the node stack if needed.
                let mut stack_idx = node_stack.len() - 1;
                if node_stack[stack_idx].1 == 0 {
                    while node_stack[stack_idx].1 == 0 {
                        if stack_idx == 0 {
                            return None;
                        } else {
                            stack_idx -= 1;
                        }
                    }
                    node_stack[stack_idx].1 -= 1;
                    while stack_idx < (node_stack.len() - 1) {
                        let child_i = node_stack[stack_idx].1;
                        let node = &node_stack[stack_idx].0.children().nodes()[child_i];
                        node_stack[stack_idx + 1] = (node, node.child_count() - 1);
                        stack_idx += 1;
                    }
                    node_stack[stack_idx].1 += 1;
                }

                // Fetch the node and child index.
                let (node, ref mut child_i) = node_stack.last_mut().unwrap();
                *child_i -= 1;

                // Get the text, sliced to the appropriate range.
                let text = node.children().nodes()[*child_i].leaf_text();
                *byte_idx -= text.len() as isize;
                let text_slice = {
                    let start_byte = if *byte_idx < 0 {
                        (-*byte_idx) as usize
                    } else {
                        0
                    };
                    let end_byte = text.len().min((total_bytes as isize - *byte_idx) as usize);
                    &text[start_byte..end_byte]
                };

                // Return the text.
                return Some(text_slice);
            }

            Chunks {
                iter:
                    ChunksEnum::Light {
                        text,
                        ref mut is_end,
                    },
                ..
            } => {
                if !*is_end || text.is_empty() {
                    return None;
                } else {
                    *is_end = false;
                    return Some(text);
                }
            }
        }
    }

    fn next_impl(&mut self) -> Option<&'a str> {
        match *self {
            Chunks {
                iter:
                    ChunksEnum::Full {
                        ref mut node_stack,
                        total_bytes,
                        ref mut byte_idx,
                    },
                ..
            } => {
                if *byte_idx >= total_bytes as isize {
                    return None;
                }

                // Progress the node stack if needed.
                let mut stack_idx = node_stack.len() - 1;
                if node_stack[stack_idx].1 >= node_stack[stack_idx].0.child_count() {
                    while node_stack[stack_idx].1 >= (node_stack[stack_idx].0.child_count() - 1) {
                        if stack_idx == 0 {
                            return None;
                        } else {
                            stack_idx -= 1;
                        }
                    }
                    node_stack[stack_idx].1 += 1;
                    while stack_idx < (node_stack.len() - 1) {
                        let child_i = node_stack[stack_idx].1;
                        let node = &node_stack[stack_idx].0.children().nodes()[child_i];
                        node_stack[stack_idx + 1] = (node, 0);
                        stack_idx += 1;
                    }
                }

                // Fetch the node and child index.
                let (node, ref mut child_i) = node_stack.last_mut().unwrap();

                // Get the text, sliced to the appropriate range.
                let text = node.children().nodes()[*child_i].leaf_text();
                let text_slice = {
                    let start_byte = if *byte_idx < 0 {
                        (-*byte_idx) as usize
                    } else {
                        0
                    };
                    let end_byte = text.len().min((total_bytes as isize - *byte_idx) as usize);
                    &text[start_byte..end_byte]
                };

                // Book keeping.
                *byte_idx += text.len() as isize;
                *child_i += 1;

                // Return the text.
                return Some(text_slice);
            }

            Chunks {
                iter:
                    ChunksEnum::Light {
                        text,
                        ref mut is_end,
                    },
                ..
            } => {
                if *is_end || text.is_empty() {
                    return None;
                } else {
                    *is_end = true;
                    return Some(text);
                }
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
        if !self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::while_let_on_iterator)]
    use super::*;
    use crate::Rope;

    const TEXT: &str = "\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        ";

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_01() {
        let r = Rope::from_str(TEXT);
        for (br, bt) in r.bytes().zip(TEXT.bytes()) {
            assert_eq!(br, bt);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_02() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes();
        while let Some(_) = itr.next() {}

        let mut i = TEXT.len();
        while let Some(b) = itr.prev() {
            i -= 1;
            assert_eq!(b, TEXT.as_bytes()[i]);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_03() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes();

        itr.next();
        itr.prev();
        assert_eq!(None, itr.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_04() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes();
        while let Some(_) = itr.next() {}

        itr.prev();
        itr.next();
        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_05() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes();

        assert_eq!(None, itr.prev());
        itr.next();
        itr.prev();
        assert_eq!(None, itr.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_06() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes();
        while let Some(_) = itr.next() {}

        assert_eq!(None, itr.next());
        itr.prev();
        itr.next();
        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_07() {
        let mut itr = Bytes::from_str("a");

        assert_eq!(Some(0x61), itr.next());
        assert_eq!(None, itr.next());
        assert_eq!(Some(0x61), itr.prev());
        assert_eq!(None, itr.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_at_01() {
        let r = Rope::from_str(TEXT);

        let mut bytes_1 = TEXT.bytes();
        for i in 0..(r.len_bytes() + 1) {
            let mut bytes_2 = r.bytes_at(i);
            assert_eq!(bytes_1.next(), bytes_2.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_at_02() {
        let r = Rope::from_str(TEXT);
        let mut bytes = r.bytes_at(r.len_bytes());
        assert_eq!(bytes.next(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_at_03() {
        let r = Rope::from_str(TEXT);
        let mut bytes_1 = r.bytes_at(r.len_bytes());
        let mut bytes_2 = TEXT.bytes();

        while let Some(b) = bytes_2.next_back() {
            assert_eq!(bytes_1.prev(), Some(b));
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_exact_size_iter_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut byte_count = s.len_bytes();
        let mut bytes = s.bytes();

        assert_eq!(byte_count, bytes.len());

        while let Some(_) = bytes.next() {
            byte_count -= 1;
            assert_eq!(byte_count, bytes.len());
        }

        bytes.next();
        bytes.next();
        bytes.next();
        bytes.next();
        bytes.next();
        bytes.next();
        bytes.next();
        assert_eq!(bytes.len(), 0);
        assert_eq!(byte_count, 0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_exact_size_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        for i in 0..=s.len_bytes() {
            let bytes = s.bytes_at(i);
            assert_eq!(s.len_bytes() - i, bytes.len());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_exact_size_iter_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut byte_count = 0;
        let mut bytes = s.bytes_at(s.len_bytes());

        assert_eq!(byte_count, bytes.len());

        while bytes.prev().is_some() {
            byte_count += 1;
            assert_eq!(byte_count, bytes.len());
        }

        assert_eq!(bytes.len(), s.len_bytes());
        bytes.prev();
        bytes.prev();
        bytes.prev();
        bytes.prev();
        bytes.prev();
        bytes.prev();
        bytes.prev();
        assert_eq!(bytes.len(), s.len_bytes());
        assert_eq!(byte_count, s.len_bytes());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_reverse_01() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes();
        let mut stack = Vec::new();

        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_reverse_02() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes_at(r.len_bytes() / 3);
        let mut stack = Vec::new();

        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_reverse_03() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes_at(r.len_bytes() / 3);
        let mut stack = Vec::new();

        itr.reverse();
        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_reverse_04() {
        let mut itr = Bytes::from_str("a");

        assert_eq!(Some(0x61), itr.next());
        assert_eq!(None, itr.next());
        itr.reverse();
        assert_eq!(Some(0x61), itr.next());
        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_reverse_exact_size_iter_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut bytes = s.bytes_at(42);
        bytes.reverse();
        let mut byte_count = 42;

        assert_eq!(42, bytes.len());

        while let Some(_) = bytes.next() {
            byte_count -= 1;
            assert_eq!(byte_count, bytes.len());
        }

        bytes.next();
        bytes.next();
        bytes.next();
        bytes.next();
        bytes.next();
        bytes.next();
        bytes.next();
        assert_eq!(bytes.len(), 0);
        assert_eq!(byte_count, 0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_01() {
        let r = Rope::from_str(TEXT);
        for (cr, ct) in r.chars().zip(TEXT.chars()) {
            assert_eq!(cr, ct);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_02() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars();
        let mut text_itr = TEXT.chars();
        while let Some(_) = itr.next() {}

        while let Some(b) = itr.prev() {
            assert_eq!(b, text_itr.next_back().unwrap());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_03() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars();

        itr.next();
        itr.prev();
        assert_eq!(None, itr.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_04() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars();
        while let Some(_) = itr.next() {}

        itr.prev();
        itr.next();
        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_05() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars();

        assert_eq!(None, itr.prev());
        itr.next();
        itr.prev();
        assert_eq!(None, itr.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_06() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars();
        while let Some(_) = itr.next() {}

        assert_eq!(None, itr.next());
        itr.prev();
        itr.next();
        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_07() {
        let mut itr = Chars::from_str("a");

        assert_eq!(Some('a'), itr.next());
        assert_eq!(None, itr.next());
        assert_eq!(Some('a'), itr.prev());
        assert_eq!(None, itr.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_at_01() {
        let r = Rope::from_str(TEXT);

        let mut chars_1 = TEXT.chars();
        for i in 0..(r.len_chars() + 1) {
            let mut chars_2 = r.chars_at(i);
            assert_eq!(chars_1.next(), chars_2.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_at_02() {
        let r = Rope::from_str(TEXT);
        let mut chars = r.chars_at(r.len_chars());
        assert_eq!(chars.next(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_at_03() {
        let r = Rope::from_str(TEXT);
        let mut chars_1 = r.chars_at(r.len_chars());
        let mut chars_2 = TEXT.chars();

        while let Some(c) = chars_2.next_back() {
            assert_eq!(chars_1.prev(), Some(c));
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_exact_size_iter_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut char_count = s.len_chars();
        let mut chars = s.chars();

        assert_eq!(char_count, chars.len());

        while let Some(_) = chars.next() {
            char_count -= 1;
            assert_eq!(char_count, chars.len());
        }

        assert_eq!(char_count, 0);
        assert_eq!(chars.len(), 0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_exact_size_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        for i in 0..=s.len_chars() {
            let chars = s.chars_at(i);
            assert_eq!(s.len_chars() - i, chars.len());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_exact_size_iter_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut char_count = 0;
        let mut chars = s.chars_at(s.len_chars());

        assert_eq!(char_count, chars.len());

        while chars.prev().is_some() {
            char_count += 1;
            assert_eq!(char_count, chars.len());
        }

        assert_eq!(char_count, s.len_chars());
        assert_eq!(chars.len(), s.len_chars());
        chars.prev();
        assert_eq!(chars.len(), s.len_chars());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_reverse_01() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars();
        let mut stack = Vec::new();

        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_reverse_02() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars_at(r.len_chars() / 3);
        let mut stack = Vec::new();

        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_reverse_03() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars_at(r.len_chars() / 3);
        let mut stack = Vec::new();

        itr.reverse();
        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_reverse_04() {
        let mut itr = Chars::from_str("a");

        assert_eq!(Some('a'), itr.next());
        assert_eq!(None, itr.next());
        itr.reverse();
        assert_eq!(Some('a'), itr.next());
        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_reverse_exact_size_iter_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut chars = s.chars_at(42);
        chars.reverse();
        let mut char_count = 42;

        assert_eq!(42, chars.len());

        while let Some(_) = chars.next() {
            char_count -= 1;
            assert_eq!(char_count, chars.len());
        }

        chars.next();
        chars.next();
        chars.next();
        chars.next();
        chars.next();
        chars.next();
        chars.next();
        assert_eq!(chars.len(), 0);
        assert_eq!(char_count, 0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_21() {
        let r = Rope::from_str("a\nb\nc\nd\ne\nf\ng\nh\n");
        for (line, c) in r.lines().zip('a'..='h') {
            assert_eq!(line, format!("{c}\n"))
        }
        for (line, c) in r
            .lines_at(r.len_lines() - 1)
            .reversed()
            .zip(('a'..='h').rev())
        {
            assert_eq!(line, format!("{c}\n"))
        }

        let r = Rope::from_str("ab\nc\nd\ne\nf\ng\nh\n");
        for (line, c) in r.slice(1..).lines().zip('b'..='h') {
            assert_eq!(line, format!("{c}\n"))
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(..);

        assert_eq!(34, r.lines().count());
        assert_eq!(34, s.lines().count());

        // Rope
        let mut lines = r.lines();
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
        let mut lines = s.lines();
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

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_02() {
        let text = "Hello there!\nHow goes it?";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines().count());
        assert_eq!(2, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_03() {
        let text = "Hello there!\nHow goes it?\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(3, r.lines().count());
        assert_eq!(3, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_04() {
        let text = "Hello there!\nHow goes it?\nYeah!";
        let r = Rope::from_str(text);
        let s1 = r.slice(..25);
        let s2 = r.slice(..26);

        assert_eq!(2, s1.lines().count());
        assert_eq!(3, s2.lines().count());

        let mut lines = s1.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s2.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_05() {
        let text = "";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(1, r.lines().count());
        assert_eq!(1, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_06() {
        let text = "a";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(1, r.lines().count());
        assert_eq!(1, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("a", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("a", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_07() {
        let text = "a\nb";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines().count());
        assert_eq!(2, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_08() {
        let text = "\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines().count());
        assert_eq!(2, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_09() {
        let text = "a\nb\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(3, r.lines().count());
        assert_eq!(3, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_10() {
        let r = Rope::from_str(TEXT);

        let mut itr = r.lines();

        assert_eq!(None, itr.prev());
        assert_eq!(None, itr.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_11() {
        let r = Rope::from_str(TEXT);

        let mut lines = Vec::new();
        let mut itr = r.lines();

        while let Some(line) = itr.next() {
            lines.push(line);
        }

        while let Some(line) = itr.prev() {
            assert_eq!(line, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_12() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut lines = Vec::new();
        let mut itr = s.lines();

        while let Some(line) = itr.next() {
            lines.push(line);
        }

        while let Some(line) = itr.prev() {
            assert_eq!(line, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_13() {
        let text = "";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines();

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_14() {
        let text = "a";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines();

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_15() {
        let text = "a\nb";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines();

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_16() {
        let text = "\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines();

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_17() {
        let text = "a\nb\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines();

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..r.len_lines() {
            let line = r.line(i);
            let mut lines = r.lines_at(i);
            assert_eq!(Some(line), lines.next());
        }

        let mut lines = r.lines_at(r.len_lines());
        assert_eq!(None, lines.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        for i in 0..s.len_lines() {
            let line = s.line(i);
            let mut lines = s.lines_at(i);
            assert_eq!(Some(line), lines.next());
        }

        let mut lines = s.lines_at(s.len_lines());
        assert_eq!(None, lines.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..34);

        let mut lines = s.lines_at(0);
        assert_eq!("", lines.next().unwrap());

        let mut lines = s.lines_at(1);
        assert_eq!(None, lines.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_exact_size_iter_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut line_count = s.len_lines();
        let mut lines = s.lines();

        assert_eq!(line_count, lines.len());

        while let Some(_) = lines.next() {
            line_count -= 1;
            assert_eq!(line_count, lines.len());
        }

        assert_eq!(lines.len(), 0);
        lines.next();
        lines.next();
        lines.next();
        lines.next();
        lines.next();
        lines.next();
        lines.next();
        assert_eq!(lines.len(), 0);
        assert_eq!(line_count, 0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_exact_size_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        for i in 0..=s.len_lines() {
            let lines = s.lines_at(i);
            assert_eq!(s.len_lines() - i, lines.len());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_exact_size_iter_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut line_count = 0;
        let mut lines = s.lines_at(s.len_lines());

        assert_eq!(line_count, lines.len());

        while lines.prev().is_some() {
            line_count += 1;
            assert_eq!(line_count, lines.len());
        }

        assert_eq!(lines.len(), s.len_lines());
        lines.prev();
        lines.prev();
        lines.prev();
        lines.prev();
        lines.prev();
        lines.prev();
        lines.prev();
        assert_eq!(lines.len(), s.len_lines());
        assert_eq!(line_count, s.len_lines());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_exact_size_iter_04() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
        for i in 0..r.len_chars() {
            let s = r.slice(..i);
            let lines = s.lines();
            if cfg!(any(feature = "cr_lines", feature = "unicode_lines")) {
                assert_eq!(lines.len(), 1 + ((i + 1) / 2));
            } else {
                assert_eq!(lines.len(), 1 + (i / 2));
            }
            assert_eq!(lines.len(), lines.count());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_reverse_01() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.lines();
        let mut stack = Vec::new();

        for _ in 0..8 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..8 {
            assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_reverse_02() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.lines_at(r.len_lines() / 3);
        let mut stack = Vec::new();

        for _ in 0..8 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..8 {
            assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_reverse_03() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.lines_at(r.len_lines() / 3);
        let mut stack = Vec::new();

        itr.reverse();
        for _ in 0..8 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..8 {
            assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_reverse_04() {
        let mut itr = Lines::from_str("a\n", 1);

        assert_eq!(Some("a\n".into()), itr.next());
        assert_eq!(Some("".into()), itr.next());
        assert_eq!(None, itr.next());
        itr.reverse();
        assert_eq!(Some("".into()), itr.next());
        assert_eq!(Some("a\n".into()), itr.next());
        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_reverse_exact_size_iter_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let mut lines = s.lines_at(4);
        lines.reverse();
        let mut line_count = 4;

        assert_eq!(4, lines.len());

        while let Some(_) = lines.next() {
            line_count -= 1;
            assert_eq!(line_count, lines.len());
        }

        lines.next();
        lines.next();
        lines.next();
        lines.next();
        lines.next();
        lines.next();
        lines.next();
        assert_eq!(lines.len(), 0);
        assert_eq!(line_count, 0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_reverse_exact_size_iter_02() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
        for i in 0..r.len_chars() {
            let s = r.slice(..i);
            let lines = s.lines_at((i + 1) / 2).reversed();
            assert_eq!(lines.len(), lines.count());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_01() {
        let r = Rope::from_str(TEXT);

        let mut idx = 0;
        for chunk in r.chunks() {
            assert_eq!(chunk, &TEXT[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_02() {
        let r = Rope::from_str("");
        let mut itr = r.chunks();

        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_03() {
        let r = Rope::from_str(TEXT);

        let mut itr = r.chunks();

        assert_eq!(None, itr.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_04() {
        let r = Rope::from_str(TEXT);

        let mut chunks = Vec::new();
        let mut itr = r.chunks();

        while let Some(text) = itr.next() {
            chunks.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, chunks.pop().unwrap());
        }

        assert!(chunks.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_byte_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..r.len_bytes() {
            let (chunk, b, c, l) = r.chunk_at_byte(i);
            let (mut chunks, bs, cs, ls) = r.chunks_at_byte(i);

            assert_eq!(b, bs);
            assert_eq!(c, cs);
            assert_eq!(l, ls);
            assert_eq!(Some(chunk), chunks.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_byte_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        for i in 0..(s.len_chars() + 1) {
            let (chunk, b, c, l) = s.chunk_at_byte(i);
            let (mut chunks, bs, cs, ls) = s.chunks_at_byte(i);

            assert_eq!(b, bs);
            assert_eq!(c, cs);
            assert_eq!(l, ls);
            assert_eq!(Some(chunk), chunks.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_byte_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let (mut chunks, _, _, _) = s.chunks_at_byte(s.len_bytes());
        assert_eq!(chunks.next(), None);

        let (mut chunks, _, _, _) = s.chunks_at_byte(s.len_bytes());
        assert_eq!(s.chunks().last(), chunks.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_byte_04() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..34);

        let (mut chunks, _, _, _) = s.chunks_at_byte(0);
        assert_eq!(chunks.next(), None);

        let (mut chunks, _, _, _) = s.chunks_at_byte(0);
        assert_eq!(chunks.prev(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_char_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..r.len_chars() {
            let (chunk, b, c, l) = r.chunk_at_char(i);
            let (mut chunks, bs, cs, ls) = r.chunks_at_char(i);

            assert_eq!(b, bs);
            assert_eq!(c, cs);
            assert_eq!(l, ls);
            assert_eq!(Some(chunk), chunks.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_char_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        for i in 0..s.len_chars() {
            let (chunk, b, c, l) = s.chunk_at_char(i);
            let (mut chunks, bs, cs, ls) = s.chunks_at_char(i);

            assert_eq!(b, bs);
            assert_eq!(c, cs);
            assert_eq!(l, ls);
            assert_eq!(Some(chunk), chunks.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_char_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let (mut chunks, _, _, _) = s.chunks_at_char(s.len_chars());
        assert_eq!(chunks.next(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_char_04() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..34);

        let (mut chunks, _, _, _) = s.chunks_at_char(0);
        assert_eq!(chunks.next(), None);

        let (mut chunks, _, _, _) = s.chunks_at_char(0);
        assert_eq!(chunks.prev(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_line_break_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..r.len_lines() {
            let (chunk, b, c, l) = r.chunk_at_line_break(i);
            let (mut chunks, bs, cs, ls) = r.chunks_at_line_break(i);

            assert_eq!(b, bs);
            assert_eq!(c, cs);
            assert_eq!(l, ls);
            assert_eq!(Some(chunk), chunks.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_line_break_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        for i in 0..s.len_lines() {
            let (chunk, b, c, l) = s.chunk_at_line_break(i);
            let (mut chunks, bs, cs, ls) = s.chunks_at_line_break(i);

            assert_eq!(Some(chunk), chunks.next());
            assert_eq!(b, bs);
            assert_eq!(c, cs);
            assert_eq!(l, ls);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_line_break_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        let (mut chunks, _, _, _) = s.chunks_at_line_break(s.len_lines());
        assert_eq!(chunks.next(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_at_line_break_04() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..34);

        let (mut chunks, _, _, _) = s.chunks_at_line_break(0);
        assert_eq!(chunks.next(), None);

        let (mut chunks, _, _, _) = s.chunks_at_line_break(0);
        assert_eq!(chunks.prev(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_reverse_01() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chunks();
        let mut stack = Vec::new();

        for _ in 0..8 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..8 {
            assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_reverse_02() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chunks_at_char(r.len_chars() / 3).0;
        let mut stack = Vec::new();

        for _ in 0..8 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..8 {
            assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_reverse_03() {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chunks_at_char(r.len_chars() / 3).0;
        let mut stack = Vec::new();

        itr.reverse();
        for _ in 0..8 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..8 {
            assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_reverse_04() {
        let mut itr = Chunks::from_str("a\n", false);

        assert_eq!(Some("a\n"), itr.next());
        assert_eq!(None, itr.next());
        itr.reverse();
        assert_eq!(Some("a\n"), itr.next());
        assert_eq!(None, itr.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        let mut s1_bytes = s1.bytes();
        let mut s2_bytes = s2.bytes();

        assert_eq!(s1, s2);
        assert_eq!(s1.byte(0), s2.as_bytes()[0]);

        assert_eq!(s1.len_bytes(), s2.len());
        assert_eq!(s1_bytes.len(), s2.len());

        for _ in 0..(s2.len() + 1) {
            assert_eq!(s1_bytes.next(), s2_bytes.next());
        }

        assert_eq!(s1_bytes.len(), 0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_sliced_reverse_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s = r.slice(s_start..s_end);

        let mut itr = s.bytes();
        let mut stack = Vec::new();
        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_at_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        let mut bytes_1 = s2.bytes();
        for i in 0..(s1.len_bytes() + 1) {
            let mut bytes_2 = s1.bytes_at(i);
            assert_eq!(bytes_1.next(), bytes_2.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_at_sliced_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);
        let mut bytes = s.bytes_at(s.len_bytes());
        assert_eq!(bytes.next(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_at_sliced_03() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        let mut bytes_1 = s1.bytes_at(s1.len_bytes());
        let mut bytes_2 = s2.bytes();
        while let Some(b) = bytes_2.next_back() {
            assert_eq!(bytes_1.prev(), Some(b));
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bytes_at_sliced_reverse_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s = r.slice(s_start..s_end);

        let mut itr = s.bytes_at(s.len_bytes() / 3);
        let mut stack = Vec::new();
        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        for (cr, ct) in s1.chars().zip(s2.chars()) {
            assert_eq!(cr, ct);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_sliced_reverse_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s = r.slice(s_start..s_end);

        let mut itr = s.chars();
        let mut stack = Vec::new();
        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_at_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        let mut chars_1 = s2.chars();
        for i in 0..(s1.len_chars() + 1) {
            let mut chars_2 = s1.chars_at(i);
            assert_eq!(chars_1.next(), chars_2.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_at_sliced_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);
        let mut chars = s.chars_at(s.len_chars());
        assert_eq!(chars.next(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_at_sliced_03() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s_start_char = r.char_to_byte(s_start);
        let s_end_char = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_char..s_end_char];

        let mut chars_1 = s1.chars_at(s1.len_chars());
        let mut chars_2 = s2.chars();
        while let Some(c) = chars_2.next_back() {
            assert_eq!(chars_1.prev(), Some(c));
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_at_sliced_reverse_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s = r.slice(s_start..s_end);

        let mut itr = s.chars_at(s.len_chars() / 3);
        let mut stack = Vec::new();
        for _ in 0..32 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..32 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        for (liner, linet) in s1.lines().zip(s2.lines()) {
            assert_eq!(liner.to_string().trim_end(), linet);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_sliced_reverse_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s = r.slice(s_start..s_end);

        let mut itr = s.lines();
        let mut stack = Vec::new();
        for _ in 0..4 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..4 {
            assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        let mut idx = 0;
        for chunk in s1.chunks() {
            assert_eq!(chunk, &s2[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }

        assert_eq!(idx, s2.len());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chunks_sliced_reverse_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 34;
        let s_end = 301;
        let s = r.slice(s_start..s_end);

        let mut itr = s.chunks();
        let mut stack = Vec::new();
        for _ in 0..8 {
            stack.push(itr.next().unwrap());
        }
        itr.reverse();
        for _ in 0..8 {
            assert_eq!(stack.pop(), itr.next());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn empty_iter() {
        let rope = Rope::from_str("");
        let r: Vec<_> = rope.lines().collect();
        assert_eq!(&[""], &*r)
    }
}
