#![allow(dead_code)]

use std;
use std::io;
use std::sync::Arc;
use std::ptr;

use child_array::ChildArray;

use iter::{RopeBytes, RopeChars, RopeGraphemes, RopeLines, RopeChunks};
use node::{Node, MAX_BYTES};
use rope_builder::RopeBuilder;
use slice::RopeSlice;
use str_utils::char_idx_to_byte_idx;
use text_info::Count;


/// A utf8 text rope.
#[derive(Debug, Clone)]
pub struct Rope {
    pub(crate) root: Arc<Node>,
}

impl Rope {
    /// Creates an empty `Rope`.
    pub fn new() -> Rope {
        Rope { root: Arc::new(Node::new()) }
    }

    /// Creates a `Rope` from a string slice.
    pub fn from_str(text: &str) -> Rope {
        let mut builder = RopeBuilder::new();
        builder.append(text);
        builder.finish()
    }

    /// Creates a `Rope` from an arbitrary reader.
    ///
    /// This expects utf8 data, and will fail if the reader provides
    /// anything else.
    ///
    /// Returns None if it fails.
    pub fn from_reader_utf8<T: io::Read>(reader: &mut T) -> Option<Rope> {
        // TODO: return a proper Result type that propagates errors.
        const BUFFER_SIZE: usize = MAX_BYTES * 2;
        let mut builder = RopeBuilder::new();
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut fill_idx = 0; // How much `buffer` is currently filled with valid data
        loop {
            match reader.read(&mut buffer[fill_idx..]) {
                Ok(read_count) => {
                    fill_idx = fill_idx + read_count;

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
                        return None;
                    }

                    // If we're done reading
                    if read_count == 0 {
                        if fill_idx > 0 {
                            // We couldn't consume all data.
                            return None;
                        } else {
                            return Some(builder.finish());
                        }
                    }
                }

                Err(_) => {
                    // Read error
                    return None;
                }
            }
        }
    }

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

    /// Inserts the given text at char index `char_idx`.
    pub fn insert(&mut self, char_idx: usize, text: &str) {
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

                let mut children = ChildArray::new();
                children.push((l_node.text_info(), Arc::new(l_node)));
                children.push((r_node.text_info(), Arc::new(r_node)));

                *root = Node::Internal(children);
            }

            // Handle seam, if any.
            if let Some(byte_pos) = seam {
                root.fix_grapheme_seam(byte_pos);
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
            let right = self.split(char_idx);
            self.append(text_rope);
            self.append(right);
        }
    }

    /// Removes the text in char range `start..end`.
    pub fn remove(&mut self, start: usize, end: usize) {
        Arc::make_mut(&mut self.root).remove(start, end);
    }

    /// Returns the char index of the given byte.
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.root.byte_to_char(byte_idx)
    }

    /// Returns the line index of the given byte.
    pub fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.root.byte_to_line(byte_idx)
    }

    /// Returns the byte index of the given char.
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.root.char_to_byte(char_idx) as usize
    }

    /// Returns the line index of the given char.
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.root.char_to_line(char_idx)
    }

    /// Returns the byte index of the start of the given line.
    pub fn line_to_byte(&self, line_idx: usize) -> usize {
        self.root.line_to_byte(line_idx)
    }

    /// Returns the char index of the start of the given line.
    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.root.line_to_char(line_idx)
    }

    /// Returns whether the given char index is a grapheme cluster
    /// boundary or not.
    pub fn is_grapheme_boundary(&self, char_idx: usize) -> bool {
        self.root.is_grapheme_boundary(char_idx)
    }

    /// Returns the previous grapheme cluster boundary to the left of
    /// the given char index (excluding the given char index itself).
    ///
    /// If `char_idx` is at the beginning of the rope, returns 0.
    pub fn prev_grapheme_boundary(&self, char_idx: usize) -> usize {
        self.root.prev_grapheme_boundary(char_idx)
    }

    /// Returns the next grapheme cluster boundary to the right of
    /// the given char index (excluding the given char index itself).
    ///
    /// If `char_idx` is at the end of the rope, returns the end
    /// position.
    pub fn next_grapheme_boundary(&self, char_idx: usize) -> usize {
        self.root.next_grapheme_boundary(char_idx)
    }

    /// Returns an immutable slice of the `Rope` in the char range `start..end`.
    pub fn slice<'a>(&'a self, start: usize, end: usize) -> RopeSlice<'a> {
        self.root.slice(start, end)
    }

    /// Creates an iterator over the bytes of the `Rope`.
    pub fn bytes<'a>(&'a self) -> RopeBytes<'a> {
        RopeBytes::new(&self.root)
    }

    /// Creates an iterator over the chars of the `Rope`.
    pub fn chars<'a>(&'a self) -> RopeChars<'a> {
        RopeChars::new(&self.root)
    }

    /// Creates an iterator over the grapheme clusters of the `Rope`.
    pub fn graphemes<'a>(&'a self) -> RopeGraphemes<'a> {
        RopeGraphemes::new(&self.root, true)
    }

    /// Creates an iterator over the lines of the `Rope`.
    pub fn lines<'a>(&'a self) -> RopeLines<'a> {
        RopeLines::new(&self.root)
    }

    /// Creates an iterator over the chunks of the `Rope`.
    pub fn chunks<'a>(&'a self) -> RopeChunks<'a> {
        RopeChunks::new(&self.root)
    }

    /// Returns the entire text of the `Rope` as a newly allocated String.
    pub fn to_string(&self) -> String {
        use iter::RopeChunks;
        let mut text = String::new();
        for chunk in RopeChunks::new(&self.root) {
            text.push_str(chunk);
        }
        text
    }

    /// Splits the `Rope` at char index `split_char_idx`.
    ///
    /// The left side of the split remians in this `Rope`, and
    /// the right side is returned as a new `Rope`.
    pub fn split(&mut self, split_char_idx: usize) -> Rope {
        // Do the split
        let mut new_rope_root = Arc::new(Arc::make_mut(&mut self.root).split(split_char_idx));

        // Fix up the edges
        Arc::make_mut(&mut self.root).zip_right();
        Arc::make_mut(&mut new_rope_root).zip_left();

        // Pull up singular nodes
        while (!self.root.is_leaf()) && self.root.child_count() == 1 {
            let child = if let Node::Internal(ref children) = *self.root {
                children.nodes()[0].clone()
            } else {
                unreachable!()
            };

            self.root = child;
        }

        while (!new_rope_root.is_leaf()) && new_rope_root.child_count() == 1 {
            let child = if let Node::Internal(ref children) = *new_rope_root {
                children.nodes()[0].clone()
            } else {
                unreachable!()
            };

            new_rope_root = child;
        }

        // Return right rope
        Rope { root: new_rope_root }
    }

    /// Appends a `Rope` to the end of this one, consuming the other `Rope`.
    pub fn append(&mut self, other: Rope) {
        let seam_byte_i = self.root.text_info().bytes;

        let l_depth = self.root.depth();
        let r_depth = other.root.depth();

        if l_depth > r_depth {
            let extra =
                Arc::make_mut(&mut self.root).append_at_depth(other.root, l_depth - r_depth);
            if let Some(node) = extra {
                let mut children = ChildArray::new();
                children.push((self.root.text_info(), self.root.clone()));
                children.push((node.text_info(), node));
                self.root = Arc::new(Node::Internal(children));
            }
        } else {
            let mut other = other;
            let extra = Arc::make_mut(&mut other.root).prepend_at_depth(
                self.root.clone(),
                r_depth - l_depth,
            );
            if let Some(node) = extra {
                let mut children = ChildArray::new();
                children.push((node.text_info(), node));
                children.push((other.root.text_info(), other.root.clone()));
                other.root = Arc::new(Node::Internal(children));
            }
            *self = other;
        };

        Arc::make_mut(&mut self.root).fix_grapheme_seam(seam_byte_i);
    }

    //--------------

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
        self.root.assert_invariants(true);
    }
}

