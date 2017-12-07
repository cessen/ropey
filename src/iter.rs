#![allow(dead_code)]

use std::str::{Bytes, Chars};

use node::Node;
use slice::RopeSlice;

//==========================================================

/// An iterator over a Rope's bytes.
pub struct RopeBytes<'a> {
    chunk_iter: RopeChunks<'a>,
    cur_chunk: Bytes<'a>,
}

impl<'a> RopeBytes<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeBytes<'b> {
        RopeBytes {
            chunk_iter: RopeChunks::new(node),
            cur_chunk: "".bytes(),
        }
    }
}

impl<'a> Iterator for RopeBytes<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        loop {
            if let Some(c) = self.cur_chunk.next() {
                return Some(c);
            } else {
                if let Some(chunk) = self.chunk_iter.next() {
                    self.cur_chunk = chunk.bytes();
                    continue;
                } else {
                    return None;
                }
            }
        }
    }
}

//==========================================================

/// An iterator over a Rope's chars.
pub struct RopeChars<'a> {
    chunk_iter: RopeChunks<'a>,
    cur_chunk: Chars<'a>,
}

impl<'a> RopeChars<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeChars<'b> {
        RopeChars {
            chunk_iter: RopeChunks::new(node),
            cur_chunk: "".chars(),
        }
    }
}

impl<'a> Iterator for RopeChars<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        loop {
            if let Some(c) = self.cur_chunk.next() {
                return Some(c);
            } else {
                if let Some(chunk) = self.chunk_iter.next() {
                    self.cur_chunk = chunk.chars();
                    continue;
                } else {
                    return None;
                }
            }
        }
    }
}

//==========================================================

/// An iterator over a Rope's chars.
pub struct RopeLines<'a> {
    node: &'a Node,
    line_idx: usize,
}

impl<'a> RopeLines<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeLines<'b> {
        RopeLines {
            node: node,
            line_idx: 0,
        }
    }
}

impl<'a> Iterator for RopeLines<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<RopeSlice<'a>> {
        if self.line_idx > self.node.line_break_count() {
            return None;
        } else {
            let a = self.node.line_to_char(self.line_idx);
            let b = if self.line_idx < self.node.line_break_count() {
                self.node.line_to_char(self.line_idx + 1)
            } else {
                self.node.char_count()
            };

            self.line_idx += 1;

            return Some(self.node.slice(a, b));
        }
    }
}

//==========================================================

/// An iterator over a Rope's contiguous str chunks.
pub struct RopeChunks<'a> {
    node_stack: Vec<&'a Node>,
}

impl<'a> RopeChunks<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeChunks<'b> {
        RopeChunks { node_stack: vec![node] }
    }
}

impl<'a> Iterator for RopeChunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        loop {
            match self.node_stack.pop() {
                Some(&Node::Leaf(ref text)) => {
                    return Some(text);
                }

                Some(&Node::Internal { ref children, .. }) => {
                    for c in children.iter().rev() {
                        self.node_stack.push(c);
                    }
                }

                _ => {
                    return None;
                }
            }
        }
    }
}
