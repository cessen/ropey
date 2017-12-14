#![allow(dead_code)]

use std::sync::Arc;

use child_array::ChildArray;
use smallvec::Array;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use slice::RopeSlice;
use small_string::SmallString;
use str_utils::{byte_idx_to_char_idx, byte_idx_to_line_idx, char_idx_to_byte_idx,
                char_idx_to_line_idx, line_idx_to_byte_idx, line_idx_to_char_idx,
                is_grapheme_boundary, prev_grapheme_boundary, next_grapheme_boundary,
                nearest_internal_grapheme_boundary, seam_is_grapheme_boundary};
use text_info::{TextInfo, Count};


// Internal node min/max values.
pub(crate) const MAX_CHILDREN: usize = 3;
const MIN_CHILDREN: usize = MAX_CHILDREN - (MAX_CHILDREN / 2);

// Leaf node min/max values.
pub(crate) const MAX_BYTES: usize = 2;
const MIN_BYTES: usize = MAX_BYTES - (MAX_BYTES / 2);


#[derive(Debug, Clone)]
pub(crate) enum Node {
    Leaf(SmallString<BackingArray>),
    Internal(ChildArray),
}

impl Node {
    /// Creates an empty node.
    pub(crate) fn new() -> Node {
        Node::Leaf(SmallString::from_str(""))
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
                    let split_pos = {
                        let pos = cur_text.len() - (cur_text.len() / 2);
                        nearest_internal_grapheme_boundary(&cur_text, pos)
                    };
                    let right_text = cur_text.split_off(split_pos);
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
            &mut Node::Internal(ref mut children) => {
                // Find the child to traverse into along with its starting char
                // offset.
                let (child_i, start_info) =
                    children.search_combine_info(|inf| char_pos <= inf.chars);
                let start_char = start_info.chars;

                // Navigate into the appropriate child
                let (residual, child_seam) = Arc::make_mut(&mut children.nodes_mut()[child_i])
                    .insert(char_pos - start_char, text);
                children.info_mut()[child_i] = children.nodes()[child_i].text_info();

                // Calculate the seam offset relative to this node
                let seam = child_seam.map(|byte_pos| byte_pos + start_info.bytes);

                // Handle the new node, if any.
                if let Some(r_node) = residual {
                    // The new node will fit as a child of this node
                    if children.len() < MAX_CHILDREN {
                        children.insert(child_i + 1, (r_node.text_info(), Arc::new(r_node)));
                        return (None, seam);
                    }
                    // The new node won't fit!  Must split.
                    else {
                        let r_children = children.insert_split(
                            child_i + 1,
                            (r_node.text_info(), Arc::new(r_node)),
                        );

                        return (Some(Node::Internal(r_children)), seam);
                    }
                } else {
                    // No new node.  Easy.
                    return (None, seam);
                }
            }
        }
    }

    // Recursive function.  The returned bool means:
    //
    // - True: I'm too small now, merge me with a neighbor.
    // - False: I'm fine, no need to merge.
    pub(crate) fn remove(&mut self, start: usize, end: usize) -> bool {
        debug_assert!(start <= end);
        if start == end {
            return false;
        }

        match self {
            &mut Node::Leaf(ref mut cur_text) => {
                debug_assert!(end <= cur_text.chars().count());
                let start_byte = char_idx_to_byte_idx(&cur_text, start);
                let end_byte = char_idx_to_byte_idx(&cur_text, end);
                cur_text.remove_range(start_byte, end_byte);

                assert!(cur_text.len() > 0);
                return cur_text.len() < MIN_BYTES;
            }

            &mut Node::Internal(ref mut children) => {
                // Find the end-point nodes of the removal
                let (l_child_i, l_acc_info) =
                    children.search_combine_info(|inf| start as Count <= inf.chars);
                let (mut r_child_i, r_acc_info) =
                    children.search_combine_info(|inf| end as Count <= inf.chars);

                let l_merge; // Flag for whether to merge the left node
                let r_merge; // Flag for whether to merge the right node
                let l_gone; // Flag for whether the left node is completely removed
                let r_gone; // Flag for whether the right node is completely removed

                // Do the removal
                if l_child_i == r_child_i {
                    let l_start = start - l_acc_info.chars as usize;
                    let l_end = end - l_acc_info.chars as usize;
                    l_gone = false;
                    r_gone = false;
                    r_merge = false;

                    l_merge = if (l_start == 0) &&
                        (l_end == children.info()[l_child_i as usize].chars as usize)
                    {
                        children.remove(l_child_i);
                        false
                    } else {
                        // Remove the text
                        let m = Arc::make_mut(&mut children.nodes_mut()[l_child_i as usize])
                            .remove(l_start, l_end);

                        // Update child info
                        children.info_mut()[l_child_i] = children.nodes()[l_child_i as usize]
                            .text_info();
                        m
                    };
                } else {
                    // Calculate the local char indices to remove for the left- and
                    // right-most nodes that touch the removal range.
                    let l_start = start - l_acc_info.chars as usize;
                    let l_end = children.info()[l_child_i as usize].chars as usize;
                    let r_start = 0;
                    let r_end = end - r_acc_info.chars as usize;

                    // Determine if the left-most or right-most nodes need to be
                    // completely removed.
                    l_gone = l_start == 0;
                    r_gone = r_end == children.info()[r_child_i as usize].chars as usize;

                    // Remove children that are completely encompassed
                    // in the range.
                    let removal_start = if l_gone { l_child_i } else { l_child_i + 1 };
                    let removal_end = if r_gone { r_child_i + 1 } else { r_child_i };
                    for _ in (removal_start as usize)..(removal_end as usize) {
                        children.remove(removal_start);
                    }

                    // Update r_child_i based on removals
                    r_child_i = if l_gone { l_child_i } else { l_child_i + 1 };

                    // Remove the text from the left and right nodes
                    // and update their text info.
                    let (info, nodes) = children.info_and_nodes_mut();
                    l_merge = if !l_gone {
                        let m =
                            Arc::make_mut(&mut nodes[l_child_i as usize]).remove(l_start, l_end);
                        info[l_child_i as usize] = nodes[l_child_i as usize].text_info();
                        m
                    } else {
                        false
                    };

                    let r_merge = if !r_gone {
                        let m =
                            Arc::make_mut(&mut nodes[r_child_i as usize]).remove(r_start, r_end);
                        info[r_child_i as usize] = nodes[r_child_i as usize].text_info();
                        m
                    } else {
                        false
                    };
                }

                // TODO:
                // Do the merging, if necessary.
                // if l_merge && r_merge {
                //
                // } else if l_merge {

                // } else if r_merge {

                // }

                debug_assert!(children.len() > 0);
                return children.len() < MIN_CHILDREN;
            }
        }
    }

    /// Splits the `Node` at char index `char_idx`, returning
    /// the right side of the split.
    pub fn split(&mut self, char_idx: usize) -> Node {
        match self {
            &mut Node::Leaf(ref mut text) => {
                let char_idx = char_idx_to_byte_idx(text, char_idx);
                Node::Leaf(text.split_off(char_idx))
            }
            &mut Node::Internal(ref mut children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| char_idx as Count <= inf.chars);
                let child_info = children.info()[child_i];

                if char_idx == acc_info.chars as usize {
                    Node::Internal(children.split_off(child_i))
                } else if char_idx == (acc_info.chars as usize + child_info.chars as usize) {
                    Node::Internal(children.split_off(child_i + 1))
                } else {
                    let mut r_children = children.split_off(child_i + 1);

                    // Recurse
                    let r_node = Arc::make_mut(&mut children.nodes_mut()[child_i]).split(
                        char_idx - acc_info.chars as usize,
                    );

                    r_children.insert(0, (r_node.text_info(), Arc::new(r_node)));

                    // TODO: optimize for not having to do this every time
                    if children.len() > 1 {
                        children.merge_distribute(child_i - 1, child_i);
                    }
                    if r_children.len() > 1 {
                        r_children.merge_distribute(0, 1);
                    }

                    Node::Internal(r_children)
                }
            }
        }
    }

    /// Returns the char index of the given byte.
    pub(crate) fn byte_to_char(&self, byte_idx: usize) -> usize {
        match self {
            &Node::Leaf(ref text) => byte_idx_to_char_idx(text, byte_idx),
            &Node::Internal(ref children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| byte_idx as Count <= inf.bytes);

                // Shortcuts
                if byte_idx == 0 {
                    return 0;
                } else if byte_idx ==
                           acc_info.bytes as usize + children.info()[child_i].bytes as usize
                {
                    return acc_info.chars as usize + children.info()[child_i].chars as usize;
                }

                acc_info.chars as usize +
                    children.nodes()[child_i].byte_to_char(byte_idx - acc_info.bytes as usize)
            }
        }
    }

    /// Returns the line index of the given byte.
    pub(crate) fn byte_to_line(&self, byte_idx: usize) -> usize {
        match self {
            &Node::Leaf(ref text) => byte_idx_to_line_idx(text, byte_idx),
            &Node::Internal(ref children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| byte_idx as Count <= inf.bytes);

                // Shortcuts
                if byte_idx == 0 {
                    return 0;
                } else if byte_idx ==
                           acc_info.bytes as usize + children.info()[child_i].bytes as usize
                {
                    return acc_info.line_breaks as usize +
                        children.info()[child_i].line_breaks as usize;
                }

                acc_info.line_breaks as usize +
                    children.nodes()[child_i].byte_to_line(byte_idx - acc_info.bytes as usize)
            }
        }
    }

    /// Returns the byte index of the given char.
    pub(crate) fn char_to_byte(&self, char_idx: usize) -> usize {
        match self {
            &Node::Leaf(ref text) => char_idx_to_byte_idx(text, char_idx),
            &Node::Internal(ref children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| char_idx as Count <= inf.chars);

                // Shortcuts
                if char_idx == 0 {
                    return 0;
                } else if char_idx ==
                           acc_info.chars as usize + children.info()[child_i].chars as usize
                {
                    return acc_info.bytes as usize + children.info()[child_i].bytes as usize;
                }

                acc_info.bytes as usize +
                    children.nodes()[child_i].char_to_byte(char_idx - acc_info.chars as usize)
            }
        }
    }

    /// Returns the line index of the given char.
    pub(crate) fn char_to_line(&self, char_idx: usize) -> usize {
        match self {
            &Node::Leaf(ref text) => char_idx_to_line_idx(text, char_idx),
            &Node::Internal(ref children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| char_idx as Count <= inf.chars);

                // Shortcuts
                if char_idx == 0 {
                    return 0;
                } else if char_idx ==
                           acc_info.chars as usize + children.info()[child_i].chars as usize
                {
                    return acc_info.line_breaks as usize +
                        children.info()[child_i].line_breaks as usize;
                }

                acc_info.line_breaks as usize +
                    children.nodes()[child_i].char_to_line(char_idx - acc_info.chars as usize)
            }
        }
    }

    /// Returns the byte index of the start of the given line.
    pub(crate) fn line_to_byte(&self, line_idx: usize) -> usize {
        match self {
            &Node::Leaf(ref text) => line_idx_to_byte_idx(text, line_idx),
            &Node::Internal(ref children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| line_idx as Count <= inf.line_breaks);

                acc_info.bytes as usize +
                    children.nodes()[child_i].line_to_byte(line_idx - acc_info.line_breaks as usize)
            }
        }
    }

    /// Returns the char index of the start of the given line.
    pub(crate) fn line_to_char(&self, line_idx: usize) -> usize {
        match self {
            &Node::Leaf(ref text) => line_idx_to_char_idx(text, line_idx),
            &Node::Internal(ref children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| line_idx as Count <= inf.line_breaks);

                acc_info.chars as usize +
                    children.nodes()[child_i].line_to_char(line_idx - acc_info.line_breaks as usize)
            }
        }
    }

    /// Returns whether the given char index is a grapheme cluster
    /// boundary or not.
    pub fn is_grapheme_boundary(&self, char_idx: usize) -> bool {
        let (chunk, offset) = self.get_chunk_at_char(char_idx);
        let byte_idx = char_idx_to_byte_idx(chunk, offset);

        is_grapheme_boundary(chunk, byte_idx)
    }

    /// Returns the previous grapheme cluster boundary to the left of
    /// the given char index (excluding the given char index itself).
    ///
    /// If `char_idx` is at the beginning of the rope, returns 0.
    pub fn prev_grapheme_boundary(&self, char_idx: usize) -> usize {
        // Take care of special case
        if char_idx == 0 {
            return 0;
        }

        let (chunk, offset) = self.get_chunk_at_char(char_idx);
        let byte_idx = char_idx_to_byte_idx(chunk, offset);
        if byte_idx == 0 {
            if char_idx == 1 {
                // Weird special-case: if the previous chunk is only
                // one char long and is also the first chunk of the
                // rope.
                return 0;
            } else {
                let (chunk, _) = self.get_chunk_at_char(char_idx - 1);
                let prev_byte_idx = prev_grapheme_boundary(chunk, chunk.len());
                return char_idx - (&chunk[prev_byte_idx..]).chars().count();
            }
        } else {
            let prev_byte_idx = prev_grapheme_boundary(chunk, byte_idx);
            return char_idx - (&chunk[prev_byte_idx..byte_idx]).chars().count();
        }
    }

    /// Returns the next grapheme cluster boundary to the right of
    /// the given char index (excluding the given char index itself).
    ///
    /// If `char_idx` is at the end of the rope, returns the end
    /// position.
    pub fn next_grapheme_boundary(&self, char_idx: usize) -> usize {
        let end_char_idx = self.text_info().chars as usize;

        // Take care of special case
        if char_idx == end_char_idx {
            return end_char_idx;
        }

        let (chunk, offset) = self.get_chunk_at_char(char_idx);
        let byte_idx = char_idx_to_byte_idx(chunk, offset);
        if byte_idx == chunk.len() {
            if char_idx == end_char_idx - 1 {
                // Weird special-case: if the next chunk is only
                // one char long and is also the last chunk of the
                // rope.
                return end_char_idx;
            } else {
                let (chunk, _) = self.get_chunk_at_char(char_idx + 1);
                let next_byte_idx = next_grapheme_boundary(chunk, 0);
                return char_idx + (&chunk[..next_byte_idx]).chars().count();
            }
        } else {
            let next_byte_idx = next_grapheme_boundary(chunk, byte_idx);
            return char_idx + (&chunk[byte_idx..next_byte_idx]).chars().count();
        };
    }

    /// Returns an immutable slice of the Rope in the char range `start..end`.
    pub(crate) fn slice<'a>(&'a self, start: usize, end: usize) -> RopeSlice<'a> {
        RopeSlice::new_with_range(self, start, end)
    }

    pub(crate) fn text_info(&self) -> TextInfo {
        match self {
            &Node::Leaf(ref text) => TextInfo::from_str(text),
            &Node::Internal(ref children) => children.combined_info(),
        }
    }

    //-----------------------------------------

    pub(crate) fn child_count(&self) -> usize {
        if let &Node::Internal(ref children) = self {
            children.len()
        } else {
            panic!()
        }
    }

    pub(crate) fn children(&mut self) -> &mut ChildArray {
        match self {
            &mut Node::Internal(ref mut children) => children,
            _ => panic!(),
        }
    }

    pub(crate) fn leaf_text(&self) -> &str {
        if let &Node::Leaf(ref text) = self {
            text
        } else {
            panic!()
        }
    }

    /// Returns the chunk that contains the given byte, and the byte's
    /// byte-offset within the chunk.
    fn get_chunk_at_byte<'a>(&'a self, byte_idx: usize) -> (&'a str, usize) {
        match self {
            &Node::Leaf(ref text) => (text, byte_idx),
            &Node::Internal(ref children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| byte_idx as Count <= inf.bytes);
                children.nodes()[child_i].get_chunk_at_byte(byte_idx - acc_info.bytes as usize)
            }
        }
    }

    /// Returns the chunk that contains the given char, and the chars's
    /// char-offset within the chunk.
    fn get_chunk_at_char<'a>(&'a self, char_idx: usize) -> (&'a str, usize) {
        match self {
            &Node::Leaf(ref text) => (text, char_idx),
            &Node::Internal(ref children) => {
                let (child_i, acc_info) =
                    children.search_combine_info(|inf| char_idx as Count <= inf.chars);
                children.nodes()[child_i].get_chunk_at_char(char_idx - acc_info.chars as usize)
            }
        }
    }

    /// Debugging tool to make sure that all of the meta-data of the
    /// tree is consistent with the actual data.
    pub(crate) fn assert_integrity(&self) {
        match self {
            &Node::Leaf(_) => {}
            &Node::Internal(ref children) => {
                for (info, node) in children.iter() {
                    assert_eq!(*info, node.text_info());
                    node.assert_integrity();
                }
            }
        }
    }

    /// Debugging tool to make sure that all of the following invariants
    /// hold true throughout the tree:
    ///
    /// - The tree is the same height everywhere.
    /// - All internal nodes have the minimum number of children.
    /// - All leaf nodes are non-empty.
    /// - Graphemes are never split over chunk boundaries.
    pub(crate) fn assert_invariants(&self, is_root: bool) {
        self.assert_balance();
        self.assert_node_size(is_root);
        if is_root {
            self.assert_grapheme_seams();
        }
    }

    /// Checks that the entire tree is the same height everywhere.
    fn assert_balance(&self) -> usize {
        // Depth, child count, and leaf node emptiness
        match self {
            &Node::Leaf(_) => 1,
            &Node::Internal(ref children) => {
                let first_depth = children.nodes()[0].assert_balance();
                for node in &children.nodes()[1..] {
                    assert_eq!(node.assert_balance(), first_depth);
                }
                first_depth + 1
            }
        }
    }

    /// Checks that all internal nodes have the minimum number of
    /// children and all non-root leaf nodes are non-empty.
    fn assert_node_size(&self, is_root: bool) {
        match self {
            &Node::Leaf(ref text) => {
                // Leaf size
                if !is_root {
                    assert!(text.len() > 0);
                }
            }
            &Node::Internal(ref children) => {
                // Child count
                if is_root {
                    assert!(children.len() > 1);
                } else {
                    assert!(children.len() >= MIN_CHILDREN);
                }

                for node in children.nodes() {
                    node.assert_node_size(false);
                }
            }
        }
    }

    /// Checks that graphemes are never split over chunk boundaries.
    fn assert_grapheme_seams(&self) {
        let slice = self.slice(0, self.text_info().chars as usize);
        if slice.chunks().count() > 0 {
            let mut itr = slice.chunks();
            let mut last_chunk = itr.next().unwrap();
            for chunk in itr {
                if chunk.len() > 1 && last_chunk.len() > 1 {
                    assert!(seam_is_grapheme_boundary(last_chunk, chunk));
                    last_chunk = chunk;
                }
            }
        }
    }

    /// A for-fun tool for playing with silly text files.
    pub(crate) fn largest_grapheme_size(&self) -> usize {
        let mut size = 0;
        let slice = self.slice(0, self.text_info().chars as usize);
        for g in slice.graphemes() {
            if g.len() > size {
                size = g.len();
            }
        }
        size
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
            &mut Node::Leaf(ref mut text) => {
                if byte_pos == 0 || byte_pos == text.len() as Count {
                    Some(text)
                } else {
                    panic!("Byte position given is not on a leaf boundary.")
                }
            }

            &mut Node::Internal(ref mut children) => {
                if byte_pos == 0 {
                    // Special-case 1
                    return Arc::make_mut(&mut children.nodes_mut()[0]).fix_grapheme_seam(byte_pos);
                } else if byte_pos == children.combined_info().bytes {
                    // Special-case 2
                    let (info, nodes) = children.info_and_nodes_mut();
                    return Arc::make_mut(nodes.last_mut().unwrap()).fix_grapheme_seam(
                        info.last()
                            .unwrap()
                            .bytes,
                    );
                } else {
                    // Find the child to navigate into
                    let (child_i, start_info) =
                        children.search_combine_info(|inf| byte_pos <= inf.bytes);
                    let start_byte = start_info.bytes;

                    let pos_in_child = byte_pos - start_byte;
                    let child_len = children.info()[child_i].bytes;

                    if pos_in_child == 0 || pos_in_child == child_len {
                        // Left or right edge, get neighbor and fix seam
                        let (info, nodes) = children.info_and_nodes_mut();

                        let ((split_l, split_r), child_l_i) = if pos_in_child == 0 {
                            (nodes.split_at_mut(child_i), child_i - 1)
                        } else {
                            (nodes.split_at_mut(child_i + 1), child_i)
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
                        return Arc::make_mut(&mut children.nodes_mut()[child_i])
                            .fix_grapheme_seam(pos_in_child);
                    }
                }
            }
        }
    }

    /// Updates the tree meta-data down the left side of the tree.
    fn fix_info_left(&mut self) {
        match self {
            &mut Node::Leaf(_) => {}
            &mut Node::Internal(ref mut children) => {
                Arc::make_mut(children.nodes_mut().first_mut().unwrap()).fix_info_left();
                *children.info_mut().first_mut().unwrap() =
                    children.nodes().first().unwrap().text_info();
            }
        }
    }

    /// Updates the tree meta-data down the right side of the tree.
    fn fix_info_right(&mut self) {
        match self {
            &mut Node::Leaf(_) => {}
            &mut Node::Internal(ref mut children) => {
                Arc::make_mut(children.nodes_mut().last_mut().unwrap()).fix_info_right();
                *children.info_mut().last_mut().unwrap() =
                    children.nodes().last().unwrap().text_info();
            }
        }
    }
}

