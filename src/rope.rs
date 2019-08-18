use std;
use std::io;
use std::iter::FromIterator;
use std::ops::RangeBounds;
use std::ptr;
use std::sync::Arc;

use crlf;
use iter::{Bytes, Chars, Chunks, Lines};
use rope_builder::RopeBuilder;
use slice::{end_bound_to_num, start_bound_to_num, RopeSlice};
use str_utils::{
    byte_to_char_idx, byte_to_line_idx, char_to_byte_idx, char_to_line_idx, line_to_byte_idx,
    line_to_char_idx,
};
use tree::{Count, Node, NodeChildren, TextInfo, MAX_BYTES};

/// A utf8 text rope.
///
/// The time complexity of nearly all edit and query operations on `Rope` are
/// worst-case `O(log N)` in the length of the rope.  `Rope` is designed to
/// work efficiently even for huge (in the gigabytes) and pathological (all on
/// one line) texts.
///
/// # Editing Operations
///
/// The primary editing operations on `Rope` are insertion and removal of text.
/// For example:
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
/// You can also split off the end of a `Rope` or append one `Rope` to another:
///
/// ```
/// # use ropey::Rope;
/// #
/// let mut rope = Rope::from_str("Hello みんなさん!");
/// let right_side = rope.split_off(6);
///
/// assert_eq!(rope, "Hello ");
/// assert_eq!(right_side, "みんなさん!");
///
/// rope.append(Rope::from_str("world!"));
///
/// assert_eq!(rope, "Hello world!");
/// ```
///
/// Note that `insert()` and `remove()` are generally faster than `split_off()`
/// and `append()`.
///
/// # Query Operations
///
/// `Rope` provides a rich set of efficient query functions, including querying
/// rope length in bytes/`char`s/lines, fetching individual `char`s or lines,
/// and converting between byte/`char`/line indices.  For example, to find the
/// starting `char` index of a given line:
///
/// ```
/// # use ropey::Rope;
/// #
/// let rope = Rope::from_str("Hello みんなさん!\nHow are you?\nThis text has multiple lines!");
///
/// assert_eq!(rope.line_to_char(0), 0);
/// assert_eq!(rope.line_to_char(1), 13);
/// assert_eq!(rope.line_to_char(2), 26);
/// ```
///
/// # Slicing
///
/// You can take immutable slices of a `Rope` using `slice()`:
///
/// ```
/// # use ropey::Rope;
/// #
/// let mut rope = Rope::from_str("Hello みんなさん!");
/// let middle = rope.slice(3..8);
///
/// assert_eq!(middle, "lo みん");
/// ```
///
/// # Cloning
///
/// Cloning `Rope`s is extremely cheap, running in `O(1)` time and taking a
/// small constant amount of memory for the new clone, regardless of text size.
/// This is accomplished by data sharing between `Rope` clones.  The memory
/// used by clones only grows incrementally as the their contents diverge due
/// to edits.  All of this is thread safe, so clones can be sent freely
/// between threads.
///
/// The primary intended use-case for this feature is to allow asynchronous
/// processing of `Rope`s.  For example, saving a large document to disk in a
/// separate thread while the user continues to perform edits.
#[derive(Clone)]
pub struct Rope {
    pub(crate) root: Arc<Node>,
}

impl Rope {
    //-----------------------------------------------------------------------
    // Constructors

    /// Creates an empty `Rope`.
    #[inline]
    pub fn new() -> Self {
        Rope {
            root: Arc::new(Node::new()),
        }
    }

