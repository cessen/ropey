#![allow(dead_code)]

use std;
use std::sync::Arc;

use str_utils::{count_chars, byte_idx_to_char_idx, byte_idx_to_line_idx, char_idx_to_byte_idx,
                char_idx_to_line_idx, line_idx_to_byte_idx, line_idx_to_char_idx,
                is_grapheme_boundary, prev_grapheme_boundary, next_grapheme_boundary};
use tree::{NodeChildren, NodeText, TextInfo, Count, MAX_CHILDREN, MIN_CHILDREN, MAX_BYTES,
           MIN_BYTES};
use tree::node_text::fix_grapheme_seam;


#[derive(Debug, Clone)]
pub(crate) enum Node {
    Leaf(NodeText),
    Internal(NodeChildren),
}

impl Node {
    /// Creates an empty node.
    pub fn new() -> Node {
        Node::Leaf(NodeText::from_str(""))
    }

    /// Total number of bytes in the Rope.
    pub fn byte_count(&self) -> usize {
        self.text_info().bytes as usize
    }

    /// Total number of chars in the Rope.
    pub fn char_count(&self) -> usize {
        self.text_info().chars as usize
    }

    /// Total number of line breaks in the Rope.
    pub fn line_break_count(&self) -> usize {
        self.text_info().line_breaks as usize
    }

