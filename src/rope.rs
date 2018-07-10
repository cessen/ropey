use std;
use std::io;
use std::ptr;
use std::sync::Arc;

use crlf;
use iter::{Bytes, Chars, Chunks, Lines};
use rope_builder::RopeBuilder;
use slice::{CharIdxRange, RopeSlice};
use str_utils::char_idx_to_byte_idx;
use tree::{Count, Node, NodeChildren, TextInfo, MAX_BYTES};

/// A utf8 text rope.
///
/// The primary editing operations available for `Rope` are insertion of text,
/// deletion of text, splitting a `Rope` in two, and appending one `Rope` to
/// another.  For example:
///
/// ```
/// # use ropey::Rope;
/// #
/// let mut rope = Rope::from_str("Hello みんなさん!");
/// rope.remove(6..11);
/// rope.insert(6, "world");
///
/// assert_eq!(rope, "Hello world!");
/// ```
///
/// Cloning `Rope`'s is extremely cheap, taking only a few instructions and
/// 8 bytes of memory, regardless of text size.  This is accomplished by data
/// sharing between `Rope` clones.  The memory used by clones only grows
/// incrementally as the their contents diverge due to edits.  All of this
/// is thread safe, so clones can be sent freely between threads.
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
/// `Rope` is written to be fast and memory efficient.  Except where otherwise
/// documented, all editing and query operations execute in worst-case
/// `O(log N)` time in the length of the rope.  It is designed to work
/// efficiently even for huge (in the gigabytes) and pathological (all on one
/// line) texts.  It should be able to handle just about anything you can throw
/// at it.
#[derive(Clone)]
pub struct Rope {
    pub(crate) root: Arc<Node>,
}

impl Rope {
    //-----------------------------------------------------------------------
    // Constructors

    /// Creates an empty `Rope`.
    pub fn new() -> Self {
        Rope {
            root: Arc::new(Node::new()),
        }
    }

    /// Creates a `Rope` from a string slice.
    ///
    /// Runs in O(N) time.
    pub fn from_str(text: &str) -> Self {
        RopeBuilder::new().build_at_once(text)
    }

