#![allow(dead_code)]

use std::marker::PhantomData;
use std::str::Chars;

use rope::{Rope, Node};

/// An iterator over a Rope's contiguous str chunks.
pub struct RopeChunkIter<'a> {
    node_stack: Vec<&'a Node>,
    _rope: PhantomData<&'a Rope>,
}

impl<'a> RopeChunkIter<'a> {
    pub(crate) fn new<'b>(r: &'b Rope) -> RopeChunkIter<'b> {
        RopeChunkIter {
            node_stack: vec![&r.root],
            _rope: PhantomData,
        }
    }
}

impl<'a> Iterator for RopeChunkIter<'a> {
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


/// An iterator over a Rope's chars.
pub struct RopeCharIter<'a> {
    chunk_iter: RopeChunkIter<'a>,
    cur_chunk: Chars<'a>,
}

impl<'a> RopeCharIter<'a> {
    pub(crate) fn new<'b>(r: &'b Rope) -> RopeCharIter<'b> {
        RopeCharIter {
            chunk_iter: RopeChunkIter::new(r),
            cur_chunk: "".chars(),
        }
    }
}

impl<'a> Iterator for RopeCharIter<'a> {
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