//===========================================================================

/// Inserts the given text into the given string at the given char index.
pub(crate) fn insert_at_char<B: Array<Item = u8>>(s: &mut SmallString<B>, text: &str, pos: usize) {
    let byte_pos = char_idx_to_byte_idx(s, pos);
    s.insert_str(byte_pos, text);
}


/// Removes the text between the given char indices in the given string.
pub(crate) fn remove_text_between_char_indices<B: Array<Item = u8>>(
    s: &mut SmallString<B>,
    pos_a: usize,
    pos_b: usize,
) {
    // Bounds checks
    assert!(
        pos_a <= pos_b,
        "remove_text_between_char_indices(): pos_a must be less than or equal to pos_b."
    );

    if pos_a == pos_b {
        return;
    }

    // Find removal positions in bytes
    // TODO: get both of these in a single pass
    let byte_pos_a = char_idx_to_byte_idx(&s[..], pos_a);
    let byte_pos_b = char_idx_to_byte_idx(&s[..], pos_b);

    // Get byte vec of string
    let byte_vec = unsafe { s.as_mut_smallvec() };

    // Move bytes to fill in the gap left by the removed bytes
    let mut from = byte_pos_b;
    let mut to = byte_pos_a;
    while from < byte_vec.len() {
        byte_vec[to] = byte_vec[from];

        from += 1;
        to += 1;
    }

    // Remove data from the end
    let final_text_size = byte_vec.len() + byte_pos_a - byte_pos_b;
    byte_vec.truncate(final_text_size);
}


