#![allow(dead_code)]

use std;
use std::io;
use std::sync::Arc;
use std::ptr;

use iter::{Bytes, Chars, Chunks, Graphemes, Lines};
use rope_builder::RopeBuilder;
use slice::RopeSlice;
use str_utils::{char_idx_to_byte_idx, seam_is_grapheme_boundary};
use tree::{Count, Node, NodeChildren, MAX_BYTES};

/// A utf8 text rope.
///
/// `Rope`'s atomic unit of data is Unicode scalar values (or `char`s in Rust).
/// Except where otherwise documented, all methods that index into a rope
/// or return an index into a rope do so by `char` index.  This makes the API's
/// intuitive and prevents accidentally creating a Rope with invalid utf8 data.
///
/// The primary editing operations available for `Rope` are insertion of text,
/// deletion of text, splitting a `Rope` in two, and appending one `Rope` to
/// another.  For example:
///
/// ```
/// # use ropey::Rope;
/// #
/// let mut rope = Rope::from_str("Hello みんなさん!");
/// rope.remove(6, 11);
/// rope.insert(6, "world");
///
/// assert_eq!(rope, "Hello world!");
/// ```
///
/// Cloning `Rope`'s is extremely cheap, taking only a few instructions and
/// 8 bytes of memory, regardless of text size.  This is accomplished by data
/// sharing between `Rope` clones, and the memory used by clones only grows
/// incrementally as the their contents diverge due to edits.  All of this
/// is thread safe, and clones can be sent freely between threads.
///
/// `Rope` tracks line endings and has efficient API's for working with lines.
/// You can convert between `char` and line index, determining which line a
/// given `char` is on or the `char` index of the beginning of a line:
///
/// ```
/// # use ropey::Rope;
/// #
/// let rope = Rope::from_str("Hello individual!\nHow are you?\nThis text has multiple lines!");
///
/// assert_eq!(rope.char_to_line(5), 0);
/// assert_eq!(rope.char_to_line(21), 1);
///
/// assert_eq!(rope.line_to_char(0), 0);
/// assert_eq!(rope.line_to_char(1), 18);
/// assert_eq!(rope.line_to_char(2), 31);
/// ```
///
/// `Rope` is written to be fast and memory efficient.  All editing and
/// query operations execute in worst-case `O(log N)` time in the length of
/// the text.  (If they don't, file a bug!)  It is designed to work efficiently
/// even for huge (in the gigabytes) and pathological (all on one line)
/// texts.  It should be able to handle just about anything you can throw at it.
#[derive(Clone)]
pub struct Rope {
    pub(crate) root: Arc<Node>,
}

impl Rope {
    //-----------------------------------------------------------------------
    // Constructors

    /// Creates an empty `Rope`.
    pub fn new() -> Rope {
        Rope {
            root: Arc::new(Node::new()),
        }
    }

    /// Creates a `Rope` from a string slice.
    pub fn from_str(text: &str) -> Rope {
        let mut builder = RopeBuilder::new();
        builder.append(text);
        builder.finish()
    }

