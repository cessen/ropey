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
use text_info::Count;


/// A utf8 text rope.
#[derive(Debug, Clone)]
pub struct Rope {
    pub(crate) root: Arc<Node>,
}

impl Rope {
    /// Creates an empty Rope.
    pub fn new() -> Rope {
        Rope { root: Arc::new(Node::new()) }
    }

    pub fn from_str(text: &str) -> Rope {
        let mut builder = RopeBuilder::new();
        builder.append(text);
        builder.finish()
    }

    /// Creates a Rope from an arbitrary input source.
    ///
    /// This can fail, since it expects utf8 input.  Returns
    /// None if it fails.
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

    /// Total number of bytes in the Rope.
    pub fn len_bytes(&self) -> usize {
        self.root.byte_count()
    }

    /// Total number of chars in the Rope.
    pub fn len_chars(&self) -> usize {
        self.root.char_count()
    }

    /// Total number of lines in the Rope.
    pub fn len_lines(&self) -> usize {
        self.root.line_break_count() + 1
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

    /// Returns an immutable slice of the Rope in the char range `start..end`.
    pub fn slice<'a>(&'a self, start: usize, end: usize) -> RopeSlice<'a> {
        self.root.slice(start, end)
    }

    /// Creates an iterator over the bytes of the Rope.
    pub fn bytes<'a>(&'a self) -> RopeBytes<'a> {
        RopeBytes::new(&self.root)
    }

    /// Creates an iterator over the chars of the Rope.
    pub fn chars<'a>(&'a self) -> RopeChars<'a> {
        RopeChars::new(&self.root)
    }

    /// Creates an iterator over the grapheme clusteres of the Rope.
    pub fn graphemes<'a>(&'a self) -> RopeGraphemes<'a> {
        RopeGraphemes::new(&self.root, true)
    }

    /// Creates an iterator over the lines of the Rope.
    pub fn lines<'a>(&'a self) -> RopeLines<'a> {
        RopeLines::new(&self.root)
    }

    /// Creates an iterator over the chunks of the Rope.
    pub fn chunks<'a>(&'a self) -> RopeChunks<'a> {
        RopeChunks::new(&self.root)
    }

    /// Returns the entire text of the Rope as a newly allocated String.
    pub fn to_string(&self) -> String {
        use iter::RopeChunks;
        let mut text = String::new();
        for chunk in RopeChunks::new(&self.root) {
            text.push_str(chunk);
        }
        text
    }

    /// Inserts the given text at char index `char_idx`.
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        let root = Arc::make_mut(&mut self.root);

        // Do the insertion
        let (residual, seam) = root.insert(char_idx as Count, text);

        // Handle root splitting, if any.
        if let Some(r_node) = residual {
            let mut l_node = Node::Empty;
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
    }

    /// Removes the text in char range `start..end`.
    pub fn remove(&mut self, start: usize, end: usize) {
        let _ = (start, end);
        unimplemented!()
    }

    /// Splits the Rope at char index `split_char_idx`.
    ///
    /// The left side of the split remians in this Rope, and
    /// the right side is returned as a new Rope.
    pub fn split(&mut self, split_char_idx: usize) -> Rope {
        let _ = split_char_idx;
        unimplemented!()
    }

    /// Appends a Rope to the end of this one, consuming the other Rope.
    pub fn append(&mut self, other: Rope) {
        let _ = other;
        unimplemented!()
    }

    //--------------

    /// Debugging tool to make sure that all of the meta-data of the
    /// tree is consistent with the actual data.
    pub(crate) fn verify_integrity(&self) {
        self.root.verify_integrity();
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
}
