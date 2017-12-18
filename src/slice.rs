#![allow(dead_code)]

use std;

use iter::{RopeBytes, RopeChars, RopeGraphemes, RopeLines, RopeChunks};
use node::Node;

/// An immutable view into part of a `Rope`.
#[derive(Copy, Clone)]
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
                &Node::Leaf(_) => break,

                &Node::Internal(ref children) => {
                    let mut start_char = 0;
                    for (i, inf) in children.info().iter().enumerate() {
                        if n_start >= start_char && n_end < (start_char + inf.chars as usize) {
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
        RopeSlice {
            node: node,
            start_char: n_start,
            end_char: n_end,
        }
    }

    /// Total number of bytes in the `RopeSlice`.
    pub fn len_bytes(&self) -> usize {
        self.node.char_to_byte(self.end_char) - self.node.char_to_byte(self.start_char)
    }

    /// Total number of chars in the `RopeSlice`.
    pub fn len_chars(&self) -> usize {
        self.end_char - self.start_char
    }

    /// Total number of lines in the `RopeSlice`.
    pub fn len_lines(&self) -> usize {
        self.node.char_to_line(self.end_char) - self.node.char_to_line(self.start_char) + 1
    }

    /// Returns an immutable slice of the `RopeSlice` in the char range `start..end`.
    pub fn slice(&self, start: usize, end: usize) -> RopeSlice<'a> {
        assert!(start <= end);
        assert!(end <= (self.end_char - self.start_char));
        RopeSlice::new_with_range(self.node, self.start_char + start, self.start_char + end)
    }

    /// Creates an iterator over the bytes of the `RopeSlice`.
    pub fn bytes(&self) -> RopeBytes<'a> {
        RopeBytes::new_with_range(self.node, self.start_char, self.end_char)
    }

    /// Creates an iterator over the chars of the `RopeSlice`.
    pub fn chars(&self) -> RopeChars<'a> {
        RopeChars::new_with_range(self.node, self.start_char, self.end_char)
    }

    /// Creates an iterator over the grapheme clusters of the `RopeSlice`.
    pub fn graphemes(&self) -> RopeGraphemes<'a> {
        RopeGraphemes::new_with_range(self.node, true, self.start_char, self.end_char)
    }

    /// Creates an iterator over the lines of the `RopeSlice`.
    pub fn lines(&self) -> RopeLines<'a> {
        RopeLines::new_with_range(self.node, self.start_char, self.end_char)
    }

    /// Creates an iterator over the chunks of the `RopeSlice`.
    pub fn chunks(&self) -> RopeChunks<'a> {
        RopeChunks::new_with_range(self.node, self.start_char, self.end_char)
    }

    /// Returns the entire text of the `RopeSlice` as a newly allocated `String`.
    pub fn to_string(&self) -> String {
        let mut text = String::new();
        for chunk in self.chunks() {
            text.push_str(chunk);
        }
        text
    }
}

//==============================================================

impl<'a> std::fmt::Debug for RopeSlice<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list().entries(self.chunks()).finish()
    }
}

impl<'a> std::fmt::Display for RopeSlice<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for chunk in self.chunks() {
            write!(f, "{}", chunk)?
        }
        Ok(())
    }
}

impl<'a> std::cmp::PartialEq<RopeSlice<'a>> for RopeSlice<'a> {
    fn eq(&self, other: &RopeSlice) -> bool {
        if self.len_bytes() != other.len_bytes() {
            return false;
        }

        let mut chunk_itr_1 = self.chunks();
        let mut chunk_itr_2 = other.chunks();
        let mut chunk1 = chunk_itr_1.next().unwrap();
        let mut chunk2 = chunk_itr_2.next().unwrap();

        loop {
            if chunk1.len() > chunk2.len() {
                if &chunk1[..chunk2.len()] != chunk2 {
                    return false;
                } else {
                    chunk1 = &chunk1[chunk2.len()..];
                    chunk2 = "";
                }
            } else {
                if &chunk2[..chunk1.len()] != chunk1 {
                    return false;
                } else {
                    chunk2 = &chunk2[chunk1.len()..];
                    chunk1 = "";
                }
            }

            if chunk1.len() == 0 {
                if let Some(chunk) = chunk_itr_1.next() {
                    chunk1 = chunk;
                } else {
                    break;
                }
            }

            if chunk2.len() == 0 {
                if let Some(chunk) = chunk_itr_2.next() {
                    chunk2 = chunk;
                } else {
                    break;
                }
            }
        }

        return true;
    }
}

impl<'a> std::cmp::PartialEq<&'a str> for RopeSlice<'a> {
    fn eq(&self, other: &&'a str) -> bool {
        if self.len_bytes() != other.len() {
            return false;
        }

        let mut idx = 0;
        for chunk in self.chunks() {
            if chunk != &other[idx..(idx + chunk.len())] {
                return false;
            }
            idx += chunk.len();
        }

        return true;
    }
}

impl<'a> std::cmp::PartialEq<RopeSlice<'a>> for &'a str {
    fn eq(&self, other: &RopeSlice<'a>) -> bool {
        other == self
    }
}

//===========================================================

#[cfg(test)]
mod tests {
    use rope::Rope;

    #[test]
    fn slice_01() {
        let text = "Hello there!  How're you doing?  It's a fine day, isn't it?  \
                    Aren't you glad we're alive?";
        let r = Rope::from_str(text);

        let s = r.slice(0, r.len_chars());

        assert_eq!(text, &s.to_string());
    }

    #[test]
    fn slice_02() {
        let text = "Hello there!  How're you doing?  It's a fine day, isn't it?  \
                    Aren't you glad we're alive?";
        let r = Rope::from_str(text);

        let s = r.slice(5, 21);

        assert_eq!(&text[5..21], &s.to_string());
    }
}
