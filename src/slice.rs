use rope::Node;

/// An immutable view into part of a Rope.
pub struct RopeSlice<'a> {
    node: &'a Node,
    start: usize,
    end: usize,
}

impl<'a> RopeSlice<'a> {
    pub(crate) fn new_from_node<'b>(node: &'b Node, start: usize, end: usize) -> RopeSlice<'b> {
        assert!(start <= end);
        assert!(end < node.text_info().chars as usize);

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
            start: n_start,
            end: n_end,
        }
    }

    /// Returns an immutable slice of the RopeSlice in the char range `start..end`.
    pub fn slice(&self, start: usize, end: usize) -> RopeSlice<'a> {
        assert!(start <= end);
        assert!(end < (self.end - self.start));
        RopeSlice::new_from_node(self.node, self.start + start, self.start + end)
    }

    /// Creates an iterator over the bytes of the RopeSlice.
    pub fn bytes<'a>(&'a self) -> RopeBytes<'a> {
        unimplemented!()
    }

    /// Creates an iterator over the chars of the RopeSlice.
    pub fn chars<'a>(&'a self) -> RopeChars<'a> {
        unimplemented!()
    }

    /// Creates an iterator over the lines of the RopeSlice.
    pub fn lines<'a>(&'a self) -> RopeLines<'a> {
        unimplemented!()
    }

    /// Creates an iterator over the chunks of the RopeSlice.
    pub fn chunks<'a>(&'a self) -> RopeChunks<'a> {
        unimplemented!()
    }
}