/// Splits a string into two strings at the char index given.
/// The first section of the split is stored in the original string,
/// while the second section of the split is returned as a new string.
pub(crate) fn split_string_at_char<B: Array<Item = u8>>(
    s1: &mut SmallString<B>,
    pos: usize,
) -> SmallString<B> {
    let split_pos = char_idx_to_byte_idx(&s1[..], pos);
    s1.split_off(split_pos)
}

/// Takes two SmallStrings and mends the grapheme boundary between them, if any.
///
/// Note: this will leave one of the strings empty if the entire composite string
/// is one big grapheme.
pub(crate) fn fix_grapheme_seam<B: Array<Item = u8>>(
    l: &mut SmallString<B>,
    r: &mut SmallString<B>,
) {
    let tot_len = l.len() + r.len();
    let mut gc = GraphemeCursor::new(l.len(), tot_len, true);
    let next = gc.next_boundary(r, l.len()).unwrap();
    let prev = {
        match gc.prev_boundary(r, l.len()) {
            Ok(pos) => pos,
            Err(GraphemeIncomplete::PrevChunk) => gc.prev_boundary(l, 0).unwrap(),
            _ => unreachable!(),
        }
    };

    // Find the new split position, if any.
    let new_split_pos = if let (Some(a), Some(b)) = (prev, next) {
        if a == l.len() {
            // We're on a graphem boundary, don't need to do anything
            return;
        }
        if a == 0 {
            b
        } else if b == tot_len {
            a
        } else if l.len() > r.len() {
            a
        } else {
            b
        }
    } else if let Some(a) = prev {
        if a == l.len() {
            return;
        }
        a
    } else if let Some(b) = next {
        b
    } else {
        unreachable!()
    };

    // Move the bytes to create the new split
    if new_split_pos < l.len() {
        r.insert_str(0, &l[new_split_pos..]);
        l.truncate(new_split_pos);
    } else {
        let pos = new_split_pos - l.len();
        l.push_str(&r[..pos]);
        r.truncate_front(pos);
    }
}