    /// Creates a `Rope` from the output of a reader.
    ///
    /// Runs in O(N) time.
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
    #[allow(unused_mut)]
    pub fn from_reader<T: io::Read>(mut reader: T) -> io::Result<Self> {
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
    ///
    /// Runs in O(1) time.
    pub fn len_bytes(&self) -> usize {
        self.root.byte_count()
    }

    /// Total number of chars in the `Rope`.
    ///
    /// Runs in O(1) time.
    pub fn len_chars(&self) -> usize {
        self.root.char_count()
    }

    /// Total number of lines in the `Rope`.
    ///
    /// Runs in O(1) time.
    pub fn len_lines(&self) -> usize {
        self.root.line_break_count() + 1
    }

    //-----------------------------------------------------------------------
    // Memory management methods

    /// Total size of the `Rope`'s text buffer space, in bytes.
    ///
    /// This includes unoccupied text buffer space.  You can calculate
    /// the unoccupied space with `capacity() - len_bytes()`.  In general,
    /// there will always be some unoccupied buffer space.
    ///
    /// Runs in O(N) time.
    pub fn capacity(&self) -> usize {
        let mut byte_count = 0;
        for chunk in self.chunks() {
            byte_count += chunk.len().max(MAX_BYTES);
        }
        byte_count
    }

    /// Shrinks the `Rope`'s capacity to the minimum possible.
    ///
    /// This will rarely result in `capacity() == len_bytes()`.  `Rope`
    /// stores text in a sequence of fixed-capacity chunks, so an exact fit
    /// only happens for texts that are both a precise multiple of that
    /// capacity _and_ have code point boundaries that line up exactly with
    /// the capacity boundaries.
    ///
    /// After calling this, the difference between `capacity()` and
    /// `len_bytes()` is typically under 1KB per megabyte of text in the
    /// `Rope`.
    ///
    /// **NOTE:** calling this on a `Rope` clone causes it to stop sharing
    /// all data with its other clones.  In such cases you will very likely
    /// be _increasing_ total memory usage despite shrinking the `Rope`'s
    /// capacity.
    ///
    /// Runs in O(N) time, and uses O(log N) additional space during
    /// shrinking.
    pub fn shrink_to_fit(&mut self) {
        let mut node_stack = Vec::new();
        let mut builder = RopeBuilder::new();

        node_stack.push(self.root.clone());
        *self = Rope::new();

        loop {
            if node_stack.is_empty() {
                break;
            }

            if node_stack.last().unwrap().is_leaf() {
                builder.append(node_stack.last().unwrap().leaf_text());
                node_stack.pop();
            } else if node_stack.last().unwrap().child_count() == 0 {
                node_stack.pop();
            } else {
                let (_, next_node) = Arc::make_mut(node_stack.last_mut().unwrap())
                    .children()
                    .remove(0);
                node_stack.push(next_node);
            }
        }

        *self = builder.finish();
    }

    //-----------------------------------------------------------------------
    // Edit methods

    /// Inserts `text` at char index `char_idx`.
    ///
    /// Runs in O(M + log N) time, where N is the length of the `Rope` and M
    /// is the length of `text`.
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

        if text.len() > MAX_BYTES * 6 {
            // For huge insertion texts, build a tree out of it and then
            // split and join.
            let text_rope = Rope::from_str(text);
            let right = self.split_off(char_idx);
            self.append(text_rope);
            self.append(right);
        } else {
            // Otherwise, for small-to-medium sized inserts, iteratively insert in
            // chunks.
            let mut text = text;
            while text.len() > 0 {
                let split_idx = crlf::find_good_split(
                    text.len() - (MAX_BYTES - 4).min(text.len()),
                    text.as_bytes(),
                    false,
                );
                let ins_text = &text[split_idx..];
                text = &text[..split_idx];

                // Do the insertion
                let mut seam = None;
                let (l_info, residual) = Arc::make_mut(&mut self.root).edit_char_range(
                    char_idx,
                    char_idx,
                    |acc_info, cur_info, leaf_text| {
                        debug_assert!(acc_info.chars as usize <= char_idx);
                        let byte_idx =
                            char_idx_to_byte_idx(leaf_text, char_idx - acc_info.chars as usize);
                        if byte_idx == 0 {
                            seam = Some(acc_info.bytes);
                        } else if byte_idx == leaf_text.len() {
                            let count = (leaf_text.len() + ins_text.len()) as Count;
                            seam = Some(acc_info.bytes + count)
                        } else {
                            seam = None
                        }

                        if (leaf_text.len() + ins_text.len()) <= MAX_BYTES {
                            // Calculate new info without doing a full re-scan of cur_text
                            let new_info = {
                                // Get summed info of current text and to-be-inserted text
                                let mut info = cur_info + TextInfo::from_str(ins_text);
                                // Check for CRLF graphemes on the insertion seams, and
                                // adjust line break counts accordingly
                                if !ins_text.is_empty() {
                                    if byte_idx > 0
                                        && leaf_text.as_bytes()[byte_idx - 1] == 0x0D
                                        && ins_text.as_bytes()[0] == 0x0A
                                    {
                                        info.line_breaks -= 1;
                                    }
                                    if byte_idx < leaf_text.len()
                                        && *ins_text.as_bytes().last().unwrap() == 0x0D
                                        && leaf_text.as_bytes()[byte_idx] == 0x0A
                                    {
                                        info.line_breaks -= 1;
                                    }
                                    if byte_idx > 0
                                        && byte_idx < leaf_text.len()
                                        && leaf_text.as_bytes()[byte_idx - 1] == 0x0D
                                        && leaf_text.as_bytes()[byte_idx] == 0x0A
                                    {
                                        info.line_breaks += 1;
                                    }
                                }
                                info
                            };
                            // Insert the text and return the new info
                            leaf_text.insert_str(byte_idx, ins_text);
                            return (new_info, None);
                        } else {
                            let r_text = leaf_text.insert_str_split(byte_idx, ins_text);
                            if r_text.len() > 0 {
                                return (
                                    TextInfo::from_str(leaf_text),
                                    Some((TextInfo::from_str(&r_text), r_text)),
                                );
                            } else {
                                // Leaf couldn't be validly split, so leave it oversized
                                return (TextInfo::from_str(leaf_text), None);
                            }
                        }
                    },
                );

                // Handle root splitting, if any.
                if let Some((r_info, r_node)) = residual {
                    let mut l_node = Arc::new(Node::new());
                    std::mem::swap(&mut l_node, &mut self.root);

                    let mut children = NodeChildren::new();
                    children.push((l_info, l_node));
                    children.push((r_info, r_node));

                    *Arc::make_mut(&mut self.root) = Node::Internal(children);
                }

                // Handle seam, if any.
                if let Some(byte_pos) = seam {
                    Arc::make_mut(&mut self.root).fix_grapheme_seam(byte_pos, true);
                }
            }
        }
    }

