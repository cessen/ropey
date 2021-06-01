use sp_std::sync::Arc;

use smallvec::alloc::string::String;
use smallvec::SmallVec;

use crate::crlf;
use crate::rope::Rope;
use crate::tree::{Node, NodeChildren, NodeText, MAX_BYTES, MAX_CHILDREN};

/// An efficient incremental `Rope` builder.
///
/// This is used to efficiently build ropes from sequences of text
/// chunks.  It is useful for creating ropes from:
///
/// - ...large text files, without pre-loading their entire contents into
///   memory (but see
///   [`Rope::from_reader()`](struct.Rope.html#method.from_reader) for a
///   convenience function that does this for utf8 text).
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
#[derive(Debug, Clone, Default)]
pub struct RopeBuilder {
    stack: SmallVec<[Arc<Node>; 4]>,
    buffer: String,
}

impl RopeBuilder {
    /// Creates a new RopeBuilder, ready for input.
    pub fn new() -> Self {
        RopeBuilder {
            stack: {
                let mut stack = SmallVec::new();
                stack.push(Arc::new(Node::new()));
                stack
            },
            buffer: String::new(),
        }
    }

    /// Appends `chunk` to the end of the in-progress `Rope`.
    ///
    /// This method is called repeatedly to incrementally build up a
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

    //-----------------------------------------------------------------

    // Internal workings of `append()`.
    fn append_internal(&mut self, chunk: &str, is_last_chunk: bool) {
        let mut chunk = chunk;

        // Repeatedly chop text off the end of the input, creating
        // leaf nodes out of them and appending them to the tree.
        while !chunk.is_empty() || (!self.buffer.is_empty() && is_last_chunk) {
            // Get the text for the next leaf
            let (leaf_text, remainder) = self.get_next_leaf_text(chunk, is_last_chunk);
            chunk = remainder;

            // Append the leaf to the rope
            match leaf_text {
                NextText::None => break,
                NextText::UseBuffer => {
                    let leaf_text = NodeText::from_str(&self.buffer);
                    self.append_leaf_node(Arc::new(Node::Leaf(leaf_text)));
                    self.buffer.clear();
                }
                NextText::String(s) => {
                    self.append_leaf_node(Arc::new(Node::Leaf(NodeText::from_str(s))));
                }
            }
        }
    }

    // Internal workings of `finish()`.
    fn finish_internal(mut self) -> Rope {
        // Zip up all the remaining nodes on the stack
        let mut stack_idx = self.stack.len() - 1;
        while stack_idx >= 1 {
            let node = self.stack.pop().unwrap();
            if let Node::Internal(ref mut children) = *Arc::make_mut(&mut self.stack[stack_idx - 1])
            {
                children.push((node.text_info(), node));
            } else {
                unreachable!();
            }
            stack_idx -= 1;
        }

        // Get root and fix any right-side nodes with too few children.
        let mut root = self.stack.pop().unwrap();
        Arc::make_mut(&mut root).zip_fix_right();

        // Create the rope, make sure it's well-formed, and return it.
        let mut rope = Rope { root: root };
        rope.pull_up_singular_nodes();
        return rope;
    }

    // Returns (next_leaf_text, remaining_text)
    #[inline(always)]
    fn get_next_leaf_text<'a>(
        &mut self,
        text: &'a str,
        is_last_chunk: bool,
    ) -> (NextText<'a>, &'a str) {
        assert!(
            self.buffer.len() < MAX_BYTES,
            "RopeBuilder: buffer is already full when receiving a chunk! \
             This should never happen!",
        );

        // Simplest case: empty buffer and enough in `text` for a full
        // chunk, so just chop a chunk off from `text` and use that.
        if self.buffer.is_empty() && text.len() >= MAX_BYTES {
            let split_idx = crlf::find_good_split(
                MAX_BYTES.min(text.len() - 1), // - 1 to avoid CRLF split.
                text.as_bytes(),
                true,
            );
            return (NextText::String(&text[..split_idx]), &text[split_idx..]);
        }
        // If the buffer + `text` is enough for a full chunk, push enough
        // of `text` onto the buffer to fill it and use that.
        else if (text.len() + self.buffer.len()) >= MAX_BYTES {
            let mut split_idx =
                crlf::find_good_split(MAX_BYTES - self.buffer.len(), text.as_bytes(), true);
            if split_idx == text.len() && text.as_bytes()[text.len() - 1] == 0x0D {
                // Avoid CRLF split.
                split_idx -= 1;
            };
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

    fn append_leaf_node(&mut self, leaf: Arc<Node>) {
        let last = self.stack.pop().unwrap();
        match *last {
            Node::Leaf(_) => {
                if last.leaf_text().is_empty() {
                    self.stack.push(leaf);
                } else {
                    let mut children = NodeChildren::new();
                    children.push((last.text_info(), last));
                    children.push((leaf.text_info(), leaf));
                    self.stack.push(Arc::new(Node::Internal(children)));
                }
            }

            Node::Internal(_) => {
                self.stack.push(last);
                let mut left = leaf;
                let mut stack_idx = (self.stack.len() - 1) as isize;
                loop {
                    if stack_idx < 0 {
                        // We're above the root, so do a root split.
                        let mut children = NodeChildren::new();
                        children.push((left.text_info(), left));
                        self.stack.insert(0, Arc::new(Node::Internal(children)));
                        break;
                    } else if self.stack[stack_idx as usize].child_count() < (MAX_CHILDREN - 1) {
                        // There's room to add a child, so do that.
                        Arc::make_mut(&mut self.stack[stack_idx as usize])
                            .children_mut()
                            .push((left.text_info(), left));
                        break;
                    } else {
                        // Not enough room to fit a child, so split.
                        left = Arc::new(Node::Internal(
                            Arc::make_mut(&mut self.stack[stack_idx as usize])
                                .children_mut()
                                .push_split((left.text_info(), left)),
                        ));
                        sp_std::mem::swap(&mut left, &mut self.stack[stack_idx as usize]);
                        stack_idx -= 1;
                    }
                }
            }
        }
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

        r.assert_integrity();
        r.assert_invariants();
    }
}
