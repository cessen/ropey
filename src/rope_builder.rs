use std::sync::Arc;

use crate::rope::Rope;
use crate::tree::{Children, Node, Text, MAX_CHILDREN, MAX_TEXT_SIZE, MIN_CHILDREN};

/// An efficient incremental `Rope` builder.
///
/// This is used to efficiently build ropes from sequences of text
/// chunks.  It is useful for creating ropes from:
///
/// - ...large text files, without pre-loading their entire contents into memory
///   (but see [`from_reader()`](crate::Rope::from_reader) for a convenience
///   function that does this for casual use cases).
/// - ...streaming data sources.
/// - ...non-utf8 text data, doing the encoding conversion incrementally
///   as you go.
///
/// Unlike repeatedly calling `Rope::insert()` on the end of a rope,
/// this API runs in time linear to the amount of data fed to it, and
/// is overall much faster.
///
/// # Example
/// ```
/// # use ropey::RopeBuilder;
/// #
/// let mut builder = RopeBuilder::new();
///
/// builder.append("Hello ");
/// builder.append("world!\n");
/// builder.append("How's ");
/// builder.append("it goin");
/// builder.append("g?");
///
/// let rope = builder.finish();
///
/// assert_eq!(rope, "Hello world!\nHow's it going?");
/// ```
#[derive(Debug, Clone)]
pub struct RopeBuilder {
    // The stack represents the right-most path down the tree of the rope,
    // starting from the root at vec index 0, and terminating in the deepest,
    // right-most non-leaf node.  The idea is that as we append more and more
    // nodes, the right-most nodes down the tree fill up, and as each one
    // fills up it gets added as a child of the node above it in the stack and
    // replaced with a fresh new node.
    //
    // There is one except to the stack termining in a non-leaf node: when a
    // leaf is the *only* node in the tree so far (i.e. the root).
    stack: Vec<Node>,

    buffer: String,
}

impl RopeBuilder {
    /// Creates a new RopeBuilder, ready for input.
    pub fn new() -> Self {
        RopeBuilder {
            stack: Vec::new(),
            buffer: String::new(),
        }
    }

    /// Appends `chunk` to the end of the in-progress `Rope`.
    ///
    /// Call this method repeatedly to incrementally build up a
    /// `Rope`.  The passed text chunk can be as large or small as
    /// desired, but larger chunks are more efficient.
    ///
    /// `chunk` must be valid utf8 text.
    pub fn append(&mut self, chunk: &str) {
        let mut chunk = chunk;

        while !chunk.is_empty() {
            if self.buffer.is_empty() && chunk.len() >= MAX_TEXT_SIZE {
                // Process text data directly, skipping the buffer.
                let split_idx = crate::find_char_boundary_l(MAX_TEXT_SIZE, chunk.as_bytes());
                self.append_leaf_node(Node::Leaf(Arc::new(Text::from_str(&chunk[..split_idx]))));
                chunk = &chunk[split_idx..];
            }
            // Note: the `- 4` is to account for the variable-length utf8
            // encoding.  Without that, we could end up stuck in an infinite
            // loop where the buffer isn't considered full and thus won't be
            // processed, but there also isn't room at the end of the buffer to
            // fit the next code point from `chunk`.
            else if self.buffer.len() > (MAX_TEXT_SIZE - 4) {
                // Process filled buffer.
                self.append_leaf_node(Node::Leaf(Arc::new(Text::from_str(&self.buffer))));
                self.buffer.clear();
            } else {
                // Append to the buffer.
                let target_len = MAX_TEXT_SIZE - self.buffer.len();
                let split_idx = crate::find_char_boundary_l(target_len, chunk.as_bytes());
                self.buffer.push_str(&chunk[..split_idx]);
                chunk = &chunk[split_idx..];
            }
        }
    }

    /// Finishes the build, and returns the `Rope`.
    ///
    /// Note: this method consumes the builder.  If you want to continue
    /// building other ropes with the same prefix, you can clone the builder
    /// before calling `finish()`.
    pub fn finish(mut self) -> Rope {
        // Append the last leaf.
        if !self.buffer.is_empty() {
            self.append_leaf_node(Node::Leaf(Arc::new(Text::from_str(&self.buffer))));
            self.buffer.clear();
        }

        // Special case for empty rope.
        if self.stack.is_empty() {
            return Rope::new();
        }

        // Zip up all the remaining nodes on the stack
        let mut stack_idx = self.stack.len() - 1;
        while stack_idx >= 1 {
            let node = self.stack.pop().unwrap();
            self.stack[stack_idx - 1]
                .children_mut()
                .push((node.text_info(), node));
            stack_idx -= 1;
        }

        // Create the rope.
        let root = {
            let mut node = self.stack.pop().unwrap();
            compute_and_set_unbalance_flags_deep(&mut node);
            node
        };
        let root_info = root.text_info();
        Rope {
            root: root,
            root_info: root_info,
            owned_slice_byte_range: [0, root_info.bytes],
        }
    }

