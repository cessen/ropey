use std::sync::Arc;

use crate::str_utils::{
    byte_to_char_idx, byte_to_line_idx, byte_to_utf16_surrogate_idx, char_to_byte_idx,
};
use crate::tree::node_text::fix_segment_seam;
use crate::tree::{
    Count, NodeChildren, NodeText, TextInfo, MAX_BYTES, MAX_CHILDREN, MIN_BYTES, MIN_CHILDREN,
};

#[derive(Debug, Clone)]
#[repr(u8, C)]
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

    /// Total number of line breaks in the Rope.
    #[inline(always)]
    pub fn utf16_surrogate_count(&self) -> usize {
        self.text_info().utf16_surrogates as usize
    }

    /// Fetches a chunk mutably, and allows it to be edited via a closure.
    ///
    /// There are three parameters:
    /// - char_idx: the chunk that contains this char is fetched,
    /// - node_info: this is the text info of the node it's being called on.
    ///              This makes it a little awkward to call, but is needed since
    ///              it's actually the parent node that contains the text info,
    ///              so the info needs to be passed in.
    /// - edit: the closure that receives the chunk and does the edits.
    ///
    /// The closure is effectively the termination case for the recursion,
    /// and takes essentially same parameters and returns the same things as
    /// the method itself.  In particular, the closure receives the char offset
    /// of char_idx within the given chunk and the TextInfo of the chunk.
    /// The main difference is that it receives a NodeText instead of a node.
    ///
    /// The closure is expected to return the updated text info of the node,
    /// and if the node had to be split, then it also returns the right-hand
    /// node along with its TextInfo as well.
    ///
    /// The main method call will then return the total updated TextInfo for
    /// the whole tree, and a new node only if the whole tree had to be split.
    /// It is up to the caller to check for that new node, and handle it by
    /// creating a new root with both the original node and the new node as
    /// children.
    pub fn edit_chunk_at_char<F>(
        &mut self,
        char_idx: usize,
        node_info: TextInfo,
        mut edit: F,
    ) -> (TextInfo, Option<(TextInfo, Arc<Node>)>)
    where
        F: FnMut(usize, TextInfo, &mut NodeText) -> (TextInfo, Option<(TextInfo, Arc<Node>)>),
    {
        match *self {
            Node::Leaf(ref mut leaf_text) => edit(char_idx, node_info, leaf_text),
            Node::Internal(ref mut children) => {
                // Compact leaf children if we're very close to maximum leaf
                // fragmentation.  This basically guards against excessive memory
                // ballooning when repeatedly appending to the end of a rope.
                // The constant here was arrived at experimentally, and is otherwise
                // fairly arbitrary.
                const FRAG_MIN_BYTES: usize = (MAX_BYTES * MIN_CHILDREN) + (MAX_BYTES / 32);
                if children.is_full()
                    && children.nodes()[0].is_leaf()
                    && (children.combined_info().bytes as usize) < FRAG_MIN_BYTES
                {
                    children.compact_leaves();
                }

                // Find the child we care about.
                let (child_i, acc_char_idx) = children.search_char_idx_only(char_idx);
                let info = children.info()[child_i];

                // Recurse into the child.
                let (l_info, residual) = Arc::make_mut(&mut children.nodes_mut()[child_i])
                    .edit_chunk_at_char(char_idx - acc_char_idx, info, edit);
                children.info_mut()[child_i] = l_info;

                // Handle the residual node if there is one and return.
                if let Some((r_info, r_node)) = residual {
                    if children.len() < MAX_CHILDREN {
                        children.insert(child_i + 1, (r_info, r_node));
                        (node_info - info + l_info + r_info, None)
                    } else {
                        let r = children.insert_split(child_i + 1, (r_info, r_node));
                        let r_info = r.combined_info();
                        (
                            children.combined_info(),
                            Some((r_info, Arc::new(Node::Internal(r)))),
                        )
                    }
                } else {
                    (node_info - info + l_info, None)
                }
            }
        }
    }

    /// Removes chars in the range `start_idx..end_idx`.
    ///
    /// Returns (in this order):
    /// - The updated TextInfo for the node.
    /// - Whether there's a possible CRLF seam that needs fixing.
    /// - Whether fix_tree_seam() needs to be run after this.
    ///
    /// WARNING: does not correctly handle all text being removed.  That
    /// should be special-cased in calling code.
    pub fn remove_char_range(
        &mut self,
        start_idx: usize,
        end_idx: usize,
        node_info: TextInfo,
    ) -> (TextInfo, bool, bool) {
        if start_idx == end_idx {
            return (node_info, false, false);
        }

        match *self {
            // If it's a leaf
            Node::Leaf(ref mut leaf_text) => {
                let byte_start = char_to_byte_idx(leaf_text, start_idx);
                let byte_end =
                    byte_start + char_to_byte_idx(&leaf_text[byte_start..], end_idx - start_idx);

                // Remove text and calculate new info & seam info
                if byte_start > 0 || byte_end < leaf_text.len() {
                    let seam = (byte_start == 0 && leaf_text.as_bytes()[byte_end] == 0x0A)
                        || (byte_end == leaf_text.len()
                            && leaf_text.as_bytes()[byte_start - 1] == 0x0D);

                    let seg_len = byte_end - byte_start; // Length of removal segement
                    if seg_len < (leaf_text.len() - seg_len) {
                        #[allow(unused_mut)]
                        let mut info =
                            node_info - TextInfo::from_str(&leaf_text[byte_start..byte_end]);

                        // Check for CRLF pairs on the removal seams, and
                        // adjust line break counts accordingly.
                        #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
                        {
                            if byte_end < leaf_text.len()
                                && leaf_text.as_bytes()[byte_end - 1] == 0x0D
                                && leaf_text.as_bytes()[byte_end] == 0x0A
                            {
                                info.line_breaks += 1;
                            }
                            if byte_start > 0 && leaf_text.as_bytes()[byte_start - 1] == 0x0D {
                                if leaf_text.as_bytes()[byte_start] == 0x0A {
                                    info.line_breaks += 1;
                                }
                                if byte_end < leaf_text.len()
                                    && leaf_text.as_bytes()[byte_end] == 0x0A
                                {
                                    info.line_breaks -= 1;
                                }
                            }
                        }

                        // Remove the text
                        leaf_text.remove_range(byte_start, byte_end);

                        (info, seam, false)
                    } else {
                        // Remove the text
                        leaf_text.remove_range(byte_start, byte_end);

                        (TextInfo::from_str(leaf_text), seam, false)
                    }
                } else {
                    // Remove all of the text
                    leaf_text.remove_range(byte_start, byte_end);

                    (TextInfo::new(), true, false)
                }
            }

            // If it's internal, it's much more complicated
            Node::Internal(ref mut children) => {
                // Shared code for handling children.
                // Returns (in this order):
                // - Whether there's a possible CRLF seam that needs fixing.
                // - Whether the tree may need invariant fixing.
                // - Updated TextInfo of the node.
                let handle_child = |children: &mut NodeChildren,
                                    child_i: usize,
                                    c_char_acc: usize|
                 -> (bool, bool, TextInfo) {
                    // Recurse into child
                    let tmp_info = children.info()[child_i];
                    let tmp_chars = children.info()[child_i].chars as usize;
                    let (new_info, seam, needs_fix) =
                        Arc::make_mut(&mut children.nodes_mut()[child_i]).remove_char_range(
                            start_idx - c_char_acc.min(start_idx),
                            (end_idx - c_char_acc).min(tmp_chars),
                            tmp_info,
                        );

                    // Handle result
                    if new_info.bytes == 0 {
                        children.remove(child_i);
                    } else {
                        children.info_mut()[child_i] = new_info;
                    }

                    (seam, needs_fix, new_info)
                };

                // Shared code for merging children
                let merge_child = |children: &mut NodeChildren, child_i: usize| {
                    if child_i < children.len()
                        && children.len() > 1
                        && children.nodes()[child_i].is_undersized()
                    {
                        if child_i == 0 {
                            children.merge_distribute(child_i, child_i + 1);
                        } else {
                            children.merge_distribute(child_i - 1, child_i);
                        }
                    }
                };

                // Get child info for the two char indices
                let ((l_child_i, l_char_acc), (r_child_i, r_char_acc)) =
                    children.search_char_idx_range(start_idx, end_idx);

                // Both indices point into the same child
                if l_child_i == r_child_i {
                    let info = children.info()[l_child_i];
                    let (seam, mut needs_fix, new_info) =
                        handle_child(children, l_child_i, l_char_acc);

                    if children.len() > 0 {
                        merge_child(children, l_child_i);

                        // If we couldn't get all children >= minimum size, then
                        // we'll need to fix that later.
                        if children.nodes()[l_child_i.min(children.len() - 1)].is_undersized() {
                            needs_fix = true;
                        }
                    }

                    return (node_info - info + new_info, seam, needs_fix);
                }
                // We're dealing with more than one child.
                else {
                    let mut needs_fix = false;

                    // Calculate the start..end range of nodes to be removed.
                    let r_child_exists: bool;
                    let start_i = l_child_i + 1;
                    let end_i = if r_char_acc + children.info()[r_child_i].chars as usize == end_idx
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

                    // Handle right child
                    if r_child_exists {
                        let (_, fix, _) = handle_child(children, l_child_i + 1, r_char_acc);
                        needs_fix |= fix;
                    }

                    // Handle left child
                    let (seam, fix, _) = handle_child(children, l_child_i, l_char_acc);
                    needs_fix |= fix;

                    if children.len() > 0 {
                        // Handle merging
                        let merge_extent = 1 + if r_child_exists { 1 } else { 0 };
                        for i in (l_child_i..(l_child_i + merge_extent)).rev() {
                            merge_child(children, i);
                        }

                        // If we couldn't get all children >= minimum size, then
                        // we'll need to fix that later.
                        if children.nodes()[l_child_i.min(children.len() - 1)].is_undersized() {
                            needs_fix = true;
                        }
                    }

                    // Return
                    return (children.combined_info(), seam, needs_fix);
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
                let byte_idx = char_to_byte_idx(text, char_idx);
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

    /// Returns the chunk that contains the given byte, and the TextInfo
    /// corresponding to the start of the chunk.
    pub fn get_chunk_at_byte(&self, byte_idx: usize) -> (&str, TextInfo) {
        let mut node = self;
        let mut byte_idx = byte_idx;
        let mut info = TextInfo::new();

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (text, info);
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

    /// Returns the chunk that contains the given char, and the TextInfo
    /// corresponding to the start of the chunk.
    pub fn get_chunk_at_char(&self, char_idx: usize) -> (&str, TextInfo) {
        let mut node = self;
        let mut char_idx = char_idx;
        let mut info = TextInfo::new();

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (text, info);
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

    /// Returns the chunk that contains the given utf16 code unit, and the
    /// TextInfo corresponding to the start of the chunk.
    pub fn get_chunk_at_utf16_code_unit(&self, utf16_idx: usize) -> (&str, TextInfo) {
        let mut node = self;
        let mut utf16_idx = utf16_idx;
        let mut info = TextInfo::new();

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (text, info);
                }
                Node::Internal(ref children) => {
                    let (child_i, acc_info) = children.search_utf16_code_unit_idx(utf16_idx);
                    info += acc_info;
                    node = &*children.nodes()[child_i];
                    utf16_idx -= (acc_info.chars + acc_info.utf16_surrogates) as usize;
                }
            }
        }
    }

    /// Returns the chunk that contains the given line break, and the TextInfo
    /// corresponding to the start of the chunk.
    ///
    /// Note: for convenience, both the beginning and end of the rope are
    /// considered line breaks for indexing.
    pub fn get_chunk_at_line_break(&self, line_break_idx: usize) -> (&str, TextInfo) {
        let mut node = self;
        let mut line_break_idx = line_break_idx;
        let mut info = TextInfo::new();

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (text, info);
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

    /// Returns the TextInfo at the given char index.
    #[inline(always)]
    pub fn char_to_text_info(&self, char_idx: usize) -> TextInfo {
        let (chunk, info) = self.get_chunk_at_char(char_idx);
        let bi = char_to_byte_idx(chunk, char_idx - info.chars as usize);
        TextInfo {
            bytes: info.bytes + bi as Count,
            chars: char_idx as Count,
            utf16_surrogates: info.utf16_surrogates
                + byte_to_utf16_surrogate_idx(chunk, bi) as Count,
            line_breaks: info.line_breaks + byte_to_line_idx(chunk, bi) as Count,
        }
    }

    /// Returns the TextInfo at the given byte index.
    #[inline(always)]
    pub fn byte_to_text_info(&self, byte_idx: usize) -> TextInfo {
        let (chunk, info) = self.get_chunk_at_byte(byte_idx);
        let bi = byte_idx - info.bytes as usize;
        let ci = byte_to_char_idx(chunk, byte_idx - info.bytes as usize);
        TextInfo {
            bytes: byte_idx as Count,
            chars: info.chars + ci as Count,
            utf16_surrogates: info.utf16_surrogates
                + byte_to_utf16_surrogate_idx(chunk, bi) as Count,
            line_breaks: info.line_breaks + byte_to_line_idx(chunk, bi) as Count,
        }
    }

    pub fn text_info(&self) -> TextInfo {
        match *self {
            Node::Leaf(ref text) => TextInfo::from_str(text),
            Node::Internal(ref children) => children.combined_info(),
        }
    }

    pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
        let (chunk, info) = self.get_chunk_at_byte(byte_idx);
        chunk.is_char_boundary(byte_idx - info.bytes as usize)
    }

    #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
    pub fn is_crlf_split(&self, char_idx: usize) -> bool {
        let (chunk, info) = self.get_chunk_at_char(char_idx);
        let idx = char_to_byte_idx(chunk, char_idx - info.chars as usize);
        if idx == 0 || idx == chunk.len() {
            false
        } else {
            let chunk = chunk.as_bytes();
            chunk[idx - 1] == 0x0D && chunk[idx] == 0x0A
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

    pub fn children(&self) -> &NodeChildren {
        match *self {
            Node::Internal(ref children) => children,
            _ => panic!(),
        }
    }

    pub fn children_mut(&mut self) -> &mut NodeChildren {
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
            Node::Internal(ref children) => {
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
    /// position in the rope) doesn't split a CRLF pair, and fixes it if it does.
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
    pub fn fix_crlf_seam(&mut self, byte_pos: Count, must_be_boundary: bool) {
        if let Node::Internal(ref mut children) = *self {
            if byte_pos == 0 {
                // Special-case 1
                Arc::make_mut(&mut children.nodes_mut()[0])
                    .fix_crlf_seam(byte_pos, must_be_boundary);
            } else if byte_pos == children.combined_info().bytes {
                // Special-case 2
                let (info, nodes) = children.data_mut();
                Arc::make_mut(nodes.last_mut().unwrap())
                    .fix_crlf_seam(info.last().unwrap().bytes, must_be_boundary);
            } else {
                // Find the child to navigate into
                let (child_i, start_info) = children.search_byte_idx(byte_pos as usize);
                let start_byte = start_info.bytes;

                let pos_in_child = byte_pos - start_byte;
                let child_len = children.info()[child_i].bytes;

                if pos_in_child == 0 || pos_in_child == child_len {
                    // Left or right edge, get neighbor and fix seam
                    let l_child_i = if pos_in_child == 0 {
                        debug_assert!(child_i != 0);
                        child_i - 1
                    } else {
                        debug_assert!(child_i < children.len());
                        child_i
                    };

                    // Scope for borrow
                    {
                        // Fetch the two children
                        let (l_child, r_child) = children.get_two_mut(l_child_i, l_child_i + 1);
                        let l_child_bytes = l_child.0.bytes;
                        let l_child = Arc::make_mut(l_child.1);
                        let r_child = Arc::make_mut(r_child.1);

                        // Get the text of the two children and fix
                        // the seam between them.
                        // Scope for borrow.
                        {
                            let (l_text, l_offset) =
                                l_child.get_chunk_at_byte_mut(l_child_bytes as usize);
                            let (r_text, r_offset) = r_child.get_chunk_at_byte_mut(0);
                            if must_be_boundary {
                                assert!(l_offset == 0 || l_offset == l_text.len());
                                assert!(r_offset == 0 || r_offset == r_text.len());
                            }
                            fix_segment_seam(l_text, r_text);
                        }

                        // Fix up the children's metadata after the change
                        // to their text.
                        l_child.fix_info_right();
                        r_child.fix_info_left();
                    }

                    // Fix up this node's metadata for those
                    // two children.
                    children.update_child_info(l_child_i);
                    children.update_child_info(l_child_i + 1);

                    // Remove the children if empty.
                    if children.info()[l_child_i + 1].bytes == 0 {
                        children.remove(l_child_i + 1);
                    } else if children.info()[l_child_i].bytes == 0 {
                        children.remove(l_child_i);
                    }
                } else {
                    // Internal to child
                    Arc::make_mut(&mut children.nodes_mut()[child_i])
                        .fix_crlf_seam(pos_in_child, must_be_boundary);

                    children.update_child_info(child_i);

                    if children.info()[child_i].bytes == 0 {
                        children.remove(child_i);
                    }
                }
            }
        }
    }

    /// Returns the chunk that contains the given byte, and the offset
    /// of that byte within the chunk.
    pub fn get_chunk_at_byte_mut(&mut self, byte_idx: usize) -> (&mut NodeText, usize) {
        match *self {
            Node::Leaf(ref mut text) => return (text, byte_idx),
            Node::Internal(ref mut children) => {
                let (child_i, acc_info) = children.search_byte_idx(byte_idx);
                Arc::make_mut(&mut children.nodes_mut()[child_i])
                    .get_chunk_at_byte_mut(byte_idx - acc_info.bytes as usize)
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
                let do_merge = (children.len() > 1)
                    && match *children.nodes()[0] {
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
                let do_merge = (children.len() > 1)
                    && match *children.nodes()[last_i] {
                        Node::Leaf(ref text) => text.len() < MIN_BYTES,
                        Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                    };

                if do_merge {
                    did_stuff |= children.merge_distribute(last_i - 1, last_i);
                }

                if !Arc::make_mut(children.nodes_mut().last_mut().unwrap()).zip_fix_right() {
                    break;
                }
            }
            did_stuff
        } else {
            false
        }
    }

    /// Fixes up the tree after remove_char_range() or Rope::append().
    ///
    /// Takes the char index of the start of the removal range.
    ///
    /// Returns whether it did anything or not that would affect the
    /// parent. True: did stuff, false: didn't do stuff
    pub fn fix_tree_seam(&mut self, char_idx: usize) -> bool {
        if let Node::Internal(ref mut children) = *self {
            let mut did_stuff = false;
            loop {
                // Do merging
                if children.len() > 1 {
                    let (child_i, start_info) = children.search_char_idx(char_idx);
                    let mut do_merge = match *children.nodes()[child_i] {
                        Node::Leaf(ref text) => text.len() < MIN_BYTES,
                        Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                    };

                    if child_i == 0 {
                        if do_merge {
                            did_stuff |= children.merge_distribute(0, 1);
                        }
                    } else {
                        do_merge = do_merge
                            || (start_info.chars as usize == char_idx
                                && match *children.nodes()[child_i - 1] {
                                    Node::Leaf(ref text) => text.len() < MIN_BYTES,
                                    Node::Internal(ref children2) => children2.len() < MIN_CHILDREN,
                                });
                        if do_merge {
                            let res = children.merge_distribute(child_i - 1, child_i);
                            did_stuff |= res
                        }
                    }
                }

                // Do recursion
                let (child_i, start_info) = children.search_char_idx(char_idx);

                if start_info.chars as usize == char_idx && child_i != 0 {
                    let tmp = children.info()[child_i - 1].chars as usize;
                    let effect_1 =
                        Arc::make_mut(&mut children.nodes_mut()[child_i - 1]).fix_tree_seam(tmp);
                    let effect_2 =
                        Arc::make_mut(&mut children.nodes_mut()[child_i]).fix_tree_seam(0);
                    if (!effect_1) && (!effect_2) {
                        break;
                    }
                } else if !Arc::make_mut(&mut children.nodes_mut()[child_i])
                    .fix_tree_seam(char_idx - start_info.chars as usize)
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
    use crate::Rope;

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
        use crate::tree::{NodeChildren, NodeText, MAX_BYTES};
        use std::sync::Arc;

        // Construct the corner case
        let nodel = Node::Leaf(NodeText::from_str(&"\n".repeat(MAX_BYTES - 1)));
        let noder = Node::Leaf(NodeText::from_str(&"\n".repeat(MAX_BYTES)));
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
        use crate::tree::{NodeChildren, NodeText, MAX_BYTES};
        use std::sync::Arc;

        // Construct the corner case
        let nodel = Node::Leaf(NodeText::from_str(&"\r".repeat(MAX_BYTES)));
        let noder = Node::Leaf(NodeText::from_str(&"\r".repeat(MAX_BYTES - 1)));
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
