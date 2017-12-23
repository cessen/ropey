use std;
use std::sync::Arc;
use std::collections::VecDeque;

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
///   entire contents of the file into memory first (but see
///   `Rope::from_reader()` which uses `RopeBuilder` internally for
///   precisely this use-case).
/// - Creating a rope from a streaming data source.
/// - Loading a non-utf8 text source into a rope, doing the encoding
///   conversion incrementally as you go.
///
/// Unlike repeatedly calling `Rope::insert()` on the end of a rope,
/// this API runs in time linear to the amount of data fed to it.  It
/// also creates more memory-compact ropes.
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
    stack: VecDeque<Node>,
    buffer: String,
}

impl RopeBuilder {
    /// Creates a new RopeBuilder, ready for input.
    pub fn new() -> RopeBuilder {
        RopeBuilder {
            stack: {
                let mut stack = VecDeque::with_capacity(8);
                stack.push_back(Node::new());
                stack
            },
            buffer: String::with_capacity(MAX_BYTES),
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
        let mut chunk = chunk;

        // Repeatedly chop text off the end of the input, creating
        // leaf nodes out of them and appending them to the tree.
        while !chunk.is_empty() {
            // Get the text for the next leaf
            let (leaf_text, remainder) = self.get_next_leaf_text(chunk);
            chunk = remainder;

            // Append the leaf to the rope
            match leaf_text {
                NextText::None => break,
                NextText::UseBuffer => {
                    let string = NodeText::from_str(&self.buffer);
                    self.append_leaf_node(Node::Leaf(string));
                    self.buffer.clear();
                }
                NextText::String(s) => {
                    self.append_leaf_node(Node::Leaf(NodeText::from_str(s)));
                }
            }
        }
    }

    /// Finishes the build, and returns the `Rope`.
    ///
    /// Note: this consumes the builder.  If you want to continue building
    /// other ropes with the same prefix, you can clone the builder before
    /// calling `finish()`.
    pub fn finish(mut self) -> Rope {
        // Append the last leaf
        if !self.buffer.is_empty() {
            let string = NodeText::from_str(&self.buffer);
            self.append_leaf_node(Node::Leaf(string));
        }

        // Zip up all the remaining nodes on the stack
        let mut stack_idx = self.stack.len() - 1;
        while stack_idx >= 1 {
            let node = self.stack.pop_back().unwrap();
            if let Node::Internal(ref mut children) = self.stack[stack_idx - 1] {
                children.push((node.text_info(), Arc::new(node)));
            } else {
                unreachable!();
            }
            stack_idx -= 1;
        }

        // Get root and fix any right-side nodes with too few children.
        let mut root = self.stack.pop_back().unwrap();
        root.zip_fix_right();

        // Crate the rope, make sure it's well-formed, and return it.
        let mut rope = Rope {
            root: Arc::new(root),
        };
        rope.pull_up_singular_nodes();
        return rope;
    }

    //-----------------------------------------------------------------

    // Returns (next_leaf_text, remaining_text)
    fn get_next_leaf_text<'a>(&mut self, text: &'a str) -> (NextText<'a>, &'a str) {
        if self.buffer.is_empty() {
            if text.len() > MAX_BYTES {
                // Simplest case: just chop off the end of `text`
                let split_idx = find_good_split_idx(text, MAX_BYTES);
                if split_idx == 0 || split_idx == text.len() {
                    self.buffer.push_str(text);
                    return (NextText::None, "");
                } else {
                    return (NextText::String(&text[..split_idx]), &text[split_idx..]);
                }
            } else {
                self.buffer.push_str(text);
                return (NextText::None, "");
            }
        } else if (text.len() + self.buffer.len()) > MAX_BYTES {
            let split_idx = if self.buffer.len() < MAX_BYTES {
                nearest_internal_grapheme_boundary(text, MAX_BYTES - self.buffer.len())
            } else {
                nearest_internal_grapheme_boundary(text, 0)
            };

            if split_idx == 0 || split_idx == text.len() {
                self.buffer.push_str(text);
                return (NextText::None, "");
            } else {
                self.buffer.push_str(&text[..split_idx]);
                return (NextText::UseBuffer, &text[split_idx..]);
            }
        } else {
            self.buffer.push_str(text);
            return (NextText::None, "");
        }
    }

    fn append_leaf_node(&mut self, leaf: Node) {
        let last = self.stack.pop_back().unwrap();
        match last {
            Node::Leaf(_) => {
                if last.leaf_text().is_empty() {
                    self.stack.push_back(leaf);
                } else {
                    let mut children = NodeChildren::new();
                    children.push((last.text_info(), Arc::new(last)));
                    children.push((leaf.text_info(), Arc::new(leaf)));
                    self.stack.push_back(Node::Internal(children));
                }
            }

            Node::Internal(_) => {
                self.stack.push_back(last);
                let mut left = leaf;
                let mut stack_idx = (self.stack.len() - 1) as isize;
                loop {
                    if stack_idx < 0 {
                        // We're above the root, so do a root split.
                        let mut children = NodeChildren::new();
                        children.push((left.text_info(), Arc::new(left)));
                        self.stack.push_front(Node::Internal(children));
                        break;
                    } else if self.stack[stack_idx as usize].child_count() < (MAX_CHILDREN - 1) {
                        // There's room to add a child, so do that.
                        self.stack[stack_idx as usize]
                            .children()
                            .push((left.text_info(), Arc::new(left)));
                        break;
                    } else {
                        // Not enough room to fit a child, so split.
                        left = Node::Internal(
                            self.stack[stack_idx as usize]
                                .children()
                                .push_split((left.text_info(), Arc::new(left))),
                        );
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
