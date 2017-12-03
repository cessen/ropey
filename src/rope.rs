#![allow(dead_code)]

use std;
use std::sync::Arc;

use arrayvec::ArrayVec;

use smallvec::Array;
use small_string::SmallString;
use small_string_utils::{insert_at_char, split_string_near_byte};


// Internal node min/max values.
const MAX_CHILDREN: usize = 16;
const MIN_CHILDREN: usize = MAX_CHILDREN / 2;

// Leaf node min/max values.
const MAX_BYTES: usize = 384;
const MIN_BYTES: usize = MAX_BYTES / 2;

#[derive(Copy, Clone)]
pub(crate) struct BackingArray([u8; MAX_BYTES]);
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
pub(crate) type Count = u32;

//-------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Rope {
    pub(crate) root: Arc<Node>,
}

impl Rope {
    pub fn new() -> Rope {
        use std::mem::size_of;
        println!("Node size: {:?}", size_of::<Node>());
        Rope { root: Arc::new(Node::new()) }
    }

    pub fn len(&self) -> usize {
        self.root.text_info().bytes as usize
    }

    pub fn char_count(&self) -> Count {
        self.root.text_info().chars
    }

    pub fn to_string(&self) -> String {
        use iter::RopeChunkIter;
        let mut text = String::new();
        for chunk in RopeChunkIter::new(self) {
            text.push_str(chunk);
        }
        text
    }

    pub fn insert(&mut self, char_pos: Count, text: &str) {
        let root = Arc::make_mut(&mut self.root);
        if let Some(r_node) = root.insert(char_pos, text) {
            let mut info = ArrayVec::new();
            info.push(root.text_info());
            info.push(r_node.text_info());

            let mut children = ArrayVec::new();
            let mut l_node = Node::Empty;
            std::mem::swap(&mut l_node, root);
            children.push(Arc::new(l_node));
            children.push(Arc::new(r_node));

            *root = Node::Internal {
                info: info,
                children: children,
            };
        }
    }
}

//-------------------------------------------------------------

#[derive(Debug, Copy, Clone)]
pub(crate) struct TextInfo {
    pub(crate) bytes: Count,
    pub(crate) chars: Count,
    pub(crate) graphemes: Count,
    pub(crate) newlines: Count,
}

impl TextInfo {
    fn new() -> TextInfo {
        TextInfo {
            bytes: 0,
            chars: 0,
            graphemes: 0,
            newlines: 0,
        }
    }

    fn from_str(text: &str) -> TextInfo {
        TextInfo {
            bytes: text.len() as Count,
            chars: text.chars().count() as Count,
            graphemes: 0, // TODO
            newlines: 0, // TODO
        }
    }

    fn combine(&self, other: &TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes + other.bytes,
            chars: self.chars + other.chars,
            graphemes: self.graphemes + other.graphemes,
            newlines: self.newlines + other.newlines,
        }
    }
}

//-------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Empty,
    Leaf(SmallString<BackingArray>),
    Internal {
        info: ArrayVec<[TextInfo; MAX_CHILDREN]>,
        children: ArrayVec<[Arc<Node>; MAX_CHILDREN]>,
    },
}

impl Node {
    fn new() -> Node {
        Node::Empty
    }

    fn text_info(&self) -> TextInfo {
        match self {
            &Node::Empty => TextInfo::new(),
            &Node::Leaf(ref text) => TextInfo::from_str(text),
            &Node::Internal { ref info, .. } => {
                info.iter().fold(TextInfo::new(), |a, b| a.combine(b))
            }
        }
    }

    /// Inserts the text at the given char index.
    ///
    /// Returns a right-side residual node if the insertion wouldn't fit
    /// within this node.
    ///
    /// TODO: handle the situation where what's being inserted is larger
    /// than MAX_BYTES.
    fn insert(&mut self, char_pos: Count, text: &str) -> Option<Node> {
        match self {
            // If it's empty, turn it into a leaf
            &mut Node::Empty => {
                *self = Node::Leaf(text.into());
                return None;
            }

            // If it's a leaf
            &mut Node::Leaf(ref mut cur_text) => {
                insert_at_char(cur_text, text, char_pos as usize);

                if cur_text.len() <= MAX_BYTES {
                    return None;
                } else {
                    let split_pos = cur_text.len() - (cur_text.len() / 2);
                    let right_text = split_string_near_byte(cur_text, split_pos);
                    if right_text.len() > 0 {
                        cur_text.shrink_to_fit();
                        return Some(Node::Leaf(right_text));
                    } else {
                        // Leaf couldn't be validly split, so leave it oversized
                        return None;
                    }
                }
            }

            // If it's internal, things get a little more complicated
            &mut Node::Internal {
                ref mut info,
                ref mut children,
            } => {
                // Find the child to traverse into along with its starting char
                // offset.
                let (child_i, start_char) = {
                    let mut child_i = 0;
                    let mut start_char = 0;
                    for &TextInfo { chars: c, .. } in info.iter() {
                        let tmp = start_char + c;
                        if char_pos <= tmp {
                            break;
                        }

                        start_char = tmp;
                        child_i += 1;
                    }
                    (child_i.min(children.len() - 1), start_char)
                };

                // Navigate into the appropriate child
                let residual =
                    Arc::make_mut(&mut children[child_i]).insert(char_pos - start_char, text);
                info[child_i] = children[child_i].text_info();

                // Handle the new node, if any.
                if let Some(r_node) = residual {
                    // The new node will fit as a child of this node
                    if children.len() < MAX_CHILDREN {
                        info.insert(child_i + 1, r_node.text_info());
                        children.insert(child_i + 1, Arc::new(r_node));
                        return None;
                    }
                    // The new node won't fit!  Must split.
                    else {
                        let extra_info = info.try_insert(child_i + 1, r_node.text_info())
                            .err()
                            .unwrap()
                            .element();
                        let extra_child = children
                            .try_insert(child_i + 1, Arc::new(r_node))
                            .err()
                            .unwrap()
                            .element();

                        let mut r_info = ArrayVec::new();
                        let mut r_children = ArrayVec::new();

                        let r_count = (children.len() + 1) / 2;
                        let l_count = (children.len() + 1) - r_count;

                        for _ in l_count..children.len() {
                            r_info.push(info.remove(l_count));
                            r_children.push(children.remove(l_count));
                        }
                        r_info.push(extra_info);
                        r_children.push(extra_child);

                        return Some(Node::Internal {
                            info: r_info,
                            children: r_children,
                        });
                    }
                } else {
                    // No new node.  Easy.
                    return None;
                }
            }
        }
    }
}