    /// Creates a `Rope` from a string slice.
    ///
    /// Runs in O(N) time.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        RopeBuilder::new().build_at_once(text)
    }

    /// Creates a `Rope` from the output of a reader.
    ///
    /// This is a convenience function.  To do more sophisticated text loading,
    /// see [`RopeBuilder`](struct.RopeBuilder.html).
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

                    // Determine how much of the buffer is valid utf8.
                    let valid_count = match std::str::from_utf8(&buffer[..fill_idx]) {
                        Ok(_) => fill_idx,
                        Err(e) => e.valid_up_to(),
                    };

                    // Append the valid part of the buffer to the rope.
                    if valid_count > 0 {
                        // The unsafe block here is reinterpreting the bytes as
                        // utf8.  This is safe because the bytes being
                        // reinterpreted have already been validated as utf8
                        // just above.
                        builder.append(unsafe {
                            std::str::from_utf8_unchecked(&buffer[..valid_count])
                        });
                    }

                    // Shift the un-read part of the buffer to the beginning.
                    if valid_count < fill_idx {
                        // The unsafe here is just used for efficiency.  This
                        // can be replaced with a safe call to `copy_within()`
                        // on the slice once that API is stabalized in the
                        // standard library.
                        unsafe {
                            ptr::copy(
                                buffer.as_ptr().add(valid_count),
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
                                "stream contained invalid UTF-8",
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
    // Convenience output methods

    /// Writes the contents of the `Rope` to a writer.
    ///
    /// This is a convenience function.  To do more sophisticated text output,
    /// see the [`Chunks`](iter/struct.Chunks.html) iterator.
    ///
    /// Runs in O(N) time.
    ///
    /// # Errors
    ///
    /// - If the writer returns an error, `write_to` stops and returns that
    ///   error.
    ///
    /// Note: some data may have been written even if an error is returned.
    #[allow(unused_mut)]
    pub fn write_to<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        for chunk in self.chunks() {
            writer.write_all(chunk.as_bytes())?;
        }

        Ok(())
    }

    //-----------------------------------------------------------------------
    // Informational methods

    /// Total number of bytes in the `Rope`.
    ///
    /// Runs in O(1) time.
    #[inline]
    pub fn len_bytes(&self) -> usize {
        self.root.byte_count()
    }

    /// Total number of chars in the `Rope`.
    ///
    /// Runs in O(1) time.
    #[inline]
    pub fn len_chars(&self) -> usize {
        self.root.char_count()
    }

    /// Total number of lines in the `Rope`.
    ///
    /// Runs in O(1) time.
    #[inline]
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
                    .children_mut()
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
    #[inline]
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to insert past end of Rope: insertion point {}, Rope length {}",
            char_idx,
            self.len_chars()
        );

        // We have three cases here:
        // 1. The insertion text is very large, in which case building a new
        //    Rope out of it and splicing it into the existing Rope is most
        //    efficient.
        // 2. The insertion text is somewhat large, in which case splitting it
        //    up into chunks and repeatedly inserting them is the most
        //    efficient.  The splitting is necessary because the insertion code
        //    only works correctly below a certain insertion size.
        // 3. The insertion text is small, in which case we can simply insert
        //    it.
        //
        // Cases #2 and #3 are rolled into one case here, where case #3 just
        // results in the text being "split" into only one chunk.
        //
        // The boundary for what constitutes "very large" text was arrived at
        // experimentally, by testing at what point Rope build + splice becomes
        // faster than split + repeated insert.  This constant is likely worth
        // revisiting from time to time as Ropey evolves.
        if text.len() > MAX_BYTES * 6 {
            // Case #1: very large text, build rope and splice it in.
            let text_rope = Rope::from_str(text);
            let right = self.split_off(char_idx);
            self.append(text_rope);
            self.append(right);
        } else {
            // Cases #2 and #3: split into chunks and repeatedly insert.
            let mut text = text;
            while !text.is_empty() {
                // Split a chunk off from the end of the text.
                // We do this from the end instead of the front so that
                // the repeated insertions can keep re-using the same
                // insertion point.
                let split_idx = crlf::find_good_split(
                    text.len() - (MAX_BYTES - 4).min(text.len()),
                    text.as_bytes(),
                    false,
                );
                let ins_text = &text[split_idx..];
                text = &text[..split_idx];

                // Do the insertion.
                self.insert_internal(char_idx, ins_text);
            }
        }
    }

    /// Inserts a single char `ch` at char index `char_idx`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    #[inline]
    pub fn insert_char(&mut self, char_idx: usize, ch: char) {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to insert past end of Rope: insertion point {}, Rope length {}",
            char_idx,
            self.len_chars()
        );

        let mut buf = [0u8; 4];
        self.insert_internal(char_idx, ch.encode_utf8(&mut buf));
    }

    /// Private internal-only method that does a single insertion of
    /// sufficiently small text.
    ///
    /// This only works correctly for insertion texts smaller than or equal to
    /// `MAX_BYTES - 4`.
    ///
    /// Note that a lot of the complexity in this method comes from avoiding
    /// splitting CRLF pairs and, when possible, avoiding re-scanning text for
    /// text info.  It is otherwise conceptually fairly straightforward.
    fn insert_internal(&mut self, char_idx: usize, ins_text: &str) {
        let mut ins_text = ins_text;
        let mut left_seam = false;
        let root_info = self.root.text_info();

        let (l_info, residual) = Arc::make_mut(&mut self.root).edit_chunk_at_char(
            char_idx,
            root_info,
            |idx, cur_info, leaf_text| {
                // First check if we have a left seam.
                if idx == 0 && char_idx > 0 && ins_text.as_bytes()[0] == 0x0A {
                    left_seam = true;
                    ins_text = &ins_text[1..];
                    // Early out if it was only an LF.
                    if ins_text.is_empty() {
                        return (cur_info, None);
                    }
                }

                // Find our byte index
                let byte_idx = char_to_byte_idx(leaf_text, idx);

                // No node splitting
                if (leaf_text.len() + ins_text.len()) <= MAX_BYTES {
                    // Calculate new info without doing a full re-scan of cur_text
                    let new_info = {
                        // Get summed info of current text and to-be-inserted text
                        let mut info = cur_info + TextInfo::from_str(ins_text);
                        // Check for CRLF pairs on the insertion seams, and
                        // adjust line break counts accordingly
                        if byte_idx > 0 {
                            if leaf_text.as_bytes()[byte_idx - 1] == 0x0D
                                && ins_text.as_bytes()[0] == 0x0A
                            {
                                info.line_breaks -= 1;
                            }
                            if byte_idx < leaf_text.len()
                                && leaf_text.as_bytes()[byte_idx - 1] == 0x0D
                                && leaf_text.as_bytes()[byte_idx] == 0x0A
                            {
                                info.line_breaks += 1;
                            }
                        }
                        if byte_idx < leaf_text.len()
                            && *ins_text.as_bytes().last().unwrap() == 0x0D
                            && leaf_text.as_bytes()[byte_idx] == 0x0A
                        {
                            info.line_breaks -= 1;
                        }
                        info
                    };
                    // Insert the text and return the new info
                    leaf_text.insert_str(byte_idx, ins_text);
                    (new_info, None)
                }
                // We're splitting the node
                else {
                    let r_text = leaf_text.insert_str_split(byte_idx, ins_text);
                    let l_text_info = TextInfo::from_str(&leaf_text);
                    if r_text.len() > 0 {
                        let r_text_info = TextInfo::from_str(&r_text);
                        (
                            l_text_info,
                            Some((r_text_info, Arc::new(Node::Leaf(r_text)))),
                        )
                    } else {
                        // Leaf couldn't be validly split, so leave it oversized
                        (l_text_info, None)
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

        // Insert the LF to the left.
        // TODO: this code feels fairly redundant with above.  Can we DRY this
        // better?
        if left_seam {
            // Do the insertion
            let root_info = self.root.text_info();
            let (l_info, residual) = Arc::make_mut(&mut self.root).edit_chunk_at_char(
                char_idx - 1,
                root_info,
                |_, cur_info, leaf_text| {
                    let byte_idx = leaf_text.len();

                    // No node splitting
                    if (leaf_text.len() + ins_text.len()) <= MAX_BYTES {
                        // Calculate new info without doing a full re-scan of cur_text
                        let mut new_info = cur_info;
                        new_info.bytes += 1;
                        new_info.chars += 1;
                        if *leaf_text.as_bytes().last().unwrap() != 0x0D {
                            new_info.line_breaks += 1;
                        }
                        // Insert the text and return the new info
                        leaf_text.insert_str(byte_idx, "\n");
                        (new_info, None)
                    }
                    // We're splitting the node
                    else {
                        let r_text = leaf_text.insert_str_split(byte_idx, "\n");
                        let l_text_info = TextInfo::from_str(&leaf_text);
                        if r_text.len() > 0 {
                            let r_text_info = TextInfo::from_str(&r_text);
                            (
                                l_text_info,
                                Some((r_text_info, Arc::new(Node::Leaf(r_text)))),
                            )
                        } else {
                            // Leaf couldn't be validly split, so leave it oversized
                            (l_text_info, None)
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
        }
    }

    /// Removes the text in the given char index range.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.  The range is in `char`
    /// indices.
    ///
    /// Runs in O(M + log N) time, where N is the length of the `Rope` and M
    /// is the length of the range being removed.
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
    pub fn remove<R>(&mut self, char_range: R)
    where
        R: RangeBounds<usize>,
    {
        let start = start_bound_to_num(char_range.start_bound()).unwrap_or(0);
        let end = end_bound_to_num(char_range.end_bound()).unwrap_or_else(|| self.len_chars());

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

            let root_info = root.text_info();
            let (_, crlf_seam, needs_fix) = root.remove_char_range(start, end, root_info);

            if crlf_seam {
                let seam_idx = root.char_to_byte_and_line(start).0;
                root.fix_crlf_seam(seam_idx as Count, false);
            }

            if needs_fix {
                root.fix_after_remove(start);
            }
        }

        self.pull_up_singular_nodes();
    }

    /// Splits the `Rope` at `char_idx`, returning the right part of
    /// the split.
    ///
    /// Runs in O(log N) time.
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
            let mut new_rope = Rope {
                root: Arc::new(Arc::make_mut(&mut self.root).split(char_idx)),
            };

            // Fix up the edges
            Arc::make_mut(&mut self.root).zip_fix_right();
            Arc::make_mut(&mut new_rope.root).zip_fix_left();
            self.pull_up_singular_nodes();
            new_rope.pull_up_singular_nodes();

            new_rope
        }
    }

    /// Appends a `Rope` to the end of this one, consuming the other `Rope`.
    ///
    /// Runs in O(log N) time.
    pub fn append(&mut self, other: Self) {
        if self.len_chars() == 0 {
            // Special case
            let mut other = other;
            std::mem::swap(self, &mut other);
        } else if other.len_chars() > 0 {
            let seam_byte_i = if other.char(0) == '\n' {
                Some(self.root.text_info().bytes)
            } else {
                None
            };

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

            if let Some(i) = seam_byte_i {
                Arc::make_mut(&mut self.root).fix_crlf_seam(i, true);
            }
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
    /// - `byte_idx` can be one-past-the-end, which will return
    ///   one-past-the-end char index.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    #[inline]
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        let (chunk, b, c, _) = self.chunk_at_byte(byte_idx);
        c + byte_to_char_idx(chunk, byte_idx - b)
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
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        let (chunk, b, _, l) = self.chunk_at_byte(byte_idx);
        l + byte_to_line_idx(chunk, byte_idx - b)
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
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        let (chunk, b, c, _) = self.chunk_at_char(char_idx);
        b + char_to_byte_idx(chunk, char_idx - c)
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
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        let (chunk, _, c, l) = self.chunk_at_char(char_idx);
        l + char_to_line_idx(chunk, char_idx - c)
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
            let (chunk, b, _, l) = self.chunk_at_line_break(line_idx);
            b + line_to_byte_idx(chunk, line_idx - l)
        }
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
            let (chunk, _, c, l) = self.chunk_at_line_break(line_idx);
            c + line_to_char_idx(chunk, line_idx - l)
        }
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
        assert!(
            byte_idx < self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        let (chunk, chunk_byte_idx, _, _) = self.chunk_at_byte(byte_idx);
        let chunk_rel_byte_idx = byte_idx - chunk_byte_idx;
        chunk.as_bytes()[chunk_rel_byte_idx]
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
        // Bounds check
        assert!(
            char_idx < self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        let (chunk, _, chunk_char_idx, _) = self.chunk_at_char(char_idx);
        let byte_idx = char_to_byte_idx(chunk, char_idx - chunk_char_idx);
        chunk[byte_idx..].chars().nth(0).unwrap()
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
    pub fn line(&self, line_idx: usize) -> RopeSlice {
        use slice::RSEnum;
        use str_utils::count_chars;

        let len_lines = self.len_lines();

        // Bounds check
        assert!(
            line_idx < len_lines,
            "Attempt to index past end of Rope: line index {}, Rope line length {}",
            line_idx,
            len_lines
        );

        let (chunk_1, _, c1, l1) = self.chunk_at_line_break(line_idx);
        let (chunk_2, _, c2, l2) = self.chunk_at_line_break(line_idx + 1);
        if c1 == c2 {
            let text1 = &chunk_1[line_to_byte_idx(chunk_1, line_idx - l1)..];
            let text2 = &text1[..line_to_byte_idx(text1, 1)];
            RopeSlice(RSEnum::Light {
                text: text2,
                char_count: count_chars(text2) as Count,
                line_break_count: if line_idx == (len_lines - 1) { 0 } else { 1 },
            })
        } else {
            let start = c1 + line_to_char_idx(chunk_1, line_idx - l1);
            let end = c2 + line_to_char_idx(chunk_2, line_idx + 1 - l2);
            self.slice(start..end)
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
    #[inline]
    pub fn chunk_at_byte(&self, byte_idx: usize) -> (&str, usize, usize, usize) {
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        self.root.get_chunk_at_byte(byte_idx)
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
    #[inline]
    pub fn chunk_at_char(&self, char_idx: usize) -> (&str, usize, usize, usize) {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        self.root.get_chunk_at_char(char_idx)
    }

    /// Returns the chunk containing the given line break.
    ///
    /// Also returns the byte and char indices of the beginning of the chunk
    /// and the index of the line that the chunk starts on.
    ///
    /// Note: for convenience, both the beginning and end of the rope are
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
    #[inline]
    pub fn chunk_at_line_break(&self, line_break_idx: usize) -> (&str, usize, usize, usize) {
        // Bounds check
        assert!(
            line_break_idx <= self.len_lines(),
            "Attempt to index past end of Rope: line break index {}, max index {}",
            line_break_idx,
            self.len_lines()
        );

        self.root.get_chunk_at_line_break(line_break_idx)
    }

    //-----------------------------------------------------------------------
    // Slicing

    /// Gets an immutable slice of the `Rope`.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.
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
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if the start of the range is greater than the end, or if the
    /// end is out of bounds (i.e. `end > len_chars()`).
    #[inline]
    pub fn slice<R>(&self, char_range: R) -> RopeSlice
    where
        R: RangeBounds<usize>,
    {
        let start = start_bound_to_num(char_range.start_bound()).unwrap_or(0);
        let end = end_bound_to_num(char_range.end_bound()).unwrap_or_else(|| self.len_chars());

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
    ///
    /// Runs in O(log N) time.
    #[inline]
    pub fn bytes(&self) -> Bytes {
        Bytes::new(&self.root)
    }

    /// Creates an iterator over the bytes of the `Rope`, starting at byte
    /// `byte_idx`.
    ///
    /// If `byte_idx == len_bytes()` then an iterator at the end of the
    /// `Rope` is created (i.e. `next()` will return `None`).
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    #[inline]
    pub fn bytes_at(&self, byte_idx: usize) -> Bytes {
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        let info = self.root.text_info();
        Bytes::new_with_range_at(
            &self.root,
            byte_idx,
            (0, info.bytes as usize),
            (0, info.chars as usize),
            (0, info.line_breaks as usize + 1),
        )
    }

    /// Creates an iterator over the chars of the `Rope`.
    ///
    /// Runs in O(log N) time.
    #[inline]
    pub fn chars(&self) -> Chars {
        Chars::new(&self.root)
    }

    /// Creates an iterator over the chars of the `Rope`, starting at char
    /// `char_idx`.
    ///
    /// If `char_idx == len_chars()` then an iterator at the end of the
    /// `Rope` is created (i.e. `next()` will return `None`).
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
    #[inline]
    pub fn chars_at(&self, char_idx: usize) -> Chars {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        let info = self.root.text_info();
        Chars::new_with_range_at(
            &self.root,
            char_idx,
            (0, info.bytes as usize),
            (0, info.chars as usize),
            (0, info.line_breaks as usize + 1),
        )
    }

    /// Creates an iterator over the lines of the `Rope`.
    ///
    /// Runs in O(log N) time.
    #[inline]
    pub fn lines(&self) -> Lines {
        Lines::new(&self.root)
    }

    /// Creates an iterator over the lines of the `Rope`, starting at line
    /// `line_idx`.
    ///
    /// If `line_idx == len_lines()` then an iterator at the end of the
    /// `Rope` is created (i.e. `next()` will return `None`).
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// Panics if `line_idx` is out of bounds (i.e. `line_idx > len_lines()`).
    #[inline]
    pub fn lines_at(&self, line_idx: usize) -> Lines {
        // Bounds check
        assert!(
            line_idx <= self.len_lines(),
            "Attempt to index past end of Rope: line index {}, Rope line length {}",
            line_idx,
            self.len_lines()
        );

        Lines::new_with_range_at(
            &self.root,
            line_idx,
            (0, self.len_bytes()),
            (0, self.len_lines()),
        )
    }

    /// Creates an iterator over the chunks of the `Rope`.
    ///
    /// Runs in O(log N) time.
    #[inline]
    pub fn chunks(&self) -> Chunks {
        Chunks::new(&self.root)
    }

    /// Creates an iterator over the chunks of the `Rope`, with the
    /// iterator starting at the chunk containing `byte_idx`.
    ///
    /// Also returns the byte and char indices of the beginning of the first
    /// chunk to be yielded, and the index of the line that chunk starts on.
    ///
    /// If `byte_idx == len_bytes()` an iterator at the end of the `Rope`
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
    pub fn chunks_at_byte(&self, byte_idx: usize) -> (Chunks, usize, usize, usize) {
        // Bounds check
        assert!(
            byte_idx <= self.len_bytes(),
            "Attempt to index past end of Rope: byte index {}, Rope byte length {}",
            byte_idx,
            self.len_bytes()
        );

        Chunks::new_with_range_at_byte(
            &self.root,
            byte_idx,
            (0, self.len_bytes()),
            (0, self.len_chars()),
            (0, self.len_lines()),
        )
    }

    /// Creates an iterator over the chunks of the `Rope`, with the
    /// iterator starting at the chunk containing `char_idx`.
    ///
    /// Also returns the byte and char indices of the beginning of the first
    /// chunk to be yielded, and the index of the line that chunk starts on.
    ///
    /// If `char_idx == len_chars()` an iterator at the end of the `Rope`
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
    pub fn chunks_at_char(&self, char_idx: usize) -> (Chunks, usize, usize, usize) {
        // Bounds check
        assert!(
            char_idx <= self.len_chars(),
            "Attempt to index past end of Rope: char index {}, Rope char length {}",
            char_idx,
            self.len_chars()
        );

        Chunks::new_with_range_at_char(
            &self.root,
            char_idx,
            (0, self.len_bytes()),
            (0, self.len_chars()),
            (0, self.len_lines()),
        )
    }

    /// Creates an iterator over the chunks of the `Rope`, with the
    /// iterator starting at the chunk containing `line_break_idx`.
    ///
    /// Also returns the byte and char indices of the beginning of the first
    /// chunk to be yielded, and the index of the line that chunk starts on.
    ///
    /// Note: for convenience, both the beginning and end of the `Rope` are
    /// considered line breaks for the purposes of indexing.  For example, in
    /// the string `"Hello \n world!"` 0 would create an iterator starting on
    /// the first chunk, 1 would create an iterator starting on the chunk
    /// containing the newline character, and 2 would create an iterator at
    /// the end of the `Rope` (yielding `None` on a call to `next()`).
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
    pub fn chunks_at_line_break(&self, line_break_idx: usize) -> (Chunks, usize, usize, usize) {
        // Bounds check
        assert!(
            line_break_idx <= self.len_lines(),
            "Attempt to index past end of Rope: line break index {}, max index {}",
            line_break_idx,
            self.len_lines()
        );

        Chunks::new_with_range_at_line_break(
            &self.root,
            line_break_idx,
            (0, self.len_bytes()),
            (0, self.len_chars()),
            (0, self.len_lines()),
        )
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
        self.assert_crlf_seams();
    }

    /// Checks that CRLF pairs are never split over chunk boundaries.
    fn assert_crlf_seams(&self) {
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

    /// Iteratively replaces the root node with its child if it only has
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
// Conversion impls

impl<'a> From<&'a str> for Rope {
    #[inline]
    fn from(text: &'a str) -> Self {
        Rope::from_str(text)
    }
}

impl<'a> From<std::borrow::Cow<'a, str>> for Rope {
    #[inline]
    fn from(text: std::borrow::Cow<'a, str>) -> Self {
        Rope::from_str(&text)
    }
}

impl From<String> for Rope {
    #[inline]
    fn from(text: String) -> Self {
        Rope::from_str(&text)
    }
}

/// Will share data where possible.
///
/// Runs in O(log N) time.
impl<'a> From<RopeSlice<'a>> for Rope {
    fn from(s: RopeSlice<'a>) -> Self {
        use slice::RSEnum;
        match s {
            RopeSlice(RSEnum::Full {
                node,
                start_char,
                end_char,
                ..
            }) => {
                let mut rope = Rope {
                    root: Arc::clone(node),
                };

                // Chop off right end if needed
                if end_char < node.text_info().chars {
                    {
                        let root = Arc::make_mut(&mut rope.root);
                        root.split(end_char as usize);
                        root.zip_fix_right();
                    }
                    rope.pull_up_singular_nodes();
                }

                // Chop off left end if needed
                if start_char > 0 {
                    {
                        let root = Arc::make_mut(&mut rope.root);
                        *root = root.split(start_char as usize);
                        root.zip_fix_left();
                    }
                    rope.pull_up_singular_nodes();
                }

                // Return the rope
                rope
            }
            RopeSlice(RSEnum::Light { text, .. }) => Rope::from_str(text),
        }
    }
}

impl From<Rope> for String {
    #[inline]
    fn from(r: Rope) -> Self {
        String::from(&r)
    }
}

impl<'a> From<&'a Rope> for String {
    #[inline]
    fn from(r: &'a Rope) -> Self {
        let mut text = String::with_capacity(r.len_bytes());
        text.extend(r.chunks());
        text
    }
}

impl<'a> From<Rope> for std::borrow::Cow<'a, str> {
    #[inline]
    fn from(r: Rope) -> Self {
        std::borrow::Cow::Owned(String::from(r))
    }
}

/// Attempts to borrow the contents of the `Rope`, but will convert to an
/// owned string if the contents is not contiguous in memory.
///
/// Runs in best case O(1), worst case O(N).
impl<'a> From<&'a Rope> for std::borrow::Cow<'a, str> {
    #[inline]
    fn from(r: &'a Rope) -> Self {
        if let Node::Leaf(ref text) = *r.root {
            std::borrow::Cow::Borrowed(text)
        } else {
            std::borrow::Cow::Owned(String::from(r))
        }
    }
}

impl<'a> FromIterator<&'a str> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = &'a str>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(chunk);
        }
        builder.finish()
    }
}

impl<'a> FromIterator<std::borrow::Cow<'a, str>> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = std::borrow::Cow<'a, str>>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(&chunk);
        }
        builder.finish()
    }
}

impl FromIterator<String> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = String>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(&chunk);
        }
        builder.finish()
    }
}

//==============================================================
// Other impls

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

impl std::cmp::Eq for Rope {}

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

impl std::cmp::Ord for Rope {
    #[inline]
    fn cmp(&self, other: &Rope) -> std::cmp::Ordering {
        self.slice(..).cmp(&other.slice(..))
    }
}

impl std::cmp::PartialOrd<Rope> for Rope {
    #[inline]
    fn partial_cmp(&self, other: &Rope) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

//==============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use str_utils::byte_to_char_idx;

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

        // Make sure removing nothing actually does nothing.
        r.remove(45..45);
        assert_eq!(r, TEXT);

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn remove_04() {
        let mut r = Rope::from_str(TEXT);

        // Make sure removing everything works.
        r.remove(0..103);
        assert_eq!(r, "");

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn remove_05() {
        let mut r = Rope::from_str(TEXT);

        // Make sure removing a large range works.
        r.remove(3..100);
        assert_eq!(r, "Helさん！");

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    #[should_panic]
    fn remove_06() {
        let mut r = Rope::from_str(TEXT);
        r.remove(56..55); // Wrong ordering of start/end
    }

    #[test]
    #[should_panic]
    fn remove_07() {
        let mut r = Rope::from_str(TEXT);
        r.remove(102..104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_08() {
        let mut r = Rope::from_str(TEXT);
        r.remove(103..104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_09() {
        let mut r = Rope::from_str(TEXT);
        r.remove(104..104); // Removing past the end
    }

    #[test]
    #[should_panic]
    fn remove_10() {
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
    fn append_07() {
        let mut r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r");
        let r2 = Rope::from_str("\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r");

        r.append(r2);
        assert_eq!(r, "\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r");
        assert_eq!(r.len_lines(), 24);

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
    fn byte_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(r.byte(0), b'H');

        // UTF-8 for "wide exclamation mark"
        assert_eq!(r.byte(124), 0xEF);
        assert_eq!(r.byte(125), 0xBC);
        assert_eq!(r.byte(126), 0x81);
    }

    #[test]
    #[should_panic]
    fn byte_02() {
        let r = Rope::from_str(TEXT);
        r.byte(127);
    }

    #[test]
    #[should_panic]
    fn byte_03() {
        let r = Rope::from_str("");
        r.byte(0);
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

        let l0 = r.line(0);
        assert_eq!(l0, "Hello there!  How're you doing?\n");
        assert_eq!(l0.len_bytes(), 32);
        assert_eq!(l0.len_chars(), 32);
        assert_eq!(l0.len_lines(), 2);

        let l1 = r.line(1);
        assert_eq!(l1, "It's a fine day, isn't it?\n");
        assert_eq!(l1.len_bytes(), 27);
        assert_eq!(l1.len_chars(), 27);
        assert_eq!(l1.len_lines(), 2);

        let l2 = r.line(2);
        assert_eq!(l2, "Aren't you glad we're alive?\n");
        assert_eq!(l2.len_bytes(), 29);
        assert_eq!(l2.len_chars(), 29);
        assert_eq!(l2.len_lines(), 2);

        let l3 = r.line(3);
        assert_eq!(l3, "こんにちは、みんなさん！");
        assert_eq!(l3.len_bytes(), 36);
        assert_eq!(l3.len_chars(), 12);
        assert_eq!(l3.len_lines(), 1);
    }

    #[test]
    fn line_02() {
        let r = Rope::from_str("Hello there!  How're you doing?\n");

        assert_eq!(r.line(0), "Hello there!  How're you doing?\n");
        assert_eq!(r.line(1), "");
    }

    #[test]
    fn line_03() {
        let r = Rope::from_str("Hi\nHi\nHi\nHi\nHi\nHi\n");

        assert_eq!(r.line(0), "Hi\n");
        assert_eq!(r.line(1), "Hi\n");
        assert_eq!(r.line(2), "Hi\n");
        assert_eq!(r.line(3), "Hi\n");
        assert_eq!(r.line(4), "Hi\n");
        assert_eq!(r.line(5), "Hi\n");
        assert_eq!(r.line(6), "");
    }

    #[test]
    fn line_04() {
        let r = Rope::from_str("");

        assert_eq!(r.line(0), "");
    }

    #[test]
    #[should_panic]
    fn line_05() {
        let r = Rope::from_str(TEXT_LINES);
        r.line(4);
    }

    #[test]
    fn line_06() {
        let r = Rope::from_str("1\n2\n3\n4\n5\n6\n7\n8");

        assert_eq!(r.line(0).len_lines(), 2);
        assert_eq!(r.line(1).len_lines(), 2);
        assert_eq!(r.line(2).len_lines(), 2);
        assert_eq!(r.line(3).len_lines(), 2);
        assert_eq!(r.line(4).len_lines(), 2);
        assert_eq!(r.line(5).len_lines(), 2);
        assert_eq!(r.line(6).len_lines(), 2);
        assert_eq!(r.line(7).len_lines(), 1);
    }

    #[test]
    fn chunk_at_byte() {
        let r = Rope::from_str(TEXT_LINES);
        let mut t = TEXT_LINES;

        let mut last_chunk = "";
        for i in 0..r.len_bytes() {
            let (chunk, b, c, l) = r.chunk_at_byte(i);
            assert_eq!(c, byte_to_char_idx(TEXT_LINES, b));
            assert_eq!(l, byte_to_line_idx(TEXT_LINES, b));
            if chunk != last_chunk {
                assert_eq!(chunk, &t[..chunk.len()]);
                t = &t[chunk.len()..];
                last_chunk = chunk;
            }

            let c1 = {
                let i2 = byte_to_char_idx(TEXT_LINES, i);
                TEXT_LINES.chars().nth(i2).unwrap()
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
        let mut t = TEXT_LINES;

        let mut last_chunk = "";
        for i in 0..r.len_chars() {
            let (chunk, b, c, l) = r.chunk_at_char(i);
            assert_eq!(b, char_to_byte_idx(TEXT_LINES, c));
            assert_eq!(l, char_to_line_idx(TEXT_LINES, c));
            if chunk != last_chunk {
                assert_eq!(chunk, &t[..chunk.len()]);
                t = &t[chunk.len()..];
                last_chunk = chunk;
            }

            let c1 = TEXT_LINES.chars().nth(i).unwrap();
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

        // First chunk
        {
            let (chunk, b, c, l) = r.chunk_at_line_break(0);
            assert_eq!(chunk, &TEXT_LINES[..chunk.len()]);
            assert_eq!(b, 0);
            assert_eq!(c, 0);
            assert_eq!(l, 0);
        }

        // Middle chunks
        for i in 1..r.len_lines() {
            let (chunk, b, c, l) = r.chunk_at_line_break(i);
            assert_eq!(chunk, &TEXT_LINES[b..(b + chunk.len())]);
            assert_eq!(c, byte_to_char_idx(TEXT_LINES, b));
            assert_eq!(l, byte_to_line_idx(TEXT_LINES, b));
            assert!(l < i);
            assert!(i <= byte_to_line_idx(TEXT_LINES, b + chunk.len()));
        }

        // Last chunk
        {
            let (chunk, b, c, l) = r.chunk_at_line_break(r.len_lines());
            assert_eq!(chunk, &TEXT_LINES[(TEXT_LINES.len() - chunk.len())..]);
            assert_eq!(chunk, &TEXT_LINES[b..]);
            assert_eq!(c, byte_to_char_idx(TEXT_LINES, b));
            assert_eq!(l, byte_to_line_idx(TEXT_LINES, b));
        }
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

    #[test]
    fn to_string_01() {
        let r = Rope::from_str(TEXT);
        let s: String = (&r).into();

        assert_eq!(r, s);
    }

    #[test]
    fn to_cow_01() {
        use std::borrow::Cow;
        let r = Rope::from_str(TEXT);
        let cow: Cow<str> = (&r).into();

        assert_eq!(r, cow);
    }

    #[test]
    fn to_cow_02() {
        use std::borrow::Cow;
        let r = Rope::from_str(TEXT);
        let cow: Cow<str> = (r.clone()).into();

        assert_eq!(r, cow);
    }

    #[test]
    fn to_cow_03() {
        use std::borrow::Cow;
        let r = Rope::from_str("a");
        let cow: Cow<str> = (&r).into();

        // Make sure it's borrowed.
        if let Cow::Owned(_) = cow {
            panic!("Small Cow conversions should result in a borrow.");
        }

        assert_eq!(r, cow);
    }

    #[test]
    fn from_rope_slice_01() {
        let r1 = Rope::from_str(TEXT);
        let s = r1.slice(..);
        let r2: Rope = s.into();

        assert_eq!(r1, r2);
        assert_eq!(s, r2);
    }

    #[test]
    fn from_rope_slice_02() {
        let r1 = Rope::from_str(TEXT);
        let s = r1.slice(0..24);
        let r2: Rope = s.into();

        assert_eq!(s, r2);
    }

    #[test]
    fn from_rope_slice_03() {
        let r1 = Rope::from_str(TEXT);
        let s = r1.slice(13..89);
        let r2: Rope = s.into();

        assert_eq!(s, r2);
    }

    #[test]
    fn from_rope_slice_04() {
        let r1 = Rope::from_str(TEXT);
        let s = r1.slice(13..41);
        let r2: Rope = s.into();

        assert_eq!(s, r2);
    }

    #[test]
    fn from_iter_01() {
        let r1 = Rope::from_str(TEXT);
        let r2: Rope = Rope::from_iter(r1.chunks());

        assert_eq!(r1, r2);
    }

    // Iterator tests are in the iter module
}
