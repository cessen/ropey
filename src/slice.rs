#![allow(dead_code)]

use iter::{RopeBytes, RopeChars, RopeLines, RopeChunks};
use node::Node;

/// An immutable view into part of a Rope.
pub struct RopeSlice<'a> {
    node: &'a Node,
    start_char: usize,
    end_char: usize,
}

impl<'a> RopeSlice<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeSlice<'b> {
        RopeSlice {
            node: node,
            start_char: 0,
            end_char: node.text_info().chars as usize,
        }
    }

    pub(crate) fn new_with_range<'b>(node: &'b Node, start: usize, end: usize) -> RopeSlice<'b> {
        assert!(start <= end);
        assert!(end <= node.text_info().chars as usize);

        // Find the deepest node that still contains the full range given.
        let mut n_start = start;
        let mut n_end = end;
        let mut node = node;
        'outer: loop {
            match node as &Node {
                &Node::Empty | &Node::Leaf(_) => break,

                &Node::Internal {
                    ref info,
                    ref children,
                } => {
                    let mut start_char = 0;
                    for (i, inf) in info.iter().enumerate() {
                        if n_start >= start_char && n_end < (start_char + inf.chars as usize) {
                            n_start -= start_char;
                            n_end -= start_char;
                            node = &children[i];
                            continue 'outer;
                        }
                        start_char += inf.chars as usize;
                    }
                    break;
                }
            }
        }

        // Create the slice
        RopeSlice {
            node: node,
            start_char: n_start,
            end_char: n_end,
        }
    }

    /// Total number of bytes in the RopeSlice.
    pub fn len_bytes(&self) -> usize {
        self.node.char_to_byte(self.end_char) - self.node.char_to_byte(self.start_char)
    }

    /// Total number of chars in the RopeSlice.
    pub fn len_chars(&self) -> usize {
        self.end_char - self.start_char
    }

    /// Total number of lines in the RopeSlice.
    pub fn len_lines(&self) -> usize {
        self.node.char_to_line(self.end_char) - self.node.char_to_line(self.start_char) + 1
    }

    /// Returns an immutable slice of the RopeSlice in the char range `start..end`.
    pub fn slice(&self, start: usize, end: usize) -> RopeSlice<'a> {
        assert!(start <= end);
        assert!(end <= (self.end_char - self.start_char));
        RopeSlice::new_with_range(self.node, self.start_char + start, self.start_char + end)
    }

    /// Creates an iterator over the bytes of the RopeSlice.
    pub fn bytes(&self) -> RopeBytes<'a> {
        RopeBytes::new_with_range(self.node, self.start_char, self.end_char)
    }

    /// Creates an iterator over the chars of the RopeSlice.
    pub fn chars(&self) -> RopeChars<'a> {
        RopeChars::new_with_range(self.node, self.start_char, self.end_char)
    }

    /// Creates an iterator over the lines of the RopeSlice.
    pub fn lines(&self) -> RopeLines<'a> {
        RopeLines::new_with_range(self.node, self.start_char, self.end_char)
    }

    /// Creates an iterator over the chunks of the RopeSlice.
    pub fn chunks(&self) -> RopeChunks<'a> {
        RopeChunks::new_with_range(self.node, self.start_char, self.end_char)
    }

    /// Returns the entire text of the RopeSlice as a newly allocated String.
    pub fn to_string(&self) -> String {
        let mut text = String::new();
        for chunk in self.chunks() {
            text.push_str(chunk);
        }
        text
    }
}

//===========================================================

#[cfg(test)]
mod tests {
    use rope::Rope;

    #[test]
    fn slice_01() {
        let mut r = Rope::new();
        let text = "Hello there!  How're you doing?  It's a fine day, isn't it?  \
                    Aren't you glad we're alive?";

        for c in text.chars().rev() {
            r.insert(0, &c.to_string());
        }

        let s = r.slice(0, r.len_chars());

        assert_eq!(text, &s.to_string());
    }

    #[test]
    fn slice_02() {
        let mut r = Rope::new();
        let text = "Hello there!  How're you doing?  It's a fine day, isn't it?  \
                    Aren't you glad we're alive?";

        for c in text.chars().rev() {
            r.insert(0, &c.to_string());
        }

        let s = r.slice(5, 21);

        assert_eq!(&text[5..21], &s.to_string());
    }
}