    /// Inserts the text at the given char index.
    ///
    /// Returns a right-side residual node if the insertion wouldn't fit
    /// within this node.  Also returns the byte position where there may
    /// be a grapheme seam to fix, if any.
    ///
    /// Note: this does not handle large insertions (i.e. larger than
    /// MAX_BYTES) well.  That is handled at Rope::insert().
    pub fn insert(&mut self, char_pos: Count, text: &str) -> (Option<Node>, Option<Count>) {
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


                if cur_text.len() <= MAX_BYTES {
                    cur_text.insert_str(byte_pos, text);
                    return (None, seam);
                } else {
                    let r_text = cur_text.insert_str_split(byte_pos, text);
                    if r_text.len() > 0 {
                        return (Some(Node::Leaf(r_text)), seam);
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

    pub fn append_at_depth(&mut self, other: Arc<Node>, depth: usize) -> Option<Arc<Node>> {
        if depth == 0 {
            match self {
                &mut Node::Leaf(_) => {
                    if !other.is_leaf() {
                        panic!("Tree-append siblings have differing types.");
                    } else {
                        return Some(other);
                    }
                }
                &mut Node::Internal(ref mut children_l) => {
                    let mut other = other;
                    if let &mut Node::Internal(ref mut children_r) = Arc::make_mut(&mut other) {
                        if (children_l.len() + children_r.len()) <= MAX_CHILDREN {
                            for _ in 0..children_r.len() {
                                children_l.push(children_r.remove(0));
                            }
                            return None;
                        } else {
                            children_l.distribute_with(children_r);
                            // Return lower down, to avoid borrow-checker.
                        }
                    } else {
                        panic!("Tree-append siblings have differing types.");
                    }
                    return Some(other);
                }
            }
        } else if let &mut Node::Internal(ref mut children) = self {
            let last_i = children.len() - 1;
            let residual = Arc::make_mut(&mut children.nodes_mut()[last_i])
                .append_at_depth(other, depth - 1);
            children.update_child_info(last_i);
            if let Some(extra_node) = residual {
                if children.len() < MAX_CHILDREN {
                    children.push((extra_node.text_info(), extra_node));
                    return None;
                } else {
                    let r_children = children.push_split((extra_node.text_info(), extra_node));
                    return Some(Arc::new(Node::Internal(r_children)));
                }
            } else {
                return None;
            }
        } else {
            panic!("Reached leaf before getting to target depth.");
        }
    }

    pub fn prepend_at_depth(&mut self, other: Arc<Node>, depth: usize) -> Option<Arc<Node>> {
        if depth == 0 {
            match self {
                &mut Node::Leaf(_) => {
                    if !other.is_leaf() {
                        panic!("Tree-append siblings have differing types.");
                    } else {
                        return Some(other);
                    }
                }
                &mut Node::Internal(ref mut children_r) => {
                    let mut other = other;
                    if let &mut Node::Internal(ref mut children_l) = Arc::make_mut(&mut other) {
                        if (children_l.len() + children_r.len()) <= MAX_CHILDREN {
                            for _ in 0..children_l.len() {
                                children_r.insert(0, children_l.pop());
                            }
                            return None;
                        } else {
                            children_l.distribute_with(children_r);
                            // Return lower down, to avoid borrow-checker.
                        }
                    } else {
                        panic!("Tree-append siblings have differing types.");
                    }
                    return Some(other);
                }
            }
        } else if let &mut Node::Internal(ref mut children) = self {
            let residual = Arc::make_mut(&mut children.nodes_mut()[0])
                .prepend_at_depth(other, depth - 1);
            children.update_child_info(0);
            if let Some(extra_node) = residual {
                if children.len() < MAX_CHILDREN {
                    children.insert(0, (extra_node.text_info(), extra_node));
                    return None;
                } else {
                    let mut r_children =
                        children.insert_split(0, (extra_node.text_info(), extra_node));
                    std::mem::swap(children, &mut r_children);
                    return Some(Arc::new(Node::Internal(r_children)));
                }
            } else {
                return None;
            }
        } else {
            panic!("Reached leaf before getting to target depth.");
        }
    }

    // Recursive function.  The returned bool means:
    //
    // - True: there are problems below me that need zipping to fix.
    // - False: I'm good!  No zipping needed!
    pub fn remove(&mut self, start: usize, end: usize) -> (bool, Option<usize>) {
        debug_assert!(start <= end);
        if start == end {
            return (false, None);
        }

        let mut need_zip = false;
        let seam;

        match self {
            &mut Node::Leaf(ref mut cur_text) => {
                debug_assert!(end <= count_chars(cur_text));
                let start_byte = char_idx_to_byte_idx(&cur_text, start);
                let end_byte = char_idx_to_byte_idx(&cur_text, end);
                let is_on_edge = start_byte == 0 || end_byte == cur_text.len();
                cur_text.remove_range(start_byte, end_byte);

                assert!(cur_text.len() > 0);
                return (
                    cur_text.len() < MIN_BYTES,
                    if is_on_edge { Some(start_byte) } else { None },
                );
            }

            &mut Node::Internal(ref mut children) => {
                // Find the end-point nodes of the removal
                let (l_child_i, l_acc_info) =
                    children.search_combine_info(|inf| start as Count <= inf.chars);
                let (mut r_child_i, r_acc_info) =
                    children.search_combine_info(|inf| end as Count <= inf.chars);

                // Do the removal
                if l_child_i == r_child_i {
                    let l_start = start - l_acc_info.chars as usize;
                    let l_end = end - l_acc_info.chars as usize;

                    let start_is_edge = l_start == 0;
                    let end_is_edge = l_end == children.info()[l_child_i as usize].chars as usize;

                    // Remove
                    if start_is_edge && end_is_edge {
                        children.remove(l_child_i);
                        seam = Some(l_acc_info.bytes as usize);
                    } else {
                        // Remove the text
                        let (child_need_zip, child_seam) =
                            Arc::make_mut(&mut children.nodes_mut()[l_child_i as usize])
                                .remove(l_start, l_end);

                        // Set return values
                        need_zip |= child_need_zip;
                        seam = child_seam.map(|idx| idx + l_acc_info.bytes as usize);

                        // Update child info
                        children.info_mut()[l_child_i] = children.nodes()[l_child_i as usize]
                            .text_info();
                    }
                } else {
                    // Calculate the local char indices to remove for the left- and
                    // right-most nodes that touch the removal range.
                    let l_start = start - l_acc_info.chars as usize;
                    let l_end = children.info()[l_child_i as usize].chars as usize;
                    let r_start = 0;
                    let r_end = end - r_acc_info.chars as usize;

                    // Determine if the left-most or right-most nodes need to be
                    // completely removed.
                    let l_gone = l_start == 0;
                    let r_gone = r_end == children.info()[r_child_i as usize].chars as usize;

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
                    if !l_gone {
                        let (child_need_zip, child_seam) =
                            Arc::make_mut(&mut nodes[l_child_i as usize]).remove(l_start, l_end);
                        info[l_child_i as usize] = nodes[l_child_i as usize].text_info();

                        // Set return values
                        need_zip |= child_need_zip;
                        seam = child_seam.map(|idx| idx + l_acc_info.bytes as usize);
                    } else {
                        seam = Some(l_acc_info.bytes as usize);
                    }

                    if !r_gone {
                        need_zip |= Arc::make_mut(&mut nodes[r_child_i as usize])
                            .remove(r_start, r_end)
                            .0;
                        info[r_child_i as usize] = nodes[r_child_i as usize].text_info();
                    }
                }

                // Do the merging, if necessary.
                // First, merge left and right if necessary/possible.
                if (l_child_i + 1) < children.len() &&
                    (children.nodes()[l_child_i].is_undersized() ||
                         children.nodes()[l_child_i + 1].is_undersized())
                {
                    children.merge_distribute(l_child_i, l_child_i + 1);
                }
                // Second, try to merge the left child again, if necessary/possible
                if children.len() > 1 && children.nodes()[l_child_i].is_undersized() {
                    if l_child_i == 0 {
                        children.merge_distribute(0, 1);
                    } else {
                        children.merge_distribute(l_child_i - 1, l_child_i);
                    }
                }

                assert!(children.is_info_accurate());

                debug_assert!(children.len() > 0);
                return (
                    need_zip ||
                        (l_child_i < children.len() &&
                             children.nodes()[l_child_i].is_undersized()),
                    seam,
                );
            }
        }
    }

    /// Splits the `Node` at char index `char_idx`, returning
    /// the right side of the split.
    pub fn split(&mut self, char_idx: usize) -> Node {
        debug_assert!(char_idx != 0);
        debug_assert!(char_idx != (self.text_info().chars as usize));
        match self {
            &mut Node::Leaf(ref mut text) => {
                let byte_idx = char_idx_to_byte_idx(text, char_idx);
                Node::Leaf(text.split_off(byte_idx))
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

                    children.update_child_info(child_i);
                    r_children.update_child_info(0);

                    Node::Internal(r_children)
                }
            }
        }
    }

    /// Returns the char index of the given byte.
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
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
    pub fn byte_to_line(&self, byte_idx: usize) -> usize {
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
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
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
    pub fn char_to_line(&self, char_idx: usize) -> usize {
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
    pub fn line_to_byte(&self, line_idx: usize) -> usize {
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
    pub fn line_to_char(&self, line_idx: usize) -> usize {
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
                return char_idx - count_chars(&chunk[prev_byte_idx..]);
            }
        } else {
            let prev_byte_idx = prev_grapheme_boundary(chunk, byte_idx);
            return char_idx - count_chars(&chunk[prev_byte_idx..byte_idx]);
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
                return char_idx + count_chars(&chunk[..next_byte_idx]);
            }
        } else {
            let next_byte_idx = next_grapheme_boundary(chunk, byte_idx);
            return char_idx + count_chars(&chunk[byte_idx..next_byte_idx]);
        };
    }

    pub fn text_info(&self) -> TextInfo {
        match self {
            &Node::Leaf(ref text) => TextInfo::from_str(text),
            &Node::Internal(ref children) => children.combined_info(),
        }
    }

    //-----------------------------------------

    pub fn child_count(&self) -> usize {
        if let &Node::Internal(ref children) = self {
            children.len()
        } else {
            panic!()
        }
    }

    pub fn children(&mut self) -> &mut NodeChildren {
        match self {
            &mut Node::Internal(ref mut children) => children,
            _ => panic!(),
        }
    }

    pub fn leaf_text(&self) -> &str {
        if let &Node::Leaf(ref text) = self {
            text
        } else {
            panic!()
        }
    }

    pub fn is_leaf(&self) -> bool {
        match self {
            &Node::Leaf(_) => true,
            &Node::Internal(_) => false,
        }
    }

    pub fn is_undersized(&self) -> bool {
        match self {
            &Node::Leaf(ref text) => text.len() < MIN_BYTES,
            &Node::Internal(ref children) => children.len() < MIN_CHILDREN,
        }
    }

    /// How many nodes deep the tree is.
    ///
    /// This counts root and leafs.  For example, a single leaf node
    /// has depth 1.
    pub fn depth(&self) -> usize {
        1 +
            match self {
                &Node::Leaf(_) => 0,
                &Node::Internal(ref children) => children.nodes()[0].depth(),
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
    pub fn assert_integrity(&self) {
        match self {
            &Node::Leaf(_) => {}
            &Node::Internal(ref children) => {
                for (info, node) in children.iter() {
                    if *info != node.text_info() {
                        assert_eq!(*info, node.text_info());
                    }
                    node.assert_integrity();
                }
            }
        }
    }

    /// Checks that the entire tree is the same height everywhere.
    pub fn assert_balance(&self) -> usize {
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
    pub fn assert_node_size(&self, is_root: bool) {
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

    /// Checks to make sure that a boundary between leaf nodes (given as a byte
    /// position in the rope) doesn't split a grapheme, and fixes it if it does.
    ///
    /// If `must_be_boundary` is true, panics if the given byte position is
    /// not on the boundary between two leaf nodes.
    ///
    /// NOTE: theoretically can leave an internal node with few than MIN_CHILDREN
    /// children, though that would require an insanely long grapheme.  Given how
    /// unlikely it is, it doesn't seem worth handling.  Code shouldn't break on
    /// such cases anyway.
    pub fn fix_grapheme_seam<'a>(
        &'a mut self,
        byte_pos: Count,
        must_be_boundary: bool,
    ) -> Option<&'a mut NodeText> {
        match self {
            &mut Node::Leaf(ref mut text) => {
                if (!must_be_boundary) || byte_pos == 0 || byte_pos == text.len() as Count {
                    Some(text)
                } else {
                    panic!("Byte position given is not on a leaf boundary.")
                }
            }

            &mut Node::Internal(ref mut children) => {
                if byte_pos == 0 {
                    // Special-case 1
                    return Arc::make_mut(&mut children.nodes_mut()[0])
                        .fix_grapheme_seam(byte_pos, must_be_boundary);
                } else if byte_pos == children.combined_info().bytes {
                    // Special-case 2
                    let (info, nodes) = children.info_and_nodes_mut();
                    return Arc::make_mut(nodes.last_mut().unwrap()).fix_grapheme_seam(
                        info.last()
                            .unwrap()
                            .bytes,
                        must_be_boundary,
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
                        let l_child_i;
                        // Scope for borrow
                        {
                            if pos_in_child == 0 {
                                debug_assert!(child_i != 0);
                                l_child_i = child_i - 1;
                            } else {
                                debug_assert!(child_i < children.len());
                                l_child_i = child_i;
                            }

                            let (mut l_child, mut r_child) =
                                children.get_two_mut(l_child_i, l_child_i + 1);
                            let l_child_bytes = l_child.0.bytes;
                            let l_child = Arc::make_mut(&mut l_child.1);
                            let r_child = Arc::make_mut(&mut r_child.1);
                            fix_grapheme_seam(
                                l_child
                                    .fix_grapheme_seam(l_child_bytes, must_be_boundary)
                                    .unwrap(),
                                r_child.fix_grapheme_seam(0, must_be_boundary).unwrap(),
                            );

                            l_child.fix_info_right();
                            r_child.fix_info_left();
                        }

                        children.update_child_info(l_child_i);
                        children.update_child_info(l_child_i + 1);
                        if children.info()[l_child_i + 1].bytes == 0 {
                            children.remove(l_child_i + 1);
                        } else if children.info()[l_child_i].bytes == 0 {
                            children.remove(l_child_i);
                        }

                        return None;
                    } else {
                        // Internal to child
                        // WARNING: we use raw pointers to work around the borrow
                        // checker here, so be careful when modifying this code!
                        {
                            let raw_text = Arc::make_mut(&mut children.nodes_mut()[child_i])
                                .fix_grapheme_seam(pos_in_child, must_be_boundary)
                                .map(|text| text as *mut NodeText);

                            // This is the bit we have to work arround.  If raw_text
                            // weren't cast to a raw_point, it's &mut would keep us
                            // from calling this.  However, this is actually safe,
                            // since it doesn't modify the `Node`.
                            children.update_child_info(child_i);

                            // If the node isn't empty, return the text.
                            if children.info()[child_i].bytes > 0 {
                                return raw_text.map(|text| unsafe { &mut *text });
                            }
                        }

                        // If the node _is_ empty, remove it.
                        children.remove(child_i);
                        return None;
                    }
                }
            }
        }
    }

    /// Updates the tree meta-data down the left side of the tree, and removes empty
    /// children as it goes as well.
    fn fix_info_left(&mut self) {
        match self {
            &mut Node::Leaf(_) => {}
            &mut Node::Internal(ref mut children) => {
                Arc::make_mut(&mut children.nodes_mut()[0]).fix_info_left();
                children.update_child_info(0);
                if children.info()[0].bytes == 0 {
                    children.remove(0);
                }
            }
        }
    }

    /// Updates the tree meta-data down the right side of the tree, and removes empty
    /// children as it goes as well.
    fn fix_info_right(&mut self) {
        match self {
            &mut Node::Leaf(_) => {}
            &mut Node::Internal(ref mut children) => {
                let idx = children.len() - 1;
                Arc::make_mut(&mut children.nodes_mut()[idx]).fix_info_right();
                children.update_child_info(idx);
                if children.info()[idx].bytes == 0 {
                    children.remove(idx);
                }
            }
        }
    }

    /// Fixes dangling nodes down the left side of the tree.
    ///
    /// Returns whether it did anything or not that would affect the
    /// parent.
    pub fn zip_fix_left(&mut self) -> bool {
        if let &mut Node::Internal(ref mut children) = self {
            let mut did_stuff = false;
            loop {
                let do_merge = (children.len() > 1) &&
                    match *children.nodes()[0] {
                        Node::Leaf(ref text) => text.len() < MIN_BYTES,
                        Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                    };

                if do_merge {
                    did_stuff |= children.merge_distribute(0, 1);
                }

                if !Arc::make_mut(&mut children.nodes_mut()[0]).zip_fix_left() {
                    break;
                }
            }
            did_stuff
        } else {
            false
        }
    }

    /// Fixes dangling nodes down the right side of the tree.
    ///
    /// Returns whether it did anything or not that would affect the
    /// parent. True: did stuff, false: didn't do stuff
    pub fn zip_fix_right(&mut self) -> bool {
        if let &mut Node::Internal(ref mut children) = self {
            let mut did_stuff = false;
            loop {
                let last_i = children.len() - 1;
                let do_merge = (children.len() > 1) &&
                    match *children.nodes()[last_i] {
                        Node::Leaf(ref text) => text.len() < MIN_BYTES,
                        Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                    };

                if do_merge {
                    did_stuff |= children.merge_distribute(last_i - 1, last_i);
                }

                if !Arc::make_mut(&mut children.nodes_mut().last_mut().unwrap()).zip_fix_right() {
                    break;
                }
            }
            did_stuff
        } else {
            false
        }
    }

    /// Fixes dangling nodes down the middle of the tree.
    ///
    /// Returns whether it did anything or not that would affect the
    /// parent. True: did stuff, false: didn't do stuff
    pub fn zip_fix(&mut self, char_idx: usize) -> bool {
        if let &mut Node::Internal(ref mut children) = self {
            let mut did_stuff = false;
            loop {
                // Do merging
                let (child_i, start_info) =
                    children.search_combine_info(|inf| char_idx <= inf.chars as usize);
                let end_info = start_info.combine(&children.info()[child_i]);

                if end_info.chars as usize == char_idx && (child_i + 1) < children.len() {
                    let do_merge = match *children.nodes()[child_i] {
                        Node::Leaf(ref text) => text.len() < MIN_BYTES,
                        Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                    } ||
                        match *children.nodes()[child_i + 1] {
                            Node::Leaf(ref text) => text.len() < MIN_BYTES,
                            Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                        };

                    if do_merge {
                        did_stuff |= children.merge_distribute(child_i, child_i + 1);
                    }
                } else {
                    let do_merge = match *children.nodes()[child_i] {
                        Node::Leaf(ref text) => text.len() < MIN_BYTES,
                        Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                    };

                    if do_merge {
                        if child_i == 0 {
                            did_stuff |= children.merge_distribute(0, 1);
                        } else {
                            did_stuff |= children.merge_distribute(child_i - 1, child_i);
                        }
                    }
                }

                // Do recursion
                let (child_i, start_info) =
                    children.search_combine_info(|inf| char_idx <= inf.chars as usize);
                let end_info = start_info.combine(&children.info()[child_i]);

                if end_info.chars as usize == char_idx && (child_i + 1) < children.len() {
                    let tmp = children.info()[child_i].chars as usize;
                    let effect_1 = Arc::make_mut(&mut children.nodes_mut()[child_i]).zip_fix(tmp);
                    let effect_2 = Arc::make_mut(&mut children.nodes_mut()[child_i + 1]).zip_fix(0);
                    if (!effect_1) && (!effect_2) {
                        break;
                    }
                } else {
                    if !Arc::make_mut(&mut children.nodes_mut()[child_i]).zip_fix(
                        char_idx - start_info.chars as usize,
                    )
                    {
                        break;
                    }
                }
            }
            assert!(children.is_info_accurate());
            did_stuff
        } else {
            false
        }
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
