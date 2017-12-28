use std;
use std::sync::Arc;
use smallvec::SmallVec;

use rope::Rope;
use str_utils::{is_grapheme_boundary, nearest_internal_grapheme_boundary, next_grapheme_boundary,
                prev_grapheme_boundary};
use tree::{Node, NodeChildren, NodeText, MAX_BYTES, MAX_CHILDREN};

/// An incremental `Rope` builder.
///
/// `RopeBuilder` is used to efficiently build `Rope`s from sequences
/// of text chunks.  It is useful for situations such as:
///
/// - Creating a rope from a large text file without pre-loading the
///   entire contents of the file into memory (but see
///   `Rope::from_reader()` which uses `RopeBuilder` internally for
///   precisely this use-case).
/// - Creating a rope from a streaming data source.
/// - Loading a non-utf8 text source into a rope, doing the encoding
///   conversion incrementally as you go.
///
/// Unlike repeatedly calling `Rope::insert()` on the end of a rope,
/// this API runs in time linear to the amount of data fed to it, and
/// is overall much faster.  It also creates more memory-compact ropes.
///
/// (The converse of this API is the [`Chunks`](iter/struct.Chunks.html)
/// iterator.)
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
    stack: SmallVec<[Arc<Node>; 4]>,
    buffer: String,
}

impl RopeBuilder {
    /// Creates a new RopeBuilder, ready for input.
    pub fn new() -> RopeBuilder {
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
    fn get_next_leaf_text<'a>(
        &mut self,
        text: &'a str,
        last_chunk: bool,
    ) -> (NextText<'a>, &'a str) {
        if self.buffer.is_empty() {
            if text.len() > MAX_BYTES {
                // Simplest case: just chop off the end of `text`
                let split_idx = find_good_split_idx(text, MAX_BYTES);
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
                nearest_internal_grapheme_boundary(text, MAX_BYTES - self.buffer.len())
            } else {
                nearest_internal_grapheme_boundary(text, 0)
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

fn find_good_split_idx(text: &str, idx: usize) -> usize {
    if is_grapheme_boundary(text, idx) {
        idx
    } else {
        let prev = prev_grapheme_boundary(text, idx);
        let next = next_grapheme_boundary(text, idx);
        if prev > 0 {
            prev
        } else {
            next
        }
    }
}