    /// Inserts a single char `ch` at char index `char_idx`.
    ///
    /// Runs in O(log N) time, where N is the length of the `Rope`.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    #[inline]
    pub fn insert_char(&mut self, char_idx: usize, ch: char) {
        let mut buf = [0u8; 4];
        self.insert(char_idx, ch.encode_utf8(&mut buf));
    }

    /// Removes the text in the given char index range.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.  The range is in `char`
    /// indices.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::Rope;
    /// let mut rope = Rope::from_str("Hello world!");
    /// rope.remove(5..);
    ///
    /// assert_eq!("Hello", rope);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the start of the range is greater than the end, or if the
    /// end is out of bounds (i.e. `end > len_chars()`).
    pub fn remove<R: CharIdxRange>(&mut self, range: R) {
        let start = range.start().unwrap_or(0);
        let end = range.end().unwrap_or_else(|| self.len_chars());

        // Bounds check
        assert!(start <= end);
        assert!(
            end <= self.len_chars(),
            "Attempt to remove past end of Rope: removal end {}, Rope length {}",
            end,
            self.len_chars()
        );

        // A special case that the rest of the logic doesn't handle
        // correctly.
        if start == 0 && end == self.len_chars() {
            self.root = Arc::new(Node::new());
            return;
        }

        // Scope to contain borrow of root
        {
            let root = Arc::make_mut(&mut self.root);
            let mut seam = None;

            let (_text_info, _residual) = {
                root.edit_char_range(start, end, |acc_info, cur_info, leaf_text| {
                    let local_start = start - (acc_info.chars as usize).min(start);
                    let local_end = (end - acc_info.chars as usize).min(cur_info.chars as usize);
                    let byte_start = char_idx_to_byte_idx(leaf_text, local_start);
                    let byte_end = char_idx_to_byte_idx(leaf_text, local_end);

                    if local_start == 0 || local_end == cur_info.chars as usize {
                        seam = Some(acc_info.bytes as usize + byte_start);
                    }

                    // Remove text and calculate new info
                    let new_info = if (byte_end - byte_start) < leaf_text.len() {
                        let rem_info = TextInfo::from_str(&leaf_text[byte_start..byte_end]);
                        let mut info = cur_info - rem_info;

                        // Check for CRLF graphemes on the insertion seams, and
                        // adjust line break counts accordingly
                        if byte_start != byte_end {
                            if byte_start > 0
                                && leaf_text.as_bytes()[byte_start - 1] == 0x0D
                                && leaf_text.as_bytes()[byte_start] == 0x0A
                            {
                                info.line_breaks += 1;
                            }
                            if byte_end < leaf_text.len()
                                && leaf_text.as_bytes()[byte_end - 1] == 0x0D
                                && leaf_text.as_bytes()[byte_end] == 0x0A
                            {
                                info.line_breaks += 1;
                            }
                            if byte_start > 0
                                && byte_end < leaf_text.len()
                                && leaf_text.as_bytes()[byte_start - 1] == 0x0D
                                && leaf_text.as_bytes()[byte_end] == 0x0A
                            {
                                info.line_breaks -= 1;
                            }
                        }

                        // Remove the text
                        leaf_text.remove_range(byte_start, byte_end);

                        info
                    } else {
                        // Remove the text
                        leaf_text.remove_range(byte_start, byte_end);

                        TextInfo::from_str(leaf_text)
                    };

                    (new_info, None)
                })
            };

            if let Some(seam_idx) = seam {
                root.fix_grapheme_seam(seam_idx as Count, false);
            }
            root.zip_fix(start);
        }

        self.pull_up_singular_nodes();
    }

    /// Splits the `Rope` at `char_idx`, returning the right part of
    /// the split.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn split_off(&mut self, char_idx: usize) -> Self {
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
    pub fn append(&mut self, other: Self) {
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
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
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
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    pub fn byte_to_line(&self, byte_idx: usize) -> usize {
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        self.root.byte_to_line(byte_idx)
    }

    /// Returns the byte index of the given char.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        self.root.char_to_byte(char_idx)
    }

    /// Returns the line index of the given char.
    ///
    /// Notes:
    ///
    /// - Lines are zero-indexed.
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

        self.root.char_to_line(char_idx)
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
    pub fn line_to_byte(&self, line_idx: usize) -> usize {
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

        let (chunk, _, chunk_char_idx) = self.root.get_chunk_at_char(char_idx);
        let byte_idx = char_idx_to_byte_idx(chunk, char_idx - chunk_char_idx);
        chunk[byte_idx..].chars().nth(0).unwrap()
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

        self.slice(start..end)
    }