    /// Creates a `Rope` from the output of a reader.
    ///
    /// # Errors
    ///
    /// - If the reader returns an error, `from_reader` stops and returns
    ///   that error.
    /// - If non-utf8 data is encountered, an IO error with kind
    ///   `InvalidData` is returned.
    ///
    /// Note: some data from the reader is likely consumed even if there is
    /// an error.
    pub fn from_reader<T: io::Read>(mut reader: T) -> io::Result<Rope> {
        const BUFFER_SIZE: usize = MAX_BYTES * 2;
        let mut builder = RopeBuilder::new();
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut fill_idx = 0; // How much `buffer` is currently filled with valid data
        loop {
            match reader.read(&mut buffer[fill_idx..]) {
                Ok(read_count) => {
                    fill_idx += read_count;

                    // Determine how much of the buffer is valid utf8
                    let valid_count = match std::str::from_utf8(&buffer[..fill_idx]) {
                        Ok(_) => fill_idx,
                        Err(e) => e.valid_up_to(),
                    };

                    // Append the valid part of the buffer to the rope.
                    if valid_count > 0 {
                        builder.append(unsafe {
                            std::str::from_utf8_unchecked(&buffer[..valid_count])
                        });
                    }

                    // Shift the un-read part of the buffer to the beginning
                    if valid_count < fill_idx {
                        unsafe {
                            ptr::copy(
                                buffer.as_ptr().offset(valid_count as isize),
                                buffer.as_mut_ptr().offset(0),
                                fill_idx - valid_count,
                            );
                        }
                    }
                    fill_idx -= valid_count;

                    if fill_idx == BUFFER_SIZE {
                        // Buffer is full and none of it could be consumed.  Utf8
                        // codepoints don't get that large, so it's clearly not
                        // valid text.
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "stream did not contain valid UTF-8",
                        ));
                    }

                    // If we're done reading
                    if read_count == 0 {
                        if fill_idx > 0 {
                            // We couldn't consume all data.
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "stream did not contain valid UTF-8",
                            ));
                        } else {
                            return Ok(builder.finish());
                        }
                    }
                }

                Err(e) => {
                    // Read error
                    return Err(e);
                }
            }
        }
    }

    //-----------------------------------------------------------------------
    // Informational methods

    /// Total number of bytes in the `Rope`.
    pub fn len_bytes(&self) -> usize {
        self.root.byte_count()
    }

    /// Total number of chars in the `Rope`.
    pub fn len_chars(&self) -> usize {
        self.root.char_count()
    }

    /// Total number of lines in the `Rope`.
    pub fn len_lines(&self) -> usize {
        self.root.line_break_count() + 1
    }

    //-----------------------------------------------------------------------
    // Edit methods

    /// Inserts `text` at char index `char_idx`.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        // TODO: handle large insertions more efficiently, instead of doing a split
        // and appends.

        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to insert past end of Rope: insertion point {}, Rope length {}",
            char_idx,
            self.len_chars()
        );

        if text.len() <= MAX_BYTES {
            // Get root for mutation
            let root = Arc::make_mut(&mut self.root);

            // Do the insertion
            let (residual, seam) = root.insert(char_idx as Count, text);

            // Handle root splitting, if any.
            if let Some(r_node) = residual {
                let mut l_node = Node::new();
                std::mem::swap(&mut l_node, root);

                let mut children = NodeChildren::new();
                children.push((l_node.text_info(), Arc::new(l_node)));
                children.push((r_node.text_info(), Arc::new(r_node)));

                *root = Node::Internal(children);
            }

            // Handle seam, if any.
            if let Some(byte_pos) = seam {
                root.fix_grapheme_seam(byte_pos, true);
            }
        } else if self.root.is_leaf() && (self.root.text_info().bytes as usize <= MAX_BYTES) {
            let mut new_rope = Rope::from_str(text);
            {
                let orig_text = self.root.leaf_text();
                let byte_idx = char_idx_to_byte_idx(orig_text, char_idx);
                new_rope.insert(0, &orig_text[..byte_idx]);
                let end_idx = new_rope.root.text_info().chars as usize;
                new_rope.insert(end_idx, &orig_text[byte_idx..]);
            }
            *self = new_rope;
        } else {
            let text_rope = Rope::from_str(text);
            let right = self.split_off(char_idx);
            self.append(text_rope);
            self.append(right);
        }
    }

    /// Removes the text in char range `start..end`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `end` or `end` is out of bounds
    /// (i.e. `end > len_chars()`).
    pub fn remove(&mut self, start: usize, end: usize) {
        // Bounds check
        assert!(start <= end);
        assert!(
            end <= self.len_chars(),
            "Attempt to remove past end of Rope: removal end {}, Rope length {}",
            end,
            self.len_chars()
        );

        // Scope to contain borrow of root
        {
            let root = Arc::make_mut(&mut self.root);
            let (need_zip, seam) = root.remove(start, end);

            if need_zip {
                root.zip_fix(start);
            }
            if let Some(seam_idx) = seam {
                root.fix_grapheme_seam(seam_idx as Count, false);
            }
        }

        self.pull_up_singular_nodes();
    }

    /// Splits the `Rope` at `char_idx`, returning the right part of
    /// the split.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn split_off(&mut self, char_idx: usize) -> Rope {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to split past end of Rope: split point {}, Rope length {}",
            char_idx,
            self.len_chars()
        );

        if char_idx == 0 {
            // Special case 1
            let mut new_rope = Rope::new();
            std::mem::swap(self, &mut new_rope);
            new_rope
        } else if char_idx == self.len_chars() {
            // Special case 2
            Rope::new()
        } else {
            // Do the split
            let mut new_rope_root = Arc::new(Arc::make_mut(&mut self.root).split(char_idx));

            // Fix up the edges
            Arc::make_mut(&mut self.root).zip_fix_right();
            Arc::make_mut(&mut new_rope_root).zip_fix_left();
            self.pull_up_singular_nodes();

            while (!new_rope_root.is_leaf()) && new_rope_root.child_count() == 1 {
                let child = if let Node::Internal(ref children) = *new_rope_root {
                    Arc::clone(&children.nodes()[0])
                } else {
                    unreachable!()
                };

                new_rope_root = child;
            }

            // Return right rope
            Rope {
                root: new_rope_root,
            }
        }
    }

    /// Appends a `Rope` to the end of this one, consuming the other `Rope`.
    pub fn append(&mut self, other: Rope) {
        if self.len_chars() == 0 {
            let mut other = other;
            std::mem::swap(self, &mut other);
        } else if other.len_chars() > 0 {
            let seam_byte_i = self.root.text_info().bytes;

            let l_depth = self.root.depth();
            let r_depth = other.root.depth();

            if l_depth > r_depth {
                let extra =
                    Arc::make_mut(&mut self.root).append_at_depth(other.root, l_depth - r_depth);
                if let Some(node) = extra {
                    let mut children = NodeChildren::new();
                    children.push((self.root.text_info(), Arc::clone(&self.root)));
                    children.push((node.text_info(), node));
                    self.root = Arc::new(Node::Internal(children));
                }
            } else {
                let mut other = other;
                let extra = Arc::make_mut(&mut other.root)
                    .prepend_at_depth(Arc::clone(&self.root), r_depth - l_depth);
                if let Some(node) = extra {
                    let mut children = NodeChildren::new();
                    children.push((node.text_info(), node));
                    children.push((other.root.text_info(), Arc::clone(&other.root)));
                    other.root = Arc::new(Node::Internal(children));
                }
                *self = other;
            };

            Arc::make_mut(&mut self.root).fix_grapheme_seam(seam_byte_i, true);
        }
    }

    //-----------------------------------------------------------------------
    // Index conversion methods

    /// Returns the char index of the given byte.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    pub(crate) fn byte_to_char(&self, byte_idx: usize) -> usize {
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        self.root.byte_to_char(byte_idx)
    }

    /// Returns the line index of the given byte.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.
    /// - If `byte_idx` is one-past-the-end, then one-past-the-end line index
    ///   is returned.  This is mainly unintuitive for empty ropes, which will
    ///   return a line index of 1 for a `byte_idx` of zero.  Otherwise it
    ///   behaves as expected.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    pub(crate) fn byte_to_line(&self, byte_idx: usize) -> usize {
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        if byte_idx == self.len_bytes() {
            self.len_lines()
        } else {
            self.root.byte_to_line(byte_idx)
        }
    }

    /// Returns the byte index of the given char.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub(crate) fn char_to_byte(&self, char_idx: usize) -> usize {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        self.root.char_to_byte(char_idx) as usize
    }

    /// Returns the line index of the given char.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.
    /// - If `char_idx` is one-past-the-end, then one-past-the-end line index
    ///   is returned.  This is mainly unintuitive for empty ropes, which will
    ///   return a line index of 1 for a `char_idx` of zero.  Otherwise it
    ///   behaves as expected.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        if char_idx == self.len_chars() {
            self.len_lines()
        } else {
            self.root.char_to_line(char_idx)
        }
    }

    /// Returns the byte index of the start of the given line.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.
    /// - `line_idx` can be one-past-the-end, which will return one-past-the-end
    ///   byte index.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx` is out of bounds (i.e. `line_idx > len_lines()`).
    pub(crate) fn line_to_byte(&self, line_idx: usize) -> usize {
        // Bounds check
        assert!(
            line_idx <= self.len_lines(),
            "Attempt to index past end of Rope: line index {}, Rope line length {}",
            line_idx,
            self.len_lines()
        );

        if line_idx == self.len_lines() {
            self.len_bytes()
        } else {
            self.root.line_to_byte(line_idx)
        }
    }

    /// Returns the char index of the start of the given line.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.
    /// - `line_idx` can be one-past-the-end, which will return one-past-the-end
    ///   char index.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx` is out of bounds (i.e. `line_idx > len_lines()`).
    pub fn line_to_char(&self, line_idx: usize) -> usize {
        // Bounds check
        assert!(
            line_idx <= self.len_lines(),
            "Attempt to index past end of Rope: line index {}, Rope line length {}",
            line_idx,
            self.len_lines()
        );

        if line_idx == self.len_lines() {
            self.len_chars()
        } else {
            self.root.line_to_char(line_idx)
        }
    }

    //-----------------------------------------------------------------------
    // Fetch methods

    /// Returns the char at `char_idx`.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx >= len_chars()`).
    pub fn char(&self, char_idx: usize) -> char {
        // Bounds check
        assert!(
            char_idx < self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        // TODO: make this more efficient.
        self.slice(char_idx, char_idx + 1).chars().nth(0).unwrap()
    }

    /// Returns the line at `line_idx`.
    ///
    /// Note: lines are zero-indexed.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx` is out of bounds (i.e. `line_idx >= len_lines()`).
    pub fn line(&self, line_idx: usize) -> RopeSlice {
        // Bounds check
        assert!(
            line_idx < self.len_lines(),
            "Attempt to index past end of Rope: line index {}, Rope line length {}",
            line_idx,
            self.len_lines()
        );

        let start = self.line_to_char(line_idx);
        let end = self.line_to_char(line_idx + 1);

        self.slice(start, end)
    }

    //-----------------------------------------------------------------------
    // Grapheme methods

    /// Returns whether `char_idx` is a grapheme cluster boundary or not.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn is_grapheme_boundary(&self, char_idx: usize) -> bool {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        self.root.is_grapheme_boundary(char_idx)
    }

    /// Returns the char index of the grapheme cluster boundary to the left
    /// of `char_idx`.
    ///
    /// This excludes any boundary that might be at `char_idx` itself, unless
    /// `char_idx` is at the beginning of the rope.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn prev_grapheme_boundary(&self, char_idx: usize) -> usize {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        self.root.prev_grapheme_boundary(char_idx)
    }

    /// Returns the char index of the grapheme cluster boundary to the right
    /// of `char_idx`.
    ///
    /// This excludes any boundary that might be at `char_idx` itself, unless
    /// `char_idx` is at the end of the rope.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn next_grapheme_boundary(&self, char_idx: usize) -> usize {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        self.root.next_grapheme_boundary(char_idx)
    }

    //-----------------------------------------------------------------------
    // Slicing

    /// Returns an immutable slice of the `Rope` in the char range `start..end`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `end` or `end` is out of bounds
    /// (i.e. `end > len_chars()`).
    pub fn slice(&self, start: usize, end: usize) -> RopeSlice {
        // Bounds check
        assert!(start <= end);
        assert!(
            end <= self.len_chars(),
            "Attempt to slice past end of Rope: slice end {}, Rope length {}",
            end,
            self.len_chars()
        );

        RopeSlice::new_with_range(&self.root, start, end)
    }

    //-----------------------------------------------------------------------
    // Iterator methods

    /// Creates an iterator over the bytes of the `Rope`.
    pub fn bytes(&self) -> Bytes {
        Bytes::new(&self.root)
    }

    /// Creates an iterator over the chars of the `Rope`.
    pub fn chars(&self) -> Chars {
        Chars::new(&self.root)
    }

    /// Creates an iterator over the grapheme clusters of the `Rope`.
    pub fn graphemes(&self) -> Graphemes {
        Graphemes::new(&self.root, true)
    }

    /// Creates an iterator over the lines of the `Rope`.
    pub fn lines(&self) -> Lines {
        Lines::new(&self.root)
    }

    /// Creates an iterator over the chunks of the `Rope`.
    pub fn chunks(&self) -> Chunks {
        Chunks::new(&self.root)
    }

    //-----------------------------------------------------------------------
    // Conversion methods

    /// Returns the entire text of the `Rope` as a newly allocated String.
    pub fn to_string(&self) -> String {
        use iter::Chunks;
        let mut text = String::with_capacity(self.len_bytes());
        for chunk in Chunks::new(&self.root) {
            text.push_str(chunk);
        }
        text
    }

    /// Returns a slice to the entire contents of the `Rope`.
    ///
    /// Mainly just a convenience method, since the `RangeArgument` trait
    /// isn't stabilized yet.
    pub fn to_slice(&self) -> RopeSlice {
        self.slice(0, self.len_chars())
    }

    //-----------------------------------------------------------------------
    // Debugging

    /// Debugging tool to make sure that all of the meta-data of the
    /// tree is consistent with the actual data.
    #[doc(hidden)]
    pub fn assert_integrity(&self) {
        self.root.assert_integrity();
    }

    /// Debugging tool to make sure that all of the following invariants
    /// hold true throughout the tree:
    ///
    /// - The tree is the same height everywhere.
    /// - All internal nodes have the minimum number of children.
    /// - All leaf nodes are non-empty.
    /// - Graphemes are never split over chunk boundaries.
    #[doc(hidden)]
    pub fn assert_invariants(&self) {
        self.root.assert_balance();
        self.root.assert_node_size(true);
        self.assert_grapheme_seams();
    }

    /// Checks that graphemes are never split over chunk boundaries.
    fn assert_grapheme_seams(&self) {
        if self.chunks().count() > 0 {
            let mut itr = self.chunks();
            let mut last_chunk = itr.next().unwrap();
            for chunk in itr {
                if !chunk.is_empty() && !last_chunk.is_empty() {
                    assert!(seam_is_grapheme_boundary(last_chunk, chunk));
                    last_chunk = chunk;
                } else if last_chunk.is_empty() {
                    last_chunk = chunk;
                }
            }
        }
    }

    /// A for-fun tool for playing with silly text files.
    #[doc(hidden)]
    pub fn largest_grapheme_size(&self) -> usize {
        let mut size = 0;
        for g in self.graphemes() {
            if g.len() > size {
                size = g.len();
            }
        }
        size
    }

    //-----------------------------------------------------------------------
    // Internal utilities

    /// Iteratively replaced the root node with its child if it only has
    /// one child.
    pub(crate) fn pull_up_singular_nodes(&mut self) {
        while (!self.root.is_leaf()) && self.root.child_count() == 1 {
            let child = if let Node::Internal(ref children) = *self.root {
                Arc::clone(&children.nodes()[0])
            } else {
                unreachable!()
            };

            self.root = child;
        }
    }
}

