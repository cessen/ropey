#![allow(dead_code)]

use std;
use std::sync::Arc;

use arrayvec::ArrayVec;

use smallvec::Array;
use small_string::SmallString;
use small_string_utils::{insert_at_char, split_string_near_byte};


// Internal node min/max values.
const MAX_CHILDREN: usize = 32;
const MIN_CHILDREN: usize = MAX_CHILDREN / 2;

// Leaf node min/max values.
const MAX_BYTES: usize = 384;
const MIN_BYTES: usize = MAX_BYTES / 2;

#[derive(Copy, Clone)]
struct BackingArray([u8; MAX_BYTES]);
unsafe impl Array for BackingArray {
    type Item = u8;
    fn size() -> usize {
        MAX_BYTES
    }
    fn ptr(&self) -> *const u8 {
        &self.0[0]
    }
    fn ptr_mut(&mut self) -> *mut u8 {
        &mut self.0[0]
    }
}

// Type alias used for char count, grapheme count, line count, etc. storage
// in nodes.
type Count = u32;

//-------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Rope {
    root: Arc<Node>,
}

impl Rope {
    pub fn new() -> Rope {
        Rope { root: Arc::new(Node::new()) }
    }

    pub fn char_count(&self) -> Count {
        self.root.char_count()
    }

    pub fn insert(&mut self, char_pos: Count, text: &str) {
        let root = Arc::make_mut(&mut self.root);
        if let (char_count, Some(r_node)) = root.insert(char_pos, text) {
            let mut char_counts = ArrayVec::new();
            char_counts.push(char_count);
            char_counts.push(r_node.char_count());

            let mut children = ArrayVec::new();
            let mut l_node = Node::Empty;
            std::mem::swap(&mut l_node, root);
            children.push(Arc::new(l_node));
            children.push(Arc::new(r_node));

            *root = Node::Internal {
                char_counts: char_counts,
                children: children,
            };
        }
    }
}

//-------------------------------------------------------------

#[derive(Debug, Clone)]
enum Node {
    Empty,
    Leaf(SmallString<BackingArray>),
    Internal {
        char_counts: ArrayVec<[Count; MAX_CHILDREN]>,
        children: ArrayVec<[Arc<Node>; MAX_CHILDREN]>,
    },
}

impl Node {
    fn new() -> Node {
        Node::Empty
    }

    fn char_count(&self) -> Count {
        match self {
            &Node::Empty => 0,
            &Node::Leaf(ref text) => text.chars().count() as Count,
            &Node::Internal { ref char_counts, .. } => {
                char_counts.iter().fold(0, |a, b| a + b) as Count
            }
        }
    }

    /// Inserts the text at the given char index.
    ///
    /// Returns an updated char count for the node, and optionally a right-side
    /// residual node that overflowed the max char or node count.
    ///
    /// TODO: handle the situation where what's being inserted is larger
    /// than MAX_BYTES.
    fn insert(&mut self, char_pos: Count, text: &str) -> (Count, Option<Node>) {
        match self {
            // If it's empty, turn it into a leaf
            &mut Node::Empty => {
                *self = Node::Leaf(text.into());
                return (text.chars().count() as Count, None);
            }

            // If it's a leaf
            &mut Node::Leaf(ref mut cur_text) => {
                insert_at_char(cur_text, text, char_pos as usize);

                if cur_text.len() <= MAX_BYTES {
                    return (cur_text.chars().count() as Count, None);
                } else {
                    let split_pos = cur_text.len() / 2;
                    let right_text = split_string_near_byte(cur_text, split_pos);
                    cur_text.shrink_to_fit();

                    return (
                        cur_text.chars().count() as Count,
                        Some(Node::Leaf(right_text)),
                    );
                }
            }

            // If it's internal, things get a little more complicated
            &mut Node::Internal {
                ref mut char_counts,
                ref mut children,
            } => {
                // Find the child to traverse into along with its starting char
                // offset.
                let (child_i, start_char) = {
                    let mut child_i = 0;
                    let mut start_char = 0;
                    for c in char_counts.iter() {
                        let tmp = start_char + *c;
                        if char_pos <= tmp {
                            break;
                        }

                        start_char = tmp;
                        child_i += 1;
                    }
                    (child_i.min(children.len() - 1), start_char)
                };

                // Navigate into the appropriate child
                let (updated_char_count, residual) =
                    Arc::make_mut(&mut children[child_i]).insert(char_pos - start_char, text);
                char_counts[child_i] = updated_char_count;

                // Handle the new node, if any.
                if let Some(r_node) = residual {
                    // The new node will fit as a child of this node
                    if children.len() < MAX_CHILDREN {
                        char_counts.insert(child_i + 1, r_node.char_count());
                        children.insert(child_i + 1, Arc::new(r_node));
                        return (char_counts.iter().sum(), None);
                    }
                    // The new node won't fit!  Must split.
                    else {
                        let extra_count = char_counts
                            .try_insert(child_i + 1, r_node.char_count())
                            .err()
                            .unwrap()
                            .element();
                        let extra_child = children
                            .try_insert(child_i + 1, Arc::new(r_node))
                            .err()
                            .unwrap()
                            .element();

                        let mut r_char_counts = ArrayVec::new();
                        let mut r_children = ArrayVec::new();

                        let r_count = (children.len() + 1) / 2;
                        let l_count = (children.len() + 1) - r_count;

                        for _ in l_count..children.len() {
                            r_char_counts.push(char_counts.remove(l_count));
                            r_children.push(children.remove(l_count));
                        }
                        r_char_counts.push(extra_count);
                        r_children.push(extra_child);

                        return (
                            char_counts.iter().sum(),
                            Some(Node::Internal {
                                char_counts: r_char_counts,
                                children: r_children,
                            }),
                        );
                    }
                } else {
                    // No new node.  Easy.
                    return (char_counts.iter().sum(), None);
                }
            }
        }
    }
}