    //-----------------------------------------------------------------

    /// Builds a rope all at once from a single string slice.
    ///
    /// This avoids the creation and use of the internal buffer.  This is for
    /// internal use only, because the public-facing API has Rope::from_str(),
    /// which is equivalent and uses this for its implementation.
    pub(crate) fn build_at_once(mut self, text: &str) -> Rope {
        let mut text = text;

        while !text.is_empty() {
            let split_idx = crate::find_char_boundary_l(MAX_TEXT_SIZE, text.as_bytes());
            self.append_leaf_node(Node::Leaf(Arc::new(Text::from_str(&text[..split_idx]))));
            text = &text[split_idx..];
        }

        self.finish()
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!). DO NOT use
    /// this outside of Ropey's code base. If you do, anything that happens is
    /// your own fault, and your issue reports will be ignored.
    ///
    /// Directly appends `contents` to the in-progress rope as a single leaf
    /// node (chunk).  This is useful for building ropes with specific chunk
    /// configurations for testing purposes.  It is exposed publicly only for
    /// use in Ropey's own test suite.
    ///
    /// This makes no attempt to be consistent with the standard `append()`
    /// method, and should not be used in conjunction with it.
    #[doc(hidden)]
    pub fn _append_chunk_as_leaf(&mut self, contents: &str) {
        self.append_leaf_node(Node::Leaf(Arc::new(Text::from_str(contents))));
    }

    fn append_leaf_node(&mut self, leaf: Node) {
        // This will fill up the tree nicely, without packing it so tight that
        // initial edits will be inefficient.
        const TARGET_CHILDREN: usize = (MIN_CHILDREN + MAX_CHILDREN) / 2;

        if self.stack.is_empty() {
            self.stack.push(leaf);
            return;
        }

        if self.stack.last().unwrap().is_leaf() {
            let last = self.stack.pop().unwrap();

            let mut children = Children::new();
            children.push((last.text_info(), last));
            children.push((leaf.text_info(), leaf));

            self.stack.push(Node::Internal(Arc::new(children)));
            return;
        }

        let mut right = leaf;
        let mut stack_idx = (self.stack.len() - 1) as isize;
        loop {
            if stack_idx < 0 {
                // We're above the root, so do a root split.
                let mut children = Children::new();
                children.push((right.text_info(), right));
                self.stack.insert(0, Node::Internal(Arc::new(children)));
                break;
            } else if self.stack[stack_idx as usize].child_count() < TARGET_CHILDREN {
                // There's room to add a child, so do that.
                self.stack[stack_idx as usize]
                    .children_mut()
                    .push((right.text_info(), right));
                break;
            } else {
                // We've reached the target child count at this level of the
                // stack, so swap out the current node at this level with a
                // fresh new node, and hold on to the reached-target-child-count
                // node to be added as a child of the next level up.
                right = {
                    let mut children = Children::new();
                    children.push((right.text_info(), right));
                    Node::Internal(Arc::new(children))
                };
                std::mem::swap(&mut right, &mut self.stack[stack_idx as usize]);
                stack_idx -= 1;
            }
        }
    }
}

impl Default for RopeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_and_set_unbalance_flags_deep(node: &mut Node) {
    match *node {
        Node::Leaf(_) => {}
        Node::Internal(ref mut children) => {
            let children = Arc::make_mut(children);
            for i in 0..children.len() {
                compute_and_set_unbalance_flags_deep(&mut children.nodes_mut()[i]);
                children.update_unbalance_flag(i);
            }
        }
    }
}

//===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 127 bytes, 103 chars, 4 lines
    const TEXT: &str = "Hello there!  How're you doing?\r\nIt's \
                        a fine day, isn't it?\r\nAren't you glad \
                        we're alive?\r\nこんにちは、みんなさん！";

    #[test]
    fn rope_builder_01() {
        let mut b = RopeBuilder::new();

        b.append("Hello there!  How're you doing?\r");
        b.append("\nIt's a fine ");
        b.append("d");
        b.append("a");
        b.append("y,");
        b.append(" ");
        b.append("isn't it?");
        b.append("\r");
        b.append("\nAren't you ");
        b.append("glad we're alive?\r");
        b.append("\n");
        b.append("こんにち");
        b.append("は、みんなさ");
        b.append("ん！");

        let r = b.finish();

        assert_eq!(r, TEXT);
        r.assert_invariants();
    }

    #[test]
    fn rope_builder_default_01() {
        let mut b = RopeBuilder::default();

        b.append("Hello there!  How're you doing?\r");
        b.append("\nIt's a fine day, isn't it?\r\nAren't you ");
        b.append("glad we're alive?\r\nこんにちは、みんなさん！");

        let r = b.finish();

        assert_eq!(r, TEXT);
        r.assert_invariants();
    }
}
