use crate::tree::Node;

#[derive(Debug, Clone)]
pub struct Chunks<'a> {
    node_stack: Vec<(&'a Node, usize)>, // (node ref, index of current child)
}

impl<'a> Chunks<'a> {
    #[inline(always)]
    pub(crate) fn new(node: &Node) -> Chunks {
        Chunks {
            node_stack: vec![(node, 0)],
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<&'a str> {
        while !self.node_stack.is_empty() {
            // Leaf.
            if self.node_stack.last().unwrap().0.is_leaf() {
                let (text, _) = self.node_stack.pop().unwrap();
                return Some(text.leaf_text_chunk());
            }

            let i = self.node_stack.len() - 1;
            if self.node_stack[i].1 >= self.node_stack[i].0.child_count() {
                // If the current node is out of children.
                self.node_stack.pop();
            } else {
                let child = &self.node_stack[i].0.children().nodes()[self.node_stack[i].1];
                self.node_stack[i].1 += 1;
                self.node_stack.push((child, 0));
            }
        }

        return None;
    }
}
