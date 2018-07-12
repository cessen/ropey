use std;
use std::sync::Arc;

use str_utils::{byte_idx_to_line_idx, char_idx_to_byte_idx};
use tree::node_text::fix_segment_seam;
use tree::{
    Count, NodeChildren, NodeText, TextInfo, MAX_BYTES, MAX_CHILDREN, MIN_BYTES, MIN_CHILDREN,
};

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Leaf(NodeText),
    Internal(NodeChildren),
}

impl Node {
    /// Creates an empty node.
    #[inline(always)]
    pub fn new() -> Self {
        Node::Leaf(NodeText::from_str(""))
    }

    /// Total number of bytes in the Rope.
    #[inline(always)]
    pub fn byte_count(&self) -> usize {
        self.text_info().bytes as usize
    }

    /// Total number of chars in the Rope.
    #[inline(always)]
    pub fn char_count(&self) -> usize {
        self.text_info().chars as usize
    }

    /// Total number of line breaks in the Rope.
    #[inline(always)]
    pub fn line_break_count(&self) -> usize {
        self.text_info().line_breaks as usize
    }

    /// Edits nodes in range `start_idx..end_idx`.
    ///
    /// Nodes completely subsumed by the range will be removed except the
    /// leftmost node even if it is subsumed, and the remaining 1 or 2 leaf
    /// nodes overlapping the range are passed to the given closure.
    ///
    /// The closure function parameters are:
    ///
    /// 1. The accumulated text info of the rope up to the left edge
    ///    of the selected leaf node.
    /// 2. The current text info of the selected leaf node.
    /// 3. The selected leaf node's text, for editing.
    ///
    /// The closure return values are:
    ///
    /// 1. The text info of the selected leaf node after the edits.
    /// 2. An optional new leaf node to the right of the selected leaf node,
    ///    along with its text info.
    ///
    /// WARNING: does not correctly handle all text being removed.  That
    /// should be special-cased in calling code.
    pub fn edit_char_range<F>(
        &mut self,
        start_idx: usize,
        end_idx: usize,
        mut edit: F,
    ) -> (TextInfo, Option<(TextInfo, Arc<Node>)>)
    where
        F: FnMut(TextInfo, TextInfo, &mut NodeText) -> (TextInfo, Option<(TextInfo, NodeText)>),
    {
        debug_assert!(start_idx <= end_idx);
        debug_assert!(end_idx <= self.text_info().chars as usize);

        match *self {
            Node::Leaf(_) => {
                let cur_info = self.text_info();
                self.edit_char_range_internal(
                    start_idx,
                    end_idx,
                    TextInfo::new(),
                    cur_info,
                    &mut edit,
                )
            }
            Node::Internal(_) => self.edit_char_range_internal(
                start_idx,
                end_idx,
                TextInfo::new(),
                TextInfo::new(),
                &mut edit,
            ),
        }
    }