//==============================================================

impl std::fmt::Debug for Rope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list().entries(self.chunks()).finish()
    }
}

impl std::fmt::Display for Rope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for chunk in self.chunks() {
            write!(f, "{}", chunk)?
        }
        Ok(())
    }
}

impl std::default::Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> std::cmp::PartialEq<Rope> for Rope {
    fn eq(&self, other: &Rope) -> bool {
        self.to_slice() == other.to_slice()
    }
}

impl<'a> std::cmp::PartialEq<&'a str> for Rope {
    fn eq(&self, other: &&'a str) -> bool {
        self.to_slice() == *other
    }
}

impl<'a> std::cmp::PartialEq<Rope> for &'a str {
    fn eq(&self, other: &Rope) -> bool {
        *self == other.to_slice()
    }
}

//==============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 127 bytes, 103 chars, 1 line
    const TEXT: &str = "Hello there!  How're you doing?  It's \
                        a fine day, isn't it?  Aren't you glad \
                        we're alive?  こんにちは、みんなさん！";
    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    #[test]
    fn new_01() {
        let r = Rope::new();
        assert_eq!(r, "");

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn from_str() {
        let r = Rope::from_str(TEXT);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn len_bytes_01() {
        let r = Rope::from_str(TEXT);
        assert_eq!(r.len_bytes(), 127);
    }

    #[test]
    fn len_bytes_02() {
        let r = Rope::from_str("");
        assert_eq!(r.len_bytes(), 0);
    }

    #[test]
    fn len_chars_01() {
        let r = Rope::from_str(TEXT);
        assert_eq!(r.len_chars(), 103);
    }

    #[test]
    fn len_chars_02() {
        let r = Rope::from_str("");
        assert_eq!(r.len_chars(), 0);
    }

    #[test]
    fn len_lines_01() {
        let r = Rope::from_str(TEXT_LINES);
        assert_eq!(r.len_lines(), 4);
    }

    #[test]
    fn len_lines_02() {
        let r = Rope::from_str("");
        assert_eq!(r.len_lines(), 1);
    }

    #[test]
    fn insert_01() {
        let mut r = Rope::from_str(TEXT);
        r.insert(3, "AA");

        assert_eq!(
            r,
            "HelAAlo there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn insert_02() {
        let mut r = Rope::from_str(TEXT);
        r.insert(0, "AA");

        assert_eq!(
            r,
            "AAHello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn insert_03() {
        let mut r = Rope::from_str(TEXT);
        r.insert(103, "AA");

        assert_eq!(
            r,
            "Hello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！AA"
        );

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn insert_04() {
        let mut r = Rope::new();
        r.insert(0, "He");
        r.insert(2, "l");
        r.insert(3, "l");
        r.insert(4, "o w");
        r.insert(7, "o");
        r.insert(8, "rl");
        r.insert(10, "d!");
        r.insert(3, "zopter");

        assert_eq!("Helzopterlo world!", r);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn insert_05() {
        let mut r = Rope::new();
        r.insert(0, "こんいちは、みんなさん！");
        r.insert(7, "zopter");
        assert_eq!("こんいちは、みzopterんなさん！", r);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn insert_06() {
        let mut r = Rope::new();
        r.insert(0, "こ");
        r.insert(1, "ん");
        r.insert(2, "い");
        r.insert(3, "ち");
        r.insert(4, "は");
        r.insert(5, "、");
        r.insert(6, "み");
        r.insert(7, "ん");
        r.insert(8, "な");
        r.insert(9, "さ");
        r.insert(10, "ん");
        r.insert(11, "！");
        r.insert(7, "zopter");
        assert_eq!("こんいちは、みzopterんなさん！", r);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn remove_01() {
        let mut r = Rope::from_str(TEXT);

        r.remove(5, 11);
        r.remove(24, 31);
        r.remove(19, 25);
        r.remove(75, 79);
        assert_eq!(
            r,
            "Hello!  How're you \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにんなさん！"
        );

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn remove_02() {
        let mut r = Rope::from_str("\r\n\r\n\r\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");

        // Make sure graphemes get merged properly, via
        // assert_invariants() below.
        r.remove(3, 6);
        assert_eq!(r, "\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn remove_03() {
        let mut r = Rope::from_str(TEXT);

        // Make sure graphemes get merged properly
        r.remove(45, 45);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    #[should_panic]
    fn remove_04() {
        let mut r = Rope::from_str(TEXT);
        r.remove(56, 55); // Wrong ordering of start/end
    }

    #[test]
    #[should_panic]
    fn remove_05() {
        let mut r = Rope::from_str(TEXT);
        r.remove(102, 104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_06() {
        let mut r = Rope::from_str(TEXT);
        r.remove(103, 104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_07() {
        let mut r = Rope::from_str(TEXT);
        r.remove(104, 104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_08() {
        let mut r = Rope::from_str(TEXT);
        r.remove(104, 105); // Removing past the end
    }

    #[test]
    fn split_off_01() {
        let mut r = Rope::from_str(TEXT);

        let r2 = r.split_off(50);
        assert_eq!(
            r,
            "Hello there!  How're you doing?  It's \
             a fine day, "
        );
        assert_eq!(
            r2,
            "isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_integrity();
        r2.assert_integrity();
        r.assert_invariants();
        r2.assert_invariants();
    }

    #[test]
    fn split_off_02() {
        let mut r = Rope::from_str(TEXT);

        let r2 = r.split_off(1);
        assert_eq!(r, "H");
        assert_eq!(
            r2,
            "ello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_integrity();
        r2.assert_integrity();
        r.assert_invariants();
        r2.assert_invariants();
    }

    #[test]
    fn split_off_03() {
        let mut r = Rope::from_str(TEXT);

        let r2 = r.split_off(102);
        assert_eq!(
            r,
            "Hello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん"
        );
        assert_eq!(r2, "！");

        r.assert_integrity();
        r2.assert_integrity();
        r.assert_invariants();
        r2.assert_invariants();
    }

    #[test]
    fn split_off_04() {
        let mut r = Rope::from_str(TEXT);

        let r2 = r.split_off(0);
        assert_eq!(r, "");
        assert_eq!(
            r2,
            "Hello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_integrity();
        r2.assert_integrity();
        r.assert_invariants();
        r2.assert_invariants();
    }

    #[test]
    fn split_off_05() {
        let mut r = Rope::from_str(TEXT);

        let r2 = r.split_off(103);
        assert_eq!(
            r,
            "Hello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );
        assert_eq!(r2, "");

        r.assert_integrity();
        r2.assert_integrity();
        r.assert_invariants();
        r2.assert_invariants();
    }

    #[test]
    #[should_panic]
    fn split_off_06() {
        let mut r = Rope::from_str(TEXT);
        r.split_off(104); // One past the end of the rope
    }

    #[test]
    fn append_01() {
        let mut r = Rope::from_str(
            "Hello there!  How're you doing?  It's \
             a fine day, isn't ",
        );
        let r2 = Rope::from_str(
            "it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！",
        );

        r.append(r2);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn append_02() {
        let mut r = Rope::from_str(
            "Hello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんに",
        );
        let r2 = Rope::from_str("ちは、みんなさん！");

        r.append(r2);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn append_03() {
        let mut r = Rope::from_str(
            "Hello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん",
        );
        let r2 = Rope::from_str("！");

        r.append(r2);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn append_04() {
        let mut r = Rope::from_str("H");
        let r2 = Rope::from_str(
            "ello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！",
        );

        r.append(r2);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn append_05() {
        let mut r = Rope::from_str(TEXT);
        let r2 = Rope::from_str("");

        r.append(r2);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn append_06() {
        let mut r = Rope::from_str("");
        let r2 = Rope::from_str(TEXT);

        r.append(r2);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn char_to_line_01() {
        let r = Rope::from_str(TEXT_LINES);

        assert_eq!(0, r.char_to_line(0));
        assert_eq!(0, r.char_to_line(1));

        assert_eq!(0, r.char_to_line(31));
        assert_eq!(1, r.char_to_line(32));
        assert_eq!(1, r.char_to_line(33));

        assert_eq!(1, r.char_to_line(58));
        assert_eq!(2, r.char_to_line(59));
        assert_eq!(2, r.char_to_line(60));

        assert_eq!(2, r.char_to_line(87));
        assert_eq!(3, r.char_to_line(88));
        assert_eq!(3, r.char_to_line(89));

        assert_eq!(4, r.char_to_line(100));
    }

    #[test]
    fn char_to_line_02() {
        let r = Rope::from_str("");
        assert_eq!(1, r.char_to_line(0));
    }

    #[test]
    #[should_panic]
    fn char_to_line_03() {
        let r = Rope::from_str(TEXT_LINES);
        r.char_to_line(101);
    }

    #[test]
    fn line_to_char_01() {
        let r = Rope::from_str(TEXT_LINES);

        assert_eq!(0, r.line_to_char(0));
        assert_eq!(32, r.line_to_char(1));
        assert_eq!(59, r.line_to_char(2));
        assert_eq!(88, r.line_to_char(3));
        assert_eq!(100, r.line_to_char(4));
    }

    #[test]
    fn line_to_char_02() {
        let r = Rope::from_str("");
        assert_eq!(0, r.line_to_char(0));
        assert_eq!(0, r.line_to_char(1));
    }

    #[test]
    #[should_panic]
    fn line_to_char_03() {
        let r = Rope::from_str(TEXT_LINES);
        r.line_to_char(5);
    }

    #[test]
    fn char_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(r.char(0), 'H');
        assert_eq!(r.char(10), 'e');
        assert_eq!(r.char(18), 'r');
        assert_eq!(r.char(102), '！');
    }

    #[test]
    #[should_panic]
    fn char_02() {
        let r = Rope::from_str(TEXT);
        r.char(103);
    }

    #[test]
    #[should_panic]
    fn char_03() {
        let r = Rope::from_str("");
        r.char(0);
    }

    #[test]
    fn line_01() {
        let r = Rope::from_str(TEXT_LINES);

        assert_eq!(r.line(0), "Hello there!  How're you doing?\n");
        assert_eq!(r.line(1), "It's a fine day, isn't it?\n");
        assert_eq!(r.line(2), "Aren't you glad we're alive?\n");
        assert_eq!(r.line(3), "こんにちは、みんなさん！");
    }

    #[test]
    fn line_02() {
        let r = Rope::from_str("Hello there!  How're you doing?\n");

        assert_eq!(r.line(0), "Hello there!  How're you doing?\n");
        assert_eq!(r.line(1), "");
    }

    #[test]
    fn line_03() {
        let r = Rope::from_str("");

        assert_eq!(r.line(0), "");
    }

    #[test]
    #[should_panic]
    fn line_04() {
        let r = Rope::from_str(TEXT_LINES);
        r.line(4);
    }

    #[test]
    fn is_grapheme_boundary_01() {
        let r = Rope::from_str(
            "Hello there!  How're you doing?\r\n\
             It's a fine day, isn't it?",
        );

        assert!(r.is_grapheme_boundary(0));
        assert!(r.is_grapheme_boundary(1));
        assert!(r.is_grapheme_boundary(31));
        assert!(!r.is_grapheme_boundary(32));
        assert!(r.is_grapheme_boundary(33));
        assert!(r.is_grapheme_boundary(58));
        assert!(r.is_grapheme_boundary(59));
    }

    #[test]
    #[should_panic]
    fn is_grapheme_boundary_02() {
        let r = Rope::from_str(
            "Hello there!  How're you doing?\r\n\
             It's a fine day, isn't it?",
        );
        r.is_grapheme_boundary(60);
    }

    #[test]
    fn prev_grapheme_boundary_01() {
        let r = Rope::from_str(
            "Hello there!  How're you doing?\r\n\
             It's a fine day, isn't it?",
        );

        assert_eq!(0, r.prev_grapheme_boundary(0));
        assert_eq!(0, r.prev_grapheme_boundary(1));
        assert_eq!(30, r.prev_grapheme_boundary(31));
        assert_eq!(31, r.prev_grapheme_boundary(32));
        assert_eq!(31, r.prev_grapheme_boundary(33));
        assert_eq!(57, r.prev_grapheme_boundary(58));
        assert_eq!(58, r.prev_grapheme_boundary(59));
    }

    #[test]
    #[should_panic]
    fn prev_grapheme_boundary_02() {
        let r = Rope::from_str(
            "Hello there!  How're you doing?\r\n\
             It's a fine day, isn't it?",
        );
        r.prev_grapheme_boundary(60);
    }

    #[test]
    fn next_grapheme_boundary_01() {
        let r = Rope::from_str(
            "Hello there!  How're you doing?\r\n\
             It's a fine day, isn't it?",
        );

        assert_eq!(1, r.next_grapheme_boundary(0));
        assert_eq!(2, r.next_grapheme_boundary(1));
        assert_eq!(33, r.next_grapheme_boundary(31));
        assert_eq!(33, r.next_grapheme_boundary(32));
        assert_eq!(34, r.next_grapheme_boundary(33));
        assert_eq!(59, r.next_grapheme_boundary(58));
        assert_eq!(59, r.next_grapheme_boundary(59));
    }

    #[test]
    #[should_panic]
    fn next_grapheme_boundary_02() {
        let r = Rope::from_str(
            "Hello there!  How're you doing?\r\n\
             It's a fine day, isn't it?",
        );
        r.next_grapheme_boundary(60);
    }

    // Slicing and iterator tests are in their respective modules
}
