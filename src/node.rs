#![allow(dead_code)]

use std;
use std::sync::Arc;
use std::collections::VecDeque;

use arrayvec::ArrayVec;
use smallvec::Array;

use slice::RopeSlice;
use small_string::SmallString;
use small_string_utils::{byte_idx_to_char_idx, byte_idx_to_line_idx, char_idx_to_byte_idx,
                         char_idx_to_line_idx, line_idx_to_byte_idx, line_idx_to_char_idx,
                         split_string_near_byte, nearest_grapheme_boundary, fix_grapheme_seam};
use text_info::{TextInfo, TextInfoArray, Count};


// Internal node min/max values.
const MAX_CHILDREN: usize = 16;
const MIN_CHILDREN: usize = MAX_CHILDREN / 2;

// Leaf node min/max values.
const MAX_BYTES: usize = 334;
const MIN_BYTES: usize = MAX_BYTES / 2;


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
    pub(crate) fn new() -> Node {
        Node::Empty
    }

    pub(crate) fn from_str(text: &str) -> Node {
        const CHUNK_SIZE: usize = (MAX_BYTES * 9) / 10;

        // We keep a stack of the right-most nodes
        // down the edge of the rope tree.  This allows
        // us to process everything without recursion.
        // Not actually sure if that's a gain or not,
        // but it works!
        let mut stack = VecDeque::with_capacity(32);
        stack.push_back(Node::Empty);

        // Loop over the text, splitting bits off the left and
        // appending them to the rope as we go.
        let mut text = text;
        while text.len() > 0 {
            // Calculate split point
            let split_idx = if text.len() > (CHUNK_SIZE * 2) {
                nearest_grapheme_boundary(text, CHUNK_SIZE)
            } else if text.len() < MAX_BYTES {
                text.len()
            } else {
                let tmp = text.len() / 2;
                nearest_grapheme_boundary(text, tmp)
            };

            // Split text off of the left
            let leaf_text = &text[..split_idx];
            text = &text[split_idx..];

            // Append the text as a leaf node, balancing the tree
            // appropriately as we go.
            let last = stack.pop_back().unwrap();
            match last {
                Node::Empty => {
                    stack.push_back(Node::Leaf(SmallString::from_str(leaf_text)));
                }

                Node::Leaf(_) => {
                    let right = Node::Leaf(SmallString::from_str(leaf_text));

                    let mut info = ArrayVec::new();
                    let mut children = ArrayVec::new();
                    info.push(last.text_info());
                    info.push(right.text_info());
                    children.push(Arc::new(last));
                    children.push(Arc::new(right));

                    stack.push_back(Node::Internal {
                        info: info,
                        children: children,
                    });
                }

                Node::Internal {
                    mut info,
                    mut children,
                } => {
                    if children.len() < (MAX_CHILDREN - 1) {
                        let right = Node::Leaf(SmallString::from_str(leaf_text));
                        info.push(right.text_info());
                        children.push(Arc::new(right));
                        stack.push_back(Node::Internal {
                            info: info,
                            children: children,
                        });
                    } else {
                        let leaf = Node::Leaf(SmallString::from_str(leaf_text));
                        let r_info = push_split_arrayvec(&mut info, leaf.text_info());
                        let r_children = push_split_arrayvec(&mut children, Arc::new(leaf));
                        stack.push_back(Node::Internal {
                            info: r_info,
                            children: r_children,
                        });

                        let mut left = Node::Internal {
                            info: info,
                            children: children,
                        };
                        let mut stack_idx = stack.len() - 1;
                        loop {
                            if stack_idx >= 1 {
                                if stack[stack_idx - 1].child_count() < (MAX_CHILDREN - 1) {
                                    if let Node::Internal {
                                        ref mut info,
                                        ref mut children,
                                    } = stack[stack_idx - 1]
                                    {
                                        info.push(left.text_info());
                                        children.push(Arc::new(left));
                                        break;
                                    } else {
                                        unreachable!()
                                    }
                                } else {
                                    let (r_info, r_children) = if let Node::Internal {
                                        ref mut info,
                                        ref mut children,
                                    } = stack[stack_idx - 1]
                                    {
                                        let r_info = push_split_arrayvec(info, left.text_info());
                                        let r_children =
                                            push_split_arrayvec(children, Arc::new(left));
                                        (r_info, r_children)
                                    } else {
                                        unreachable!()
                                    };
                                    left = Node::Internal {
                                        info: r_info,
                                        children: r_children,
                                    };
                                    std::mem::swap(&mut stack[stack_idx - 1], &mut left);
                                    stack_idx -= 1;
                                }
                            } else {
                                let mut info = ArrayVec::new();
                                let mut children = ArrayVec::new();
                                info.push(left.text_info());
                                children.push(Arc::new(left));
                                stack.push_front(Node::Internal {
                                    info: info,
                                    children: children,
                                });
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Zip up all the remaining nodes on the stack
        let mut stack_idx = stack.len() - 1;
        while stack_idx >= 1 {
            let node = stack.pop_back().unwrap();
            if let Node::Internal {
                ref mut info,
                ref mut children,
            } = stack[stack_idx - 1]
            {
                info.push(node.text_info());
                children.push(Arc::new(node));
            } else {
                unreachable!();
            }
            stack_idx -= 1;
        }

        // Return the root.
        stack.pop_back().unwrap()
    }

    /// Total number of bytes in the Rope.
    pub(crate) fn byte_count(&self) -> usize {
        self.text_info().bytes as usize
    }

    /// Total number of chars in the Rope.
    pub(crate) fn char_count(&self) -> usize {
        self.text_info().chars as usize
    }

    /// Total number of line breaks in the Rope.
    pub(crate) fn line_break_count(&self) -> usize {
        self.text_info().line_breaks as usize
    }

    /// Returns the char index of the given byte.
    pub(crate) fn byte_to_char(&self, byte_idx: usize) -> usize {
        match self {
            &Node::Empty => 0,
            &Node::Leaf(ref text) => byte_idx_to_char_idx(text, byte_idx),
            &Node::Internal {
                ref info,
                ref children,
            } => {
                let (child_i, acc_info) = info.search_combine(|inf| byte_idx as Count <= inf.bytes);

                // Shortcuts
                if byte_idx == 0 {
                    return 0;
                } else if byte_idx == acc_info.bytes as usize + info[child_i].bytes as usize {
                    return acc_info.chars as usize + info[child_i].chars as usize;
                }

                acc_info.chars as usize +
                    children[child_i].byte_to_char(byte_idx - acc_info.bytes as usize)
            }
        }
    }

    /// Returns the line index of the given byte.
    pub(crate) fn byte_to_line(&self, byte_idx: usize) -> usize {
        match self {
            &Node::Empty => 0,
            &Node::Leaf(ref text) => byte_idx_to_line_idx(text, byte_idx),
            &Node::Internal {
                ref info,
                ref children,
            } => {
                let (child_i, acc_info) = info.search_combine(|inf| byte_idx as Count <= inf.bytes);

                // Shortcuts
                if byte_idx == 0 {
                    return 0;
                } else if byte_idx ==
                           acc_info.line_breaks as usize + info[child_i].line_breaks as usize
                {
                    return acc_info.line_breaks as usize + info[child_i].line_breaks as usize;
                }

                acc_info.line_breaks as usize +
                    children[child_i].byte_to_line(byte_idx - acc_info.bytes as usize)
            }
        }
    }

    /// Returns the byte index of the given char.
    pub(crate) fn char_to_byte(&self, char_idx: usize) -> usize {
        match self {
            &Node::Empty => 0,
            &Node::Leaf(ref text) => char_idx_to_byte_idx(text, char_idx),
            &Node::Internal {
                ref info,
                ref children,
            } => {
                let (child_i, acc_info) = info.search_combine(|inf| char_idx as Count <= inf.chars);

                // Shortcuts
                if char_idx == 0 {
                    return 0;
                } else if char_idx == acc_info.chars as usize + info[child_i].chars as usize {
                    return acc_info.bytes as usize + info[child_i].bytes as usize;
                }

                acc_info.bytes as usize +
                    children[child_i].char_to_byte(char_idx - acc_info.chars as usize)
            }
        }
    }

    /// Returns the line index of the given char.
    pub(crate) fn char_to_line(&self, char_idx: usize) -> usize {
        match self {
            &Node::Empty => 0,
            &Node::Leaf(ref text) => char_idx_to_line_idx(text, char_idx),
            &Node::Internal {
                ref info,
                ref children,
            } => {
                let (child_i, acc_info) = info.search_combine(|inf| char_idx as Count <= inf.chars);

                // Shortcuts
                if char_idx == 0 {
                    return 0;
                } else if char_idx == acc_info.chars as usize + info[child_i].chars as usize {
                    return acc_info.line_breaks as usize + info[child_i].line_breaks as usize;
                }

                acc_info.line_breaks as usize +
                    children[child_i].char_to_line(char_idx - acc_info.chars as usize)
            }
        }
    }

    /// Returns the byte index of the start of the given line.
    pub(crate) fn line_to_byte(&self, line_idx: usize) -> usize {
        match self {
            &Node::Empty => 0,
            &Node::Leaf(ref text) => line_idx_to_byte_idx(text, line_idx),
            &Node::Internal {
                ref info,
                ref children,
            } => {
                let (child_i, acc_info) =
                    info.search_combine(|inf| line_idx as Count <= inf.line_breaks);

                acc_info.bytes as usize +
                    children[child_i].line_to_byte(line_idx - acc_info.line_breaks as usize)
            }
        }
    }

    /// Returns the char index of the start of the given line.
    pub(crate) fn line_to_char(&self, line_idx: usize) -> usize {
        match self {
            &Node::Empty => 0,
            &Node::Leaf(ref text) => line_idx_to_char_idx(text, line_idx),
            &Node::Internal {
                ref info,
                ref children,
            } => {
                let (child_i, acc_info) =
                    info.search_combine(|inf| line_idx as Count <= inf.line_breaks);

                acc_info.chars as usize +
                    children[child_i].line_to_char(line_idx - acc_info.line_breaks as usize)
            }
        }
    }

    /// Returns an immutable slice of the Rope in the char range `start..end`.
    pub(crate) fn slice<'a>(&'a self, start: usize, end: usize) -> RopeSlice<'a> {
        RopeSlice::new_with_range(self, start, end)
    }

    pub(crate) fn text_info(&self) -> TextInfo {
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
    /// within this node.  Also returns the byte position where there may
    /// be a grapheme seam to fix, if any.
    ///
    /// TODO: handle the situation where what's being inserted is larger
    /// than MAX_BYTES.
    pub(crate) fn insert(&mut self, char_pos: Count, text: &str) -> (Option<Node>, Option<Count>) {
        match self {
            // If it's empty, turn it into a leaf
            &mut Node::Empty => {
                *self = Node::Leaf(text.into());
                return (None, None);
            }

            // If it's a leaf
            &mut Node::Leaf(ref mut cur_text) => {
                let byte_pos = char_idx_to_byte_idx(cur_text, char_pos as usize);
                let seam = if byte_pos == 0 {
                    Some(0)
                } else if byte_pos == cur_text.len() {
                    let count = (cur_text.len() + text.len()) as Count;
                    Some(count)
                } else {
                    None
                };

                cur_text.insert_str(byte_pos, text);

                if cur_text.len() <= MAX_BYTES {
                    return (None, seam);
                } else {
                    let split_pos = cur_text.len() - (cur_text.len() / 2);
                    let right_text = split_string_near_byte(cur_text, split_pos);
                    if right_text.len() > 0 {
                        cur_text.shrink_to_fit();
                        return (Some(Node::Leaf(right_text)), seam);
                    } else {
                        // Leaf couldn't be validly split, so leave it oversized
                        return (None, seam);
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
                let (child_i, start_info) = info.search_combine(|inf| char_pos <= inf.chars);
                let start_char = start_info.chars;

                // Navigate into the appropriate child
                let (residual, child_seam) =
                    Arc::make_mut(&mut children[child_i]).insert(char_pos - start_char, text);
                info[child_i] = children[child_i].text_info();

                // Calculate the seam offset relative to this node
                let seam = child_seam.map(|byte_pos| byte_pos + start_info.bytes);

                // Handle the new node, if any.
                if let Some(r_node) = residual {
                    // The new node will fit as a child of this node
                    if children.len() < MAX_CHILDREN {
                        info.insert(child_i + 1, r_node.text_info());
                        children.insert(child_i + 1, Arc::new(r_node));
                        return (None, seam);
                    }
                    // The new node won't fit!  Must split.
                    else {
                        let r_info = insert_split_arrayvec(info, r_node.text_info(), child_i + 1);
                        let r_children =
                            insert_split_arrayvec(children, Arc::new(r_node), child_i + 1);

                        return (
                            Some(Node::Internal {
                                info: r_info,
                                children: r_children,
                            }),
                            seam,
                        );
                    }
                } else {
                    // No new node.  Easy.
                    return (None, seam);
                }
            }
        }
    }

    //-----------------------------------------

    fn child_count(&self) -> usize {
        if let &Node::Internal { ref children, .. } = self {
            children.len()
        } else {
            panic!()
        }
    }

    /// Debugging tool to make sure that all of the meta-data of the
    /// tree is consistent with the actual data.
    pub(crate) fn verify_integrity(&self) {
        match self {
            &Node::Empty => {}
            &Node::Leaf(_) => {}
            &Node::Internal {
                ref info,
                ref children,
            } => {
                assert_eq!(info.len(), children.len());
                for (inf, child) in info.iter().zip(children.iter()) {
                    assert_eq!(*inf, child.text_info());
                    child.verify_integrity();
                }
            }
        }
    }

    /// Checks to make sure that a boundary between leaf nodes (given as a byte
    /// position in the rope) doesn't split a grapheme, and fixes it if it does.
    ///
    /// Note: panics if the given byte position is not on the boundary between
    /// two leaf nodes.
    pub(crate) fn fix_grapheme_seam(
        &mut self,
        byte_pos: Count,
    ) -> Option<&mut SmallString<BackingArray>> {
        match self {
            &mut Node::Empty => return None,

            &mut Node::Leaf(ref mut text) => {
                if byte_pos == 0 || byte_pos == text.len() as Count {
                    Some(text)
                } else {
                    panic!("Byte position given is not on a leaf boundary.")
                }
            }

            &mut Node::Internal {
                ref mut info,
                ref mut children,
            } => {
                if byte_pos == 0 {
                    // Special-case 1
                    return Arc::make_mut(&mut children[0]).fix_grapheme_seam(byte_pos);
                } else if byte_pos == info.combine().bytes {
                    // Special-case 2
                    return Arc::make_mut(children.last_mut().unwrap())
                        .fix_grapheme_seam(info.last().unwrap().bytes);
                } else {
                    // Find the child to navigate into
                    let (child_i, start_info) = info.search_combine(|inf| byte_pos <= inf.bytes);
                    let start_byte = start_info.bytes;

                    let pos_in_child = byte_pos - start_byte;
                    let child_len = info[child_i].bytes;

                    if pos_in_child == 0 || pos_in_child == child_len {
                        // Left or right edge, get neighbor and fix seam
                        let ((split_l, split_r), child_l_i) = if pos_in_child == 0 {
                            (children.split_at_mut(child_i), child_i - 1)
                        } else {
                            (children.split_at_mut(child_i + 1), child_i)
                        };
                        let left_child = Arc::make_mut(split_l.last_mut().unwrap());
                        let right_child = Arc::make_mut(split_r.first_mut().unwrap());
                        fix_grapheme_seam(
                            left_child.fix_grapheme_seam(info[child_l_i].bytes).unwrap(),
                            right_child.fix_grapheme_seam(0).unwrap(),
                        );
                        left_child.fix_info_right();
                        right_child.fix_info_left();
                        info[child_l_i] = left_child.text_info();
                        info[child_l_i + 1] = right_child.text_info();
                        return None;
                    } else {
                        // Internal to child
                        return Arc::make_mut(&mut children[child_i]).fix_grapheme_seam(
                            pos_in_child,
                        );
                    }
                }
            }
        }
    }

    /// Updates the tree meta-data down the left side of the tree.
    fn fix_info_left(&mut self) {
        match self {
            &mut Node::Empty => {}
            &mut Node::Leaf(_) => {}
            &mut Node::Internal {
                ref mut info,
                ref mut children,
            } => {
                let left = Arc::make_mut(children.first_mut().unwrap());
                left.fix_info_left();
                *info.first_mut().unwrap() = left.text_info();
            }
        }
    }

    /// Updates the tree meta-data down the right side of the tree.
    fn fix_info_right(&mut self) {
        match self {
            &mut Node::Empty => {}
            &mut Node::Leaf(_) => {}
            &mut Node::Internal {
                ref mut info,
                ref mut children,
            } => {
                let right = Arc::make_mut(children.last_mut().unwrap());
                right.fix_info_right();
                *info.last_mut().unwrap() = right.text_info();
            }
        }
    }
}

//=======================================================

/// Pushes an element onto the end of an ArrayVec,
/// and then splits it in half, returning the right
/// half.
///
/// This works even when the given ArrayVec is full.
pub fn push_split_arrayvec<T>(
    v: &mut ArrayVec<[T; MAX_CHILDREN]>,
    new_child: T,
) -> ArrayVec<[T; MAX_CHILDREN]> {
    let mut right = ArrayVec::new();

    let r_count = (v.len() + 1) / 2;
    let l_count = (v.len() + 1) - r_count;

    for _ in l_count..v.len() {
        right.push(v.remove(l_count));
    }
    right.push(new_child);

    right
}

/// Inserts an element into an ArrayVec, and then splits
/// it in half, returning the right half.
///
/// This works even when the given ArrayVec is full.
pub fn insert_split_arrayvec<T>(
    v: &mut ArrayVec<[T; MAX_CHILDREN]>,
    new_child: T,
    idx: usize,
) -> ArrayVec<[T; MAX_CHILDREN]> {
    assert!(v.len() > 0);
    let extra = if idx < v.len() {
        let extra = v.pop().unwrap();
        v.insert(idx, new_child);
        extra
    } else {
        new_child
    };

    push_split_arrayvec(v, extra)
}

//=======================================================

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

//=======================================================

#[cfg(test)]
mod tests {
    use rope::Rope;

    const TEXT: &str = "\r\nHello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n";

    #[test]
    fn line_to_byte_01() {
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

        assert_eq!(3, r.root.line_break_count());
        assert_eq!(0, r.line_to_byte(0));
        assert_eq!(2, r.line_to_byte(1));
        assert_eq!(93, r.line_to_byte(2));
        assert_eq!(209, r.line_to_byte(3));
    }

    #[test]
    fn line_to_char_01() {
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

        assert_eq!(3, r.root.line_break_count());
        assert_eq!(0, r.line_to_char(0));
        assert_eq!(2, r.line_to_char(1));
        assert_eq!(93, r.line_to_char(2));
        assert_eq!(133, r.line_to_char(3));
    }
}