//===========================================================================

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

//===========================================================================

#[cfg(test)]
mod tests {
    use rope::Rope;

    // 133 chars, 209 bytes
    const TEXT: &str = "\r\nHello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n";

    #[test]
    fn line_to_byte_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(3, r.root.line_break_count());
        assert_eq!(0, r.line_to_byte(0));
        assert_eq!(2, r.line_to_byte(1));
        assert_eq!(93, r.line_to_byte(2));
        assert_eq!(209, r.line_to_byte(3));
    }

    #[test]
    fn line_to_char_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(3, r.root.line_break_count());
        assert_eq!(0, r.line_to_char(0));
        assert_eq!(2, r.line_to_char(1));
        assert_eq!(93, r.line_to_char(2));
        assert_eq!(133, r.line_to_char(3));
    }

    #[test]
    fn is_grapheme_boundary_01() {
        let r = Rope::from_str(TEXT);

        assert!(r.is_grapheme_boundary(0));
        assert!(r.is_grapheme_boundary(133));
        assert!(r.is_grapheme_boundary(91));
        assert!(r.is_grapheme_boundary(93));
        assert!(r.is_grapheme_boundary(125));

        assert!(!r.is_grapheme_boundary(1));
        assert!(!r.is_grapheme_boundary(132));
        assert!(!r.is_grapheme_boundary(92));
    }

    #[test]
    fn prev_grapheme_boundary_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.prev_grapheme_boundary(0));
        assert_eq!(131, r.prev_grapheme_boundary(133));
        assert_eq!(90, r.prev_grapheme_boundary(91));
        assert_eq!(91, r.prev_grapheme_boundary(93));
        assert_eq!(124, r.prev_grapheme_boundary(125));

        assert_eq!(0, r.prev_grapheme_boundary(1));
        assert_eq!(131, r.prev_grapheme_boundary(132));
        assert_eq!(91, r.prev_grapheme_boundary(92));
    }

    #[test]
    fn next_grapheme_boundary_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(2, r.next_grapheme_boundary(0));
        assert_eq!(133, r.next_grapheme_boundary(133));
        assert_eq!(93, r.next_grapheme_boundary(91));
        assert_eq!(94, r.next_grapheme_boundary(93));
        assert_eq!(126, r.next_grapheme_boundary(125));

        assert_eq!(2, r.next_grapheme_boundary(1));
        assert_eq!(133, r.next_grapheme_boundary(132));
        assert_eq!(93, r.next_grapheme_boundary(92));
    }
}