    // Internal implementation of edit_char_range(), above.
    fn edit_char_range_internal<F>(
        &mut self,
        start_idx: usize,
        end_idx: usize,
        acc_info: TextInfo,
        cur_info: TextInfo,
        edit: &mut F,
    ) -> (TextInfo, Option<(TextInfo, Arc<Node>)>)
    where
        F: FnMut(TextInfo, TextInfo, &mut NodeText) -> (TextInfo, Option<(TextInfo, NodeText)>),
    {
        match *self {
            // If it's a leaf
            Node::Leaf(ref mut cur_text) => {
                let (info, residual) = edit(acc_info, cur_info, cur_text);

                if let Some((r_info, r_text)) = residual {
                    (info, Some((r_info, Arc::new(Node::Leaf(r_text)))))
                } else {
                    (info, None)
                }
            }

            // If it's internal, it's much more complicated
            Node::Internal(ref mut children) => {
                // Shared code for handling children.
                let mut handle_child = |children: &mut NodeChildren,
                                        child_i: usize,
                                        c_acc_info: TextInfo|
                 -> Option<Arc<Node>> {
                    // Recurse into child
                    let tmp_info = children.info()[child_i];
                    let tmp_chars = children.info()[child_i].chars as usize;
                    let (new_info, residual) = Arc::make_mut(&mut children.nodes_mut()[child_i])
                        .edit_char_range_internal(
                            start_idx - (c_acc_info.chars as usize).min(start_idx),
                            (end_idx - c_acc_info.chars as usize).min(tmp_chars),
                            acc_info + c_acc_info,
                            tmp_info,
                            edit,
                        );

                    // Handle result
                    if new_info.bytes == 0 {
                        children.remove(child_i);
                        debug_assert!(residual.is_none());
                        return None;
                    } else {
                        children.info_mut()[child_i] = new_info;

                        // Handle node residual
                        if let Some((info, node)) = residual {
                            // The new node will fit as a child of this node
                            if children.len() < MAX_CHILDREN {
                                children.insert(child_i + 1, (info, node));
                                return None;
                            }
                            // The new node won't fit!  Must split.
                            else {
                                return Some(Arc::new(Node::Internal(
                                    children.insert_split(child_i + 1, (info, node)),
                                )));
                            }
                        } else {
                            return None;
                        }
                    }
                };

                // Shared code for merging children
                let merge_child = |children: &mut NodeChildren,
                                   split_node: &mut Option<Arc<Node>>,
                                   child_i: usize|
                 -> bool {
                    if child_i < children.len() {
                        if children.len() > 1 && children.nodes()[child_i].is_undersized() {
                            if child_i == 0 {
                                children.merge_distribute(child_i, child_i + 1)
                            } else {
                                children.merge_distribute(child_i - 1, child_i)
                            }
                        } else {
                            false
                        }
                    } else if let Some(ref mut node) = *split_node {
                        let r_children = Arc::make_mut(node).children();
                        let child_i = child_i - children.len();
                        if r_children.len() > 1 && r_children.nodes()[child_i].is_undersized() {
                            if child_i == 0 {
                                r_children.merge_distribute(child_i, child_i + 1)
                            } else {
                                r_children.merge_distribute(child_i - 1, child_i)
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                // Compact leaf children if we're close to maximum leaf
                // fragmentation.
                if children.is_full()
                    && children.nodes()[0].is_leaf()
                    && (children.combined_info().bytes as usize) < (MAX_BYTES * (MIN_CHILDREN + 1))
                {
                    children.compact_leaves();
                }

                // Early-out optimization, to make simple insertion faster
                if start_idx == end_idx {
                    let (child_i, child_acc_info) = children.search_char_idx(start_idx);
                    let residual = handle_child(children, child_i, child_acc_info);
                    return (
                        children.combined_info(),
                        residual.map(|c| (c.text_info(), c)),
                    );
                }

                // Get child info for the two char indices
                let ((l_child_i, l_acc_info), (r_child_i, r_acc_info)) =
                    children.search_char_idx_range(start_idx, end_idx);

                // Both indices point into the same child
                if l_child_i == r_child_i {
                    let mut residual = handle_child(children, l_child_i, l_acc_info);
                    merge_child(children, &mut residual, l_child_i);

                    return (
                        children.combined_info(),
                        residual.map(|c| (c.text_info(), c)),
                    );
                }
                // We're dealing with more than one child.
                else {
                    // Calculate the start..end range of nodes to be removed.
                    let r_child_exists: bool;
                    let start_i = l_child_i + 1;
                    let end_i = if (r_acc_info.chars + children.info()[r_child_i].chars) as usize
                        == end_idx
                    {
                        r_child_exists = false;
                        r_child_i + 1
                    } else {
                        r_child_exists = true;
                        r_child_i
                    };

                    // Remove the children
                    for _ in start_i..end_i {
                        children.remove(start_i);
                    }

                    let tot_children = children.len(); // Used later during merging

                    // Handle right child
                    let mut split_children = if r_child_exists {
                        handle_child(children, l_child_i + 1, r_acc_info)
                    } else {
                        None
                    };

                    // Handle left child
                    if split_children.is_none() {
                        // We have to check because merging may have
                        split_children = handle_child(children, l_child_i, l_acc_info);
                    } else if l_child_i < children.len() {
                        let tmp = handle_child(children, l_child_i, l_acc_info);
                        debug_assert!(tmp.is_none());
                    } else if let Some(ref mut r_children) = split_children {
                        let tmp = handle_child(
                            Arc::make_mut(r_children).children(),
                            l_child_i - children.len(),
                            l_acc_info,
                        );
                        debug_assert!(tmp.is_none());
                    }

                    // Handle merging
                    let merge_extent = {
                        let new_tot_children = children.len()
                            + split_children
                                .as_ref()
                                .map(|c| c.child_count())
                                .unwrap_or(0);
                        1 + new_tot_children - tot_children + if r_child_exists { 1 } else { 0 }
                    };
                    for i in (l_child_i..(l_child_i + merge_extent)).rev() {
                        merge_child(children, &mut split_children, i);
                    }

                    // Return
                    return (
                        children.combined_info(),
                        split_children.map(|c| (c.text_info(), c)),
                    );
                }
            }
        }
    }

    pub fn append_at_depth(&mut self, other: Arc<Node>, depth: usize) -> Option<Arc<Node>> {
        if depth == 0 {
            match *self {
                Node::Leaf(_) => {
                    if !other.is_leaf() {
                        panic!("Tree-append siblings have differing types.");
                    } else {
                        return Some(other);
                    }
                }
                Node::Internal(ref mut children_l) => {
                    let mut other = other;
                    if let Node::Internal(ref mut children_r) = *Arc::make_mut(&mut other) {
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
        } else if let Node::Internal(ref mut children) = *self {
            let last_i = children.len() - 1;
            let residual =
                Arc::make_mut(&mut children.nodes_mut()[last_i]).append_at_depth(other, depth - 1);
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
            match *self {
                Node::Leaf(_) => {
                    if !other.is_leaf() {
                        panic!("Tree-append siblings have differing types.");
                    } else {
                        return Some(other);
                    }
                }
                Node::Internal(ref mut children_r) => {
                    let mut other = other;
                    if let Node::Internal(ref mut children_l) = *Arc::make_mut(&mut other) {
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
        } else if let Node::Internal(ref mut children) = *self {
            let residual =
                Arc::make_mut(&mut children.nodes_mut()[0]).prepend_at_depth(other, depth - 1);
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

    /// Splits the `Node` at char index `char_idx`, returning
    /// the right side of the split.
    pub fn split(&mut self, char_idx: usize) -> Node {
        debug_assert!(char_idx != 0);
        debug_assert!(char_idx != (self.text_info().chars as usize));
        match *self {
            Node::Leaf(ref mut text) => {
                let byte_idx = char_idx_to_byte_idx(text, char_idx);
                Node::Leaf(text.split_off(byte_idx))
            }
            Node::Internal(ref mut children) => {
                let (child_i, acc_info) = children.search_char_idx(char_idx);
                let child_info = children.info()[child_i];

                if char_idx == acc_info.chars as usize {
                    Node::Internal(children.split_off(child_i))
                } else if char_idx == (acc_info.chars as usize + child_info.chars as usize) {
                    Node::Internal(children.split_off(child_i + 1))
                } else {
                    let mut r_children = children.split_off(child_i + 1);

                    // Recurse
                    let r_node = Arc::make_mut(&mut children.nodes_mut()[child_i])
                        .split(char_idx - acc_info.chars as usize);

                    r_children.insert(0, (r_node.text_info(), Arc::new(r_node)));

                    children.update_child_info(child_i);
                    r_children.update_child_info(0);

                    Node::Internal(r_children)
                }
            }
        }
    }

    /// Returns the chunk that contains the given byte, and the chunk's starting
    /// byte and char indices and the index of the line that the chunk starts on.
    ///
    /// Return takes the form of `(chunk, chunk_char_idx, chunk_byte_idx, chunk_line_idx)`.
    pub fn get_chunk_at_byte(&self, byte_idx: usize) -> (&str, usize, usize, usize) {
        let mut node = self;
        let mut byte_idx = byte_idx;
        let mut info = TextInfo::new();

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (
                        text,
                        info.bytes as usize,
                        info.chars as usize,
                        info.line_breaks as usize,
                    )
                }
                Node::Internal(ref children) => {
                    let (child_i, acc_info) = children.search_byte_idx(byte_idx);
                    info += acc_info;
                    node = &*children.nodes()[child_i];
                    byte_idx -= acc_info.bytes as usize;
                }
            }
        }
    }

    /// Returns the chunk that contains the given char, and the chunk's starting
    /// byte and char indices and the index of the line that the chunk starts on.
    ///
    /// Return takes the form of `(chunk, chunk_char_idx, chunk_byte_idx, chunk_line_idx)`.
    pub fn get_chunk_at_char(&self, char_idx: usize) -> (&str, usize, usize, usize) {
        let mut node = self;
        let mut char_idx = char_idx;
        let mut info = TextInfo::new();

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (
                        text,
                        info.bytes as usize,
                        info.chars as usize,
                        info.line_breaks as usize,
                    )
                }
                Node::Internal(ref children) => {
                    let (child_i, acc_info) = children.search_char_idx(char_idx);
                    info += acc_info;
                    node = &*children.nodes()[child_i];
                    char_idx -= acc_info.chars as usize;
                }
            }
        }
    }

    /// Returns the chunk that contains the given line break, and the chunk's
    /// starting byte and char indices and the index of the line that the
    /// chunk starts on.
    ///
    /// Note: for convenience, both the beginning and end of the rope are
    /// considered line breaks for indexing.
    ///
    /// Return takes the form of `(chunk, chunk_char_idx, chunk_byte_idx, chunk_line_idx)`.
    pub fn get_chunk_at_line_break(&self, line_break_idx: usize) -> (&str, usize, usize, usize) {
        let mut node = self;
        let mut line_break_idx = line_break_idx;
        let mut info = TextInfo::new();

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (
                        text,
                        info.bytes as usize,
                        info.chars as usize,
                        info.line_breaks as usize,
                    )
                }
                Node::Internal(ref children) => {
                    let (child_i, acc_info) = children.search_line_break_idx(line_break_idx);
                    info += acc_info;
                    node = &*children.nodes()[child_i];
                    line_break_idx -= acc_info.line_breaks as usize;
                }
            }
        }
    }

    /// Returns the byte and line index of the given char.
    #[inline(always)]
    pub fn char_to_byte_and_line(&self, char_idx: usize) -> (usize, usize) {
        let (chunk, b, c, l) = self.get_chunk_at_char(char_idx);
        let bi = char_idx_to_byte_idx(chunk, char_idx - c);
        (b + bi, l + byte_idx_to_line_idx(chunk, bi))
    }

    pub fn text_info(&self) -> TextInfo {
        match *self {
            Node::Leaf(ref text) => TextInfo::from_str(text),
            Node::Internal(ref children) => children.combined_info(),
        }
    }

    //-----------------------------------------

    pub fn child_count(&self) -> usize {
        if let Node::Internal(ref children) = *self {
            children.len()
        } else {
            panic!()
        }
    }

    pub fn children(&mut self) -> &mut NodeChildren {
        match *self {
            Node::Internal(ref mut children) => children,
            _ => panic!(),
        }
    }

    pub fn leaf_text(&self) -> &str {
        if let Node::Leaf(ref text) = *self {
            text
        } else {
            panic!()
        }
    }

    pub fn leaf_text_mut(&mut self) -> &mut NodeText {
        if let Node::Leaf(ref mut text) = *self {
            text
        } else {
            panic!()
        }
    }

    pub fn is_leaf(&self) -> bool {
        match *self {
            Node::Leaf(_) => true,
            Node::Internal(_) => false,
        }
    }

    pub fn is_undersized(&self) -> bool {
        match *self {
            Node::Leaf(ref text) => text.len() < MIN_BYTES,
            Node::Internal(ref children) => children.len() < MIN_CHILDREN,
        }
    }

    /// How many nodes deep the tree is.
    ///
    /// This counts root and leafs.  For example, a single leaf node
    /// has depth 1.
    pub fn depth(&self) -> usize {
        let mut node = self;
        let mut depth = 0;

        loop {
            match *node {
                Node::Leaf(_) => return depth,
                Node::Internal(ref children) => {
                    depth += 1;
                    node = &*children.nodes()[0];
                }
            }
        }
    }

    /// Debugging tool to make sure that all of the meta-data of the
    /// tree is consistent with the actual data.
    pub fn assert_integrity(&self) {
        match *self {
            Node::Leaf(_) => {}
            Node::Internal(ref children) => for (info, node) in children.iter() {
                if *info != node.text_info() {
                    assert_eq!(*info, node.text_info());
                }
                node.assert_integrity();
            },
        }
    }

    /// Checks that the entire tree is the same height everywhere.
    pub fn assert_balance(&self) -> usize {
        // Depth, child count, and leaf node emptiness
        match *self {
            Node::Leaf(_) => 1,
            Node::Internal(ref children) => {
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
        match *self {
            Node::Leaf(ref text) => {
                // Leaf size
                if !is_root {
                    assert!(text.len() > 0);
                }
            }
            Node::Internal(ref children) => {
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
    /// TODO: theoretically this can leave an internal node with fewer than
    /// MIN_CHILDREN children, although it is exceedingly unlikely with any
    /// remotely sane text.  In the mean time, right now no code actually
    /// depends on there being at least MIN_CHILDREN in an internal node.
    /// But this should nevertheless get addressed at some point.
    /// Probably the most straight-forward way to address this is via the
    /// `fix_info_*` methods below, but I'm not totally sure.
    pub fn fix_grapheme_seam(
        &mut self,
        byte_pos: Count,
        must_be_boundary: bool,
    ) -> Option<&mut NodeText> {
        match *self {
            Node::Leaf(ref mut text) => {
                if (!must_be_boundary) || byte_pos == 0 || byte_pos == text.len() as Count {
                    Some(text)
                } else {
                    panic!("Byte position given is not on a leaf boundary.")
                }
            }

            Node::Internal(ref mut children) => {
                if byte_pos == 0 {
                    // Special-case 1
                    return Arc::make_mut(&mut children.nodes_mut()[0])
                        .fix_grapheme_seam(byte_pos, must_be_boundary);
                } else if byte_pos == children.combined_info().bytes {
                    // Special-case 2
                    let (info, nodes) = children.data_mut();
                    return Arc::make_mut(nodes.last_mut().unwrap())
                        .fix_grapheme_seam(info.last().unwrap().bytes, must_be_boundary);
                } else {
                    // Find the child to navigate into
                    let (child_i, start_info) = children.search_byte_idx(byte_pos as usize);
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
                            fix_segment_seam(
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
        match *self {
            Node::Leaf(_) => {}
            Node::Internal(ref mut children) => {
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
        match *self {
            Node::Leaf(_) => {}
            Node::Internal(ref mut children) => {
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
        if let Node::Internal(ref mut children) = *self {
            let mut did_stuff = false;
            loop {
                let do_merge = (children.len() > 1) && match *children.nodes()[0] {
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
        if let Node::Internal(ref mut children) = *self {
            let mut did_stuff = false;
            loop {
                let last_i = children.len() - 1;
                let do_merge = (children.len() > 1) && match *children.nodes()[last_i] {
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
        if let Node::Internal(ref mut children) = *self {
            let mut did_stuff = false;
            loop {
                // Do merging
                if children.len() > 1 {
                    let (child_i, start_info) =
                        children.search_combine_info(|inf| char_idx <= inf.chars as usize);
                    let end_info = start_info + children.info()[child_i];

                    if end_info.chars as usize == char_idx && (child_i + 1) < children.len() {
                        let do_merge = match *children.nodes()[child_i] {
                            Node::Leaf(ref text) => text.len() < MIN_BYTES,
                            Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                        }
                            || match *children.nodes()[child_i + 1] {
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
                }

                // Do recursion
                let (child_i, start_info) = children.search_char_idx(char_idx);
                let end_info = start_info + children.info()[child_i];

                if end_info.chars as usize == char_idx && (child_i + 1) < children.len() {
                    let tmp = children.info()[child_i].chars as usize;
                    let effect_1 = Arc::make_mut(&mut children.nodes_mut()[child_i]).zip_fix(tmp);
                    let effect_2 = Arc::make_mut(&mut children.nodes_mut()[child_i + 1]).zip_fix(0);
                    if (!effect_1) && (!effect_2) {
                        break;
                    }
                } else if !Arc::make_mut(&mut children.nodes_mut()[child_i])
                    .zip_fix(char_idx - start_info.chars as usize)
                {
                    break;
                }
            }
            debug_assert!(children.is_info_accurate());
            did_stuff
        } else {
            false
        }
    }
}

//===========================================================================

#[cfg(test)]
mod tests {
    use Rope;

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
    fn crlf_corner_case_01() {
        use super::Node;
        use std::iter;
        use std::sync::Arc;
        use tree::{NodeChildren, NodeText, MAX_BYTES};

        // Construct the corner case
        let nodel = Node::Leaf(NodeText::from_str(&iter::repeat("\n")
            .take(MAX_BYTES - 1)
            .collect::<String>()));
        let noder = Node::Leaf(NodeText::from_str(&iter::repeat("\n")
            .take(MAX_BYTES)
            .collect::<String>()));
        let mut children = NodeChildren::new();
        children.push((nodel.text_info(), Arc::new(nodel)));
        children.push((noder.text_info(), Arc::new(noder)));
        let root = Node::Internal(children);
        let mut rope = Rope {
            root: Arc::new(root),
        };
        assert_eq!(rope.char(0), '\n');
        assert_eq!(rope.len_chars(), MAX_BYTES * 2 - 1);

        // Do the potentially problematic insertion
        rope.insert(MAX_BYTES - 1, "\r");
    }

    #[test]
    fn crlf_corner_case_02() {
        use super::Node;
        use std::iter;
        use std::sync::Arc;
        use tree::{NodeChildren, NodeText, MAX_BYTES};

        // Construct the corner case
        let nodel = Node::Leaf(NodeText::from_str(&iter::repeat("\r")
            .take(MAX_BYTES)
            .collect::<String>()));
        let noder = Node::Leaf(NodeText::from_str(&iter::repeat("\r")
            .take(MAX_BYTES - 1)
            .collect::<String>()));
        let mut children = NodeChildren::new();
        children.push((nodel.text_info(), Arc::new(nodel)));
        children.push((noder.text_info(), Arc::new(noder)));
        let root = Node::Internal(children);
        let mut rope = Rope {
            root: Arc::new(root),
        };
        assert_eq!(rope.char(0), '\r');
        assert_eq!(rope.len_chars(), MAX_BYTES * 2 - 1);

        // Do the potentially problematic insertion
        rope.insert(MAX_BYTES, "\n");
    }
}