//==============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_01() {
        let mut r = Rope::new();
        r.insert(0, "Hello world!");
        r.insert(3, "zopter");

        assert_eq!("Helzopterlo world!", &r.to_string());
    }

    #[test]
    fn insert_02() {
        let mut r = Rope::new();
        r.insert(0, "Hello world!");
        r.insert(0, "zopter");

        assert_eq!("zopterHello world!", &r.to_string());
    }

    #[test]
    fn insert_03() {
        let mut r = Rope::new();
        r.insert(0, "Hello world!");
        r.insert(12, "zopter");

        assert_eq!("Hello world!zopter", &r.to_string());
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

        assert_eq!("Helzopterlo world!", &r.to_string());
    }

    #[test]
    fn insert_05() {
        let mut r = Rope::new();
        r.insert(0, "こんいちは、みんなさん！");
        r.insert(7, "zopter");
        assert_eq!("こんいちは、みzopterんなさん！", &r.to_string());
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
        assert_eq!("こんいちは、みzopterんなさん！", &r.to_string());
    }

    #[test]
    fn remove_01() {
        let mut r = Rope::from_str(
            "Hello world! How are you doing? こんいちは、みんなさん！",
        );

        r.remove(5, 11);
        r.remove(24, 31);
        r.remove(19, 25);
        assert_eq!("Hello! How are you みんなさん！", &r.to_string());

        // r.assert_integrity();
        // r.assert_invariants();
    }

    #[test]
    fn split_01() {
        let mut r = Rope::from_str(
            "Hello world! How are you doing? こんいちは、みんなさん！",
        );

        let r2 = r.split(20);
        assert_eq!("Hello world! How are", &r.to_string());
        assert_eq!(
            " you doing? こんいちは、みんなさん！",
            &r2.to_string()
        );

        r.assert_integrity();
        r2.assert_integrity();
        r.assert_invariants();
        r2.assert_invariants();
    }

    #[test]
    fn split_02() {
        let mut r = Rope::from_str(
            "Hello world! How are you doing? こんいちは、みんなさん！",
        );

        let r2 = r.split(0);
        assert_eq!("", &r.to_string());
        assert_eq!(
            "Hello world! How are you doing? こんいちは、みんなさん！",
            &r2.to_string()
        );

        r.assert_integrity();
        r2.assert_integrity();
        r.assert_invariants();
        r2.assert_invariants();
    }

    #[test]
    fn split_03() {
        let mut r = Rope::from_str(
            "Hello world! How are you doing? こんいちは、みんなさん！",
        );

        let r2 = r.split(44);
        assert_eq!(
            "Hello world! How are you doing? こんいちは、みんなさん！",
            &r.to_string()
        );
        assert_eq!("", &r2.to_string());

        r.assert_integrity();
        r2.assert_integrity();
        r.assert_invariants();
        r2.assert_invariants();
    }

    #[test]
    fn append_01() {
        let mut r = Rope::from_str("Hello world! How are");
        let r2 = Rope::from_str(" you doing? こんいちは、みんなさん！");

        r.append(r2);
        assert_eq!(
            &r.to_string(),
            "Hello world! How are you doing? こんいちは、みんなさん！"
        );

        r.assert_integrity();
        r.assert_invariants();
    }

    #[test]
    fn append_02() {
        let mut r = Rope::from_str("Hello world! How are you doing? こんい");
        let r2 = Rope::from_str("ちは、みんなさん！");

        r.append(r2);
        assert_eq!(
            &r.to_string(),
            "Hello world! How are you doing? こんいちは、みんなさん！"
        );

        r.assert_integrity();
        r.assert_invariants();
    }
}
