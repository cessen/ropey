use std::sync::Arc;

use crate::rope::Rope;
use crate::tree::{Children, Node, Text, MAX_CHILDREN, MAX_TEXT_SIZE};

/// An efficient incremental `Rope` builder.
///
/// This is used to efficiently build ropes from sequences of text
/// chunks.  It is useful for creating ropes from:
///
/// - ...large text files, without pre-loading their entire contents into
///   memory.
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
    stack: Vec<Node>,
    buffer: String,
    last_chunk_len_bytes: usize,
}

impl RopeBuilder {
    /// Creates a new RopeBuilder, ready for input.
    pub fn new() -> Self {
        RopeBuilder {
            stack: Vec::new(),
            buffer: String::new(),
            last_chunk_len_bytes: 0,
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
        self.append_internal(chunk, false);
    }

    /// Finishes the build, and returns the `Rope`.
    ///
    /// Note: this method consumes the builder.  If you want to continue
    /// building other ropes with the same prefix, you can clone the builder
    /// before calling `finish()`.
    pub fn finish(mut self) -> Rope {
        // Append the last leaf
        self.append_internal("", true);
        self.finish_internal()
    }

    /// Builds a rope all at once from a single string slice.
    ///
    /// This avoids the creation and use of the internal buffer.  This is
    /// for internal use only, because the public-facing API has
    /// Rope::from_str(), which actually uses this for its implementation.
    pub(crate) fn build_at_once(mut self, chunk: &str) -> Rope {
        self.append_internal(chunk, true);
        self.finish_internal()
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!).
    ///
    /// Appends `contents` to the in-progress rope as a single leaf
    /// node (chunk).  This is useful for building ropes with specific
    /// chunk configurations for testing purposes.  It will happily append
    /// both empty and more-than-max-size chunks.
    ///
    /// This makes no attempt to be consistent with the standard `append()`
    /// method, and should not be used in conjunction with it.
    #[doc(hidden)]
    pub fn _append_chunk(&mut self, contents: &str) {
        self.append_leaf_node(Node::Leaf(Arc::new(Text::from_str(contents))));
    }

    //-----------------------------------------------------------------

    // Internal workings of `append()`.
    fn append_internal(&mut self, chunk: &str, is_last_chunk: bool) {
        let mut chunk = chunk;

        // Repeatedly chop text off the end of the input, creating
        // leaf nodes out of them and appending them to the tree.
        while !chunk.is_empty() || (!self.buffer.is_empty() && is_last_chunk) {
            // Get the text for the next leaf.
            let (leaf_text, remainder) = self.get_next_leaf_text(chunk, is_last_chunk);
            chunk = remainder;

            self.last_chunk_len_bytes = chunk.len();

            // Append the leaf to the rope.
            match leaf_text {
                NextText::None => break,
                NextText::UseBuffer => {
                    let leaf_text = Text::from_str(&self.buffer);
                    self.append_leaf_node(Node::Leaf(Arc::new(leaf_text)));
                    self.buffer.clear();
                }
                NextText::String(s) => {
                    self.append_leaf_node(Node::Leaf(Arc::new(Text::from_str(s))));
                }
            }
        }
    }

    // Internal workings of `finish()`.
    //
    // When `fix_tree` is false, the resulting node tree is NOT fixed up
    // to adhere to the btree invariants.  This is useful for some testing
    // code.  But generally, `fix_tree` should be set to true.
    fn finish_internal(mut self) -> Rope {
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
        let root = self.stack.pop().unwrap();
        let root_info = root.text_info();
        Rope {
            root: root,
            root_info: root_info,
        }
    }

    // Returns (next_leaf_text, remaining_text)
    #[inline(always)]
    fn get_next_leaf_text<'a>(
        &mut self,
        text: &'a str,
        is_last_chunk: bool,
    ) -> (NextText<'a>, &'a str) {
        assert!(
            self.buffer.len() < MAX_TEXT_SIZE,
            "RopeBuilder: buffer is already full when receiving a chunk! \
             This should never happen!",
        );

        // Simplest case: empty buffer and enough in `text` for a full
        // chunk, so just chop a chunk off from `text` and use that.
        if self.buffer.is_empty() && text.len() >= MAX_TEXT_SIZE {
            let split_idx = crate::find_split(
                MAX_TEXT_SIZE.min(text.len() - 1), // - 1 to avoid CRLF split.
                text.as_bytes(),
            );
            return (NextText::String(&text[..split_idx]), &text[split_idx..]);
        }
        // If the buffer + `text` is enough for a full chunk, push enough
        // of `text` onto the buffer to fill it and use that.
        else if (text.len() + self.buffer.len()) >= MAX_TEXT_SIZE {
            let split_idx = crate::find_split(MAX_TEXT_SIZE - self.buffer.len(), text.as_bytes());
            self.buffer.push_str(&text[..split_idx]);
            return (NextText::UseBuffer, &text[split_idx..]);
        }
        // If we don't have enough text for a full chunk.
        else {
            // If it's our last chunk, wrap it all up!
            if is_last_chunk {
                if self.buffer.is_empty() {
                    return if text.is_empty() {
                        (NextText::None, "")
                    } else {
                        (NextText::String(text), "")
                    };
                } else {
                    self.buffer.push_str(text);
                    return (NextText::UseBuffer, "");
                }
            }
            // Otherwise, just push to the buffer.
            else {
                self.buffer.push_str(text);
                return (NextText::None, "");
            }
        }
    }