    /// Returns the chunk containing the given byte index, along with
    /// the byte and char indices of the beginning of the chunk.
    ///
    /// The return value is organized as `(chunk, chunk_byte_idx, chunk_char_idx)`.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    pub fn chunk_at_byte(&self, byte_idx: usize) -> (&str, usize, usize) {
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        self.root.get_chunk_at_byte(byte_idx)
    }

    /// Returns the chunk containing the given char index, along with
    /// the byte and char indices of the beginning of the chunk.
    ///
    /// The return value is organized as `(chunk, chunk_byte_idx, chunk_char_idx)`.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    pub fn chunk_at_char(&self, char_idx: usize) -> (&str, usize, usize) {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        self.root.get_chunk_at_char(char_idx)
    }

    //-----------------------------------------------------------------------
    // Slicing

    /// Gets an immutable slice of the `Rope`.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.  The range is in `char`
    /// indices.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::Rope;
    /// let rope = Rope::from_str("Hello world!");
    /// let slice = rope.slice(..5);
    ///
    /// assert_eq!("Hello", slice);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the start of the range is greater than the end, or if the
    /// end is out of bounds (i.e. `end > len_chars()`).
    pub fn slice<R: CharIdxRange>(&self, range: R) -> RopeSlice {
        let start = range.start().unwrap_or(0);
        let end = range.end().unwrap_or_else(|| self.len_chars());

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
    ///
    /// Runs in O(N) time.
    pub fn to_string(&self) -> String {
        use iter::Chunks;
        let mut text = String::with_capacity(self.len_bytes());
        for chunk in Chunks::new(&self.root) {
            text.push_str(chunk);
        }
        text
    }

    //-----------------------------------------------------------------------
    // Debugging

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    ///
    /// Debugging tool to make sure that all of the meta-data of the
    /// tree is consistent with the actual data.
    #[doc(hidden)]
    pub fn assert_integrity(&self) {
        self.root.assert_integrity();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    ///
    /// Debugging tool to make sure that all of the following invariants
    /// hold true throughout the tree:
    ///
    /// - The tree is the same height everywhere.
    /// - All internal nodes have the minimum number of children.
    /// - All leaf nodes are non-empty.
    /// - CRLF pairs are never split over chunk boundaries.
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
                    assert!(crlf::seam_is_break(last_chunk.as_bytes(), chunk.as_bytes()));
                    last_chunk = chunk;
                } else if last_chunk.is_empty() {
                    last_chunk = chunk;
                }
            }
        }
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
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for chunk in self.chunks() {
            write!(f, "{}", chunk)?
        }
        Ok(())
    }
}

impl std::default::Default for Rope {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl std::cmp::PartialEq<Rope> for Rope {
    #[inline]
    fn eq(&self, other: &Rope) -> bool {
        self.slice(..) == other.slice(..)
    }
}

impl<'a> std::cmp::PartialEq<&'a str> for Rope {
    #[inline]
    fn eq(&self, other: &&'a str) -> bool {
        self.slice(..) == *other
    }
}

impl<'a> std::cmp::PartialEq<Rope> for &'a str {
    #[inline]
    fn eq(&self, other: &Rope) -> bool {
        *self == other.slice(..)
    }
}

impl std::cmp::PartialEq<str> for Rope {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.slice(..) == other
    }
}

impl std::cmp::PartialEq<Rope> for str {
    #[inline]
    fn eq(&self, other: &Rope) -> bool {
        self == other.slice(..)
    }
}

impl<'a> std::cmp::PartialEq<String> for Rope {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.slice(..) == other.as_str()
    }
}

impl<'a> std::cmp::PartialEq<Rope> for String {
    #[inline]
    fn eq(&self, other: &Rope) -> bool {
        self.as_str() == other.slice(..)
    }
}

impl<'a> std::cmp::PartialEq<std::borrow::Cow<'a, str>> for Rope {
    #[inline]
    fn eq(&self, other: &std::borrow::Cow<'a, str>) -> bool {
        self.slice(..) == **other
    }
}

