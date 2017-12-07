#![allow(dead_code)]

use std;
use std::sync::Arc;

use arrayvec::ArrayVec;

use iter::{RopeBytes, RopeChars, RopeLines, RopeChunks};
use node::Node;
use slice::RopeSlice;
use text_info::Count;


#[derive(Debug, Clone)]
pub struct Rope {
    pub(crate) root: Arc<Node>,
}

impl Rope {
    /// Creates an empty Rope.
    pub fn new() -> Rope {
        use std::mem::size_of;
        println!("Node size: {:?}", size_of::<Node>());
        Rope { root: Arc::new(Node::new()) }
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

            let mut info = ArrayVec::new();
            info.push(l_node.text_info());
            info.push(r_node.text_info());

            let mut children = ArrayVec::new();
            children.push(Arc::new(l_node));
            children.push(Arc::new(r_node));

            *root = Node::Internal {
                info: info,
                children: children,
            };
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