    fn append_leaf_node(&mut self, leaf: Node) {
        let last = if let Some(last) = self.stack.pop() {
            last
        } else {
            self.stack.push(leaf);
            return;
        };

        match last {
            Node::Leaf(_) => {
                let mut children = Children::new();
                children.push((last.text_info(), last));
                children.push((leaf.text_info(), leaf));
                self.stack.push(Node::Internal(Arc::new(children)));
            }

            Node::Internal(_) => {
                self.stack.push(last);
                let mut left = leaf;
                let mut stack_idx = (self.stack.len() - 1) as isize;
                loop {
                    if stack_idx < 0 {
                        // We're above the root, so do a root split.
                        let mut children = Children::new();
                        children.push((left.text_info(), left));
                        self.stack.insert(0, Node::Internal(Arc::new(children)));
                        break;
                    } else if self.stack[stack_idx as usize].child_count() < (MAX_CHILDREN - 1) {
                        // There's room to add a child, so do that.
                        self.stack[stack_idx as usize]
                            .children_mut()
                            .push((left.text_info(), left));
                        break;
                    } else {
                        // Not enough room to fit a child, so split.
                        left = Node::Internal(Arc::new(
                            self.stack[stack_idx as usize]
                                .children_mut()
                                .push_split((left.text_info(), left)),
                        ));
                        std::mem::swap(&mut left, &mut self.stack[stack_idx as usize]);
                        stack_idx -= 1;
                    }
                }
            }
        }
    }
}

impl Default for RopeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

enum NextText<'a> {
    None,
    UseBuffer,
    String(&'a str),
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

        dbg!();
        b.append("Hello there!  How're you doing?\r");
        dbg!();
        b.append("\nIt's a fine ");
        dbg!();
        b.append("d");
        dbg!();
        b.append("a");
        dbg!();
        b.append("y,");
        dbg!();
        b.append(" ");
        dbg!();
        b.append("isn't it?");
        dbg!();
        b.append("\r");
        dbg!();
        b.append("\nAren't you ");
        dbg!();
        b.append("glad we're alive?\r");
        dbg!();
        b.append("\n");
        dbg!();
        b.append("こんにち");
        dbg!();
        b.append("は、みんなさ");
        dbg!();
        b.append("ん！");
        dbg!();

        let r = b.finish();
        dbg!();

        assert_eq!(r, TEXT);
        dbg!();

        // r.assert_integrity();
        // r.assert_invariants();
    }

    #[test]
    fn rope_builder_default_01() {
        let mut b = RopeBuilder::default();

        b.append("Hello there!  How're you doing?\r");
        b.append("\nIt's a fine day, isn't it?\r\nAren't you ");
        b.append("glad we're alive?\r\nこんにちは、みんなさん！");

        let r = b.finish();

        assert_eq!(r, TEXT);

        // r.assert_integrity();
        // r.assert_invariants();
    }
}
