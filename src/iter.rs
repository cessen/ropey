use crate::tree::Node;

#[derive(Debug, Clone)]
pub struct Chunks<'a> {
    node_stack: Vec<(&'a Node, usize)>, // (node ref, index of current child)
    byte_range: [usize; 2],
    current_byte_idx: usize,
}

impl<'a> Chunks<'a> {
    #[inline(always)]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], mut at_byte_idx: usize) -> Chunks {
        debug_assert!(byte_range[0] <= at_byte_idx && at_byte_idx <= byte_range[1]);

        // Special case: if it's an empty rope, don't store anything.
        if byte_range[0] == byte_range[1] || node.is_empty() {
            return Chunks {
                node_stack: vec![],
                byte_range: [0, 0],
                current_byte_idx: 0,
            };
        }

        let mut chunks = Chunks {
            node_stack: vec![],
            byte_range: byte_range,
            current_byte_idx: 0,
        };

        let mut current_node = node;
        let mut current_byte_offset = 0;
        loop {
            match current_node {
                &Node::Leaf(_) => {
                    chunks.node_stack.push((current_node, 0));
                    chunks.current_byte_idx = current_byte_offset;
                    break;
                }

                &Node::Internal(ref children) => {
                    let (child_i, acc_byte_idx) = children.search_byte_idx_only(at_byte_idx);

                    current_byte_offset += acc_byte_idx;
                    at_byte_idx -= acc_byte_idx;

                    chunks.node_stack.push((current_node, child_i));
                    current_node = &children.nodes()[child_i];
                }
            }
        }

        chunks
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<&'a str> {
        // Already at the end, or it's an empty rope.
        if self.current_byte_idx >= self.byte_range[1] || self.node_stack.is_empty() {
            return None;
        }

        // Prepare the chunk text.
        let text = self.node_stack.last().unwrap().0.leaf_text();
        let trimmed_chunk = {
            let mut chunk = text.text();
            if (self.current_byte_idx + chunk.len()) > self.byte_range[1] {
                chunk = &chunk[..(self.byte_range[1] - self.current_byte_idx)];
            }
            if self.current_byte_idx < self.byte_range[0] {
                chunk = &chunk[(self.byte_range[0] - self.current_byte_idx)..];
            }
            chunk
        };

        // Update the byte index.
        self.current_byte_idx += text.len();

        // If we didn't reach the end, progress the node stack.
        if self.current_byte_idx < self.byte_range[1] && self.node_stack.len() > 1 {
            self.node_stack.pop();

            // Find the deepest node that's not at it's end already.
            while self.node_stack.last().unwrap().1
                >= (self.node_stack.last().unwrap().0.child_count() - 1)
            {
                if self.node_stack.len() == 1 {
                    // This would leave the stack empty if we popped, and should
                    // be impossible to reach due to checking our position in
                    // the text against the byte range above.
                    unreachable!();
                }
                self.node_stack.pop();
            }

            // Refill the stack starting from that node.
            self.node_stack.last_mut().unwrap().1 += 1;
            while self.node_stack.last().unwrap().0.is_internal() {
                let child_i = self.node_stack.last().unwrap().1;
                let node = &self.node_stack.last().unwrap().0.children().nodes()[child_i];
                self.node_stack.push((node, 0));
            }
        }

        // Finally, return the prepared chunk text.
        Some(trimmed_chunk)
    }
}

#[cfg(test)]
mod tests {
    use crate::{rope_builder::RopeBuilder, Rope};

    fn hello_world_repeat_rope() -> Rope {
        let mut rb = RopeBuilder::new();
        for _ in 0..4 {
            rb._append_chunk_as_leaf("Hello ");
            rb._append_chunk_as_leaf("world!");
        }
        rb.finish()
    }

    #[test]
    fn chunks_iter_01() {
        let r = hello_world_repeat_rope();

        let mut chunks = r.chunks();
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(None, chunks.next());
    }

    #[test]
    fn chunks_iter_02() {
        let r = Rope::from_str("");

        let mut chunks = r.chunks();
        assert_eq!(None, chunks.next());
    }

    #[test]
    fn chunks_iter_03() {
        let r = hello_world_repeat_rope();
        let s = r.slice(3..45);

        let mut chunks = s.chunks();
        assert_eq!(Some("lo "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("wor"), chunks.next());
        assert_eq!(None, chunks.next());
    }

    #[test]
    fn chunks_iter_04() {
        let r = hello_world_repeat_rope();
        let s = r.slice(8..40);

        let mut chunks = s.chunks();
        assert_eq!(Some("rld!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hell"), chunks.next());
        assert_eq!(None, chunks.next());
    }

    #[test]
    fn chunks_iter_05() {
        let r = hello_world_repeat_rope();
        let s = r.slice(14..14);

        let mut chunks = s.chunks();
        assert_eq!(None, chunks.next());
    }
}
