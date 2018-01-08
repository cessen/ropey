#![doc(hidden)]

use std;
use std::sync::Arc;

use smallvec::SmallVec;

use rope::Rope;
use segmenter::{DefaultSegmenter, MainSegmenter, Segmenter};
use tree::{Node, NodeChildren, NodeText, MAX_BYTES, MAX_CHILDREN};

/// An efficient incremental `Rope` builder.
///
/// This is used to efficiently build ropes from sequences of text
/// chunks.  It is useful for creating ropes from:
///
/// - ...large text files, without pre-loading their entire contents into
///   memory (but see `Rope::from_reader()` which uses this internally
///   for precisely that use-case).
/// - ...streaming data sources.
/// - ...non-utf8 text data, doing the encoding conversion incrementally
///   as you go.
///
/// Unlike repeatedly calling `Rope::insert()` on the end of a rope,
/// this API runs in time linear to the amount of data fed to it, and
/// is overall much faster.
///
/// The converse of this API is the [`Chunks`](iter/struct.Chunks.html)
/// iterator, which is useful for efficiently streaming a rope's text
/// data _out_.
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
pub struct RopeBuilder<S = DefaultSegmenter>
where
    S: Segmenter,
{
    stack: SmallVec<[Arc<Node<S>>; 4]>,
    buffer: String,
}

impl RopeBuilder<DefaultSegmenter> {
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
}

impl<S: Segmenter> RopeBuilder<S> {
    /// Creates a new RopeBuilder with a custom segmenter.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::RopeBuilder;
    /// use ropey::segmenter::NullSegmenter;
    ///
    /// let mut builder = RopeBuilder::<NullSegmenter>::with_segmenter();
    /// ```
    pub fn with_segmenter() -> Self {
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
    pub fn finish(mut self) -> Rope<S> {
        // Append the last leaf
        self.append_internal("", true);
        self.finish_internal()
    }

    /// Builds a rope all at once from a single string slice.
    ///
    /// This avoids the creation and use of the internal buffer.  This is
    /// for internal use only, because the public-facing API has
    /// Rope::from_str(), which actually uses this for its implementation.
    pub(crate) fn build_at_once(mut self, chunk: &str) -> Rope<S> {
        self.append_internal(chunk, true);
        self.finish_internal()
    }

    //-----------------------------------------------------------------

    // Internal workings of `append()`.
    fn append_internal(&mut self, chunk: &str, last_chunk: bool) {
        let mut chunk = chunk;

        // Repeatedly chop text off the end of the input, creating
        // leaf nodes out of them and appending them to the tree.
        while !chunk.is_empty() || (!self.buffer.is_empty() && last_chunk) {
            // Get the text for the next leaf
            let (leaf_text, remainder) = self.get_next_leaf_text(chunk, last_chunk);
            chunk = remainder;

            // Append the leaf to the rope
            match leaf_text {
                NextText::None => break,
                NextText::UseBuffer => {
                    let string = NodeText::from_str(&self.buffer);
                    self.append_leaf_node(Arc::new(Node::Leaf(string)));
                    self.buffer.clear();
                }
                NextText::String(s) => {
                    self.append_leaf_node(Arc::new(Node::Leaf(NodeText::from_str(s))));
                }
            }
        }
    }

    // Internal workings of `finish()`.
    fn finish_internal(mut self) -> Rope<S> {
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
    fn get_next_leaf_text<'a>(
        &mut self,
        text: &'a str,
        last_chunk: bool,
    ) -> (NextText<'a>, &'a str) {
        if self.buffer.is_empty() {
            if text.len() > MAX_BYTES {
                // Simplest case: just chop off the end of `text`
                let split_idx = MainSegmenter::<S>::find_good_split(MAX_BYTES, text, true);
                if (split_idx == 0 || split_idx == text.len()) && !last_chunk {
                    self.buffer.push_str(text);
                    return (NextText::None, "");
                } else {
                    return (NextText::String(&text[..split_idx]), &text[split_idx..]);
                }
            } else if !last_chunk {
                self.buffer.push_str(text);
                return (NextText::None, "");
            } else {
                return (NextText::String(text), "");
            }
        } else if (text.len() + self.buffer.len()) > MAX_BYTES {
            let split_idx = if self.buffer.len() < MAX_BYTES {
                MainSegmenter::<S>::nearest_internal_break(MAX_BYTES - self.buffer.len(), text)
            } else {
                MainSegmenter::<S>::nearest_internal_break(0, text)
            };

            if (split_idx == 0 || split_idx == text.len()) && !last_chunk {
                self.buffer.push_str(text);
                return (NextText::None, "");
            } else {
                self.buffer.push_str(&text[..split_idx]);
                return (NextText::UseBuffer, &text[split_idx..]);
            }
        } else if !last_chunk {
            self.buffer.push_str(text);
            return (NextText::None, "");
        } else {
            self.buffer.push_str(text);
            return (NextText::UseBuffer, "");
        }
    }

    fn append_leaf_node(&mut self, leaf: Arc<Node<S>>) {
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
                            .children()
                            .push((left.text_info(), left));
                        break;
                    } else {
                        // Not enough room to fit a child, so split.
                        left = Arc::new(Node::Internal(
                            Arc::make_mut(&mut self.stack[stack_idx as usize])
                                .children()
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