impl<'a> std::cmp::PartialEq<Rope> for std::borrow::Cow<'a, str> {
    #[inline]
    fn eq(&self, other: &Rope) -> bool {
        **self == other.slice(..)
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
    fn insert_char_01() {
        let mut r = Rope::from_str(TEXT);
        r.insert_char(3, 'A');
        r.insert_char(12, '!');
        r.insert_char(12, '!');

        assert_eq!(
            r,
            "HelAlo there!!!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn insert_char_02() {
        let mut r = Rope::new();

        r.insert_char(0, '！');
        r.insert_char(0, 'こ');
        r.insert_char(1, 'ん');
        r.insert_char(2, 'い');
        r.insert_char(3, 'ち');
        r.insert_char(4, 'は');
        r.insert_char(5, '、');
        r.insert_char(6, 'み');
        r.insert_char(7, 'ん');
        r.insert_char(8, 'な');
        r.insert_char(9, 'さ');
        r.insert_char(10, 'ん');
        assert_eq!("こんいちは、みんなさん！", r);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn remove_01() {
        let mut r = Rope::from_str(TEXT);

        r.remove(5..11);
        r.remove(24..31);
        r.remove(19..25);
        r.remove(75..79);
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

        // Make sure CRLF pairs get merged properly, via
        // assert_invariants() below.
        r.remove(3..6);
        assert_eq!(r, "\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn remove_03() {
        let mut r = Rope::from_str(TEXT);

        // Make sure crlf pairs get merged properly
        r.remove(45..45);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn remove_04() {
        let mut r = Rope::from_str(TEXT);

        // Make sure crlf pairs get merged properly
        r.remove(0..103);
        assert_eq!(r, "");

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    #[should_panic]
    fn remove_05() {
        let mut r = Rope::from_str(TEXT);
        r.remove(56..55); // Wrong ordering of start/end
    }

    #[test]
    #[should_panic]
    fn remove_06() {
        let mut r = Rope::from_str(TEXT);
        r.remove(102..104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_07() {
        let mut r = Rope::from_str(TEXT);
        r.remove(103..104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_08() {
        let mut r = Rope::from_str(TEXT);
        r.remove(104..104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_09() {
        let mut r = Rope::from_str(TEXT);
        r.remove(104..105); // Removing past the end
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
    fn shrink_to_fit_01() {
        let mut r = Rope::new();
        for _ in 0..10 {
            let len = r.len_chars();
            r.insert(len / 2, "こ");
            r.insert(len / 2, "ん");
            r.insert(len / 2, "い");
            r.insert(len / 2, "ち");
            r.insert(len / 2, "は");
            r.insert(len / 2, "、");
            r.insert(len / 2, "み");
            r.insert(len / 2, "ん");
            r.insert(len / 2, "な");
            r.insert(len / 2, "さ");
            r.insert(len / 2, "ん");
            r.insert(len / 2, "！");
            r.insert(len / 2, "zopter");
        }

        let r2 = r.clone();
        r.shrink_to_fit();

        assert_eq!(r, r2);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn byte_to_char_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.byte_to_char(0));
        assert_eq!(1, r.byte_to_char(1));
        assert_eq!(2, r.byte_to_char(2));

        assert_eq!(91, r.byte_to_char(91));
        assert_eq!(91, r.byte_to_char(92));
        assert_eq!(91, r.byte_to_char(93));

        assert_eq!(92, r.byte_to_char(94));
        assert_eq!(92, r.byte_to_char(95));
        assert_eq!(92, r.byte_to_char(96));

        assert_eq!(102, r.byte_to_char(124));
        assert_eq!(102, r.byte_to_char(125));
        assert_eq!(102, r.byte_to_char(126));
        assert_eq!(103, r.byte_to_char(127));
    }

    #[test]
    fn byte_to_line_01() {
        let r = Rope::from_str(TEXT_LINES);

        assert_eq!(0, r.byte_to_line(0));
        assert_eq!(0, r.byte_to_line(1));

        assert_eq!(0, r.byte_to_line(31));
        assert_eq!(1, r.byte_to_line(32));
        assert_eq!(1, r.byte_to_line(33));

        assert_eq!(1, r.byte_to_line(58));
        assert_eq!(2, r.byte_to_line(59));
        assert_eq!(2, r.byte_to_line(60));

        assert_eq!(2, r.byte_to_line(87));
        assert_eq!(3, r.byte_to_line(88));
        assert_eq!(3, r.byte_to_line(89));
        assert_eq!(3, r.byte_to_line(124));
    }

    #[test]
    fn byte_to_line_02() {
        let r = Rope::from_str("");
        assert_eq!(0, r.byte_to_line(0));
    }

    #[test]
    fn byte_to_line_03() {
        let r = Rope::from_str("Hi there\n");
        assert_eq!(0, r.byte_to_line(0));
        assert_eq!(0, r.byte_to_line(8));
        assert_eq!(1, r.byte_to_line(9));
    }

    #[test]
    #[should_panic]
    fn byte_to_line_04() {
        let r = Rope::from_str(TEXT_LINES);
        r.byte_to_line(125);
    }

    #[test]
    fn char_to_byte_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.char_to_byte(0));
        assert_eq!(1, r.char_to_byte(1));
        assert_eq!(2, r.char_to_byte(2));

        assert_eq!(91, r.char_to_byte(91));
        assert_eq!(94, r.char_to_byte(92));
        assert_eq!(97, r.char_to_byte(93));
        assert_eq!(100, r.char_to_byte(94));

        assert_eq!(124, r.char_to_byte(102));
        assert_eq!(127, r.char_to_byte(103));
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
        assert_eq!(3, r.char_to_line(100));
    }

    #[test]
    fn char_to_line_02() {
        let r = Rope::from_str("");
        assert_eq!(0, r.char_to_line(0));
    }

    #[test]
    fn char_to_line_03() {
        let r = Rope::from_str("Hi there\n");
        assert_eq!(0, r.char_to_line(0));
        assert_eq!(0, r.char_to_line(8));
        assert_eq!(1, r.char_to_line(9));
    }

    #[test]
    #[should_panic]
    fn char_to_line_04() {
        let r = Rope::from_str(TEXT_LINES);
        r.char_to_line(101);
    }

    #[test]
    fn line_to_byte_01() {
        let r = Rope::from_str(TEXT_LINES);

        assert_eq!(0, r.line_to_byte(0));
        assert_eq!(32, r.line_to_byte(1));
        assert_eq!(59, r.line_to_byte(2));
        assert_eq!(88, r.line_to_byte(3));
        assert_eq!(124, r.line_to_byte(4));
    }

    #[test]
    fn line_to_byte_02() {
        let r = Rope::from_str("");
        assert_eq!(0, r.line_to_byte(0));
        assert_eq!(0, r.line_to_byte(1));
    }

    #[test]
    #[should_panic]
    fn line_to_byte_03() {
        let r = Rope::from_str(TEXT_LINES);
        r.line_to_byte(5);
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
    fn slice_01() {
        let r = Rope::from_str(TEXT);

        let s = r.slice(0..r.len_chars());

        assert_eq!(TEXT, s);
    }

    #[test]
    fn slice_02() {
        let r = Rope::from_str(TEXT);

        let s = r.slice(5..21);

        assert_eq!(&TEXT[5..21], s);
    }

    #[test]
    fn slice_03() {
        let r = Rope::from_str(TEXT);

        let s = r.slice(31..97);

        assert_eq!(&TEXT[31..109], s);
    }

    #[test]
    fn slice_04() {
        let r = Rope::from_str(TEXT);

        let s = r.slice(53..53);

        assert_eq!("", s);
    }

    #[test]
    #[should_panic]
    fn slice_05() {
        let r = Rope::from_str(TEXT);
        r.slice(53..52);
    }

    #[test]
    #[should_panic]
    fn slice_06() {
        let r = Rope::from_str(TEXT);
        r.slice(102..104);
    }

    #[test]
    fn eq_rope_01() {
        let r = Rope::from_str("");

        assert_eq!(r, r);
    }

    #[test]
    fn eq_rope_02() {
        let r = Rope::from_str(TEXT);

        assert_eq!(r, r);
    }

    #[test]
    fn eq_rope_03() {
        let r1 = Rope::from_str(TEXT);
        let mut r2 = r1.clone();
        r2.remove(26..27);
        r2.insert(26, "z");

        assert_ne!(r1, r2);
    }

    #[test]
    fn eq_rope_04() {
        let r = Rope::from_str("");

        assert_eq!(r, "");
        assert_eq!("", r);
    }

    #[test]
    fn eq_rope_05() {
        let r = Rope::from_str(TEXT);

        assert_eq!(r, TEXT);
        assert_eq!(TEXT, r);
    }

    #[test]
    fn eq_rope_06() {
        let mut r = Rope::from_str(TEXT);
        r.remove(26..27);
        r.insert(26, "z");

        assert_ne!(r, TEXT);
        assert_ne!(TEXT, r);
    }

    #[test]
    fn eq_rope_07() {
        let r = Rope::from_str(TEXT);
        let s: String = TEXT.into();

        assert_eq!(r, s);
        assert_eq!(s, r);
    }

    // Iterator tests are in the iter module
}
