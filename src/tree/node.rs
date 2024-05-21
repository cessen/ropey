use std::sync::Arc;

use crate::{str_utils, Error::*, Result};

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
use crate::LineType;

use super::{Children, Text, TextInfo, MAX_CHILDREN, MAX_TEXT_SIZE, MIN_CHILDREN, MIN_TEXT_SIZE};

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Internal(Arc<Children>),
    Leaf(Arc<Text>),
}

impl Node {
    /// Shallowly computes the text info of this node.
    ///
    /// Assumes that the info of this node's children is up to date.
    pub(crate) fn text_info(&self) -> TextInfo {
        match *self {
            Node::Internal(ref children) => {
                let mut acc_info = TextInfo::new();
                for info in children.info() {
                    acc_info = acc_info.concat(*info);
                }
                acc_info
            }
            Node::Leaf(ref text) => text.text_info(),
        }
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        match *self {
            Self::Internal(ref children) => children.nodes().is_empty(),
            Self::Leaf(ref text) => text.len() == 0,
        }
    }

    #[inline(always)]
    pub(crate) fn is_internal(&self) -> bool {
        match *self {
            Self::Internal(_) => true,
            Self::Leaf(_) => false,
        }
    }

    #[inline(always)]
    pub(crate) fn is_leaf(&self) -> bool {
        match *self {
            Self::Internal(_) => false,
            Self::Leaf(_) => true,
        }
    }

    #[inline(always)]
    pub fn is_directly_unbalanced(&self) -> bool {
        match *self {
            Node::Leaf(ref text) => text.len() < MIN_TEXT_SIZE,
            Node::Internal(ref children) => children.len() < MIN_CHILDREN,
        }
    }

    #[inline(always)]
    pub fn is_subtree_unbalanced(&self) -> bool {
        match *self {
            Node::Leaf(_) => false,
            Node::Internal(ref children) => children.is_any_unbalanced(),
        }
    }

    pub fn child_count(&self) -> usize {
        self.children().len()
    }

    pub fn children(&self) -> &Children {
        match *self {
            Node::Internal(ref children) => children,
            _ => panic!(),
        }
    }

    pub fn children_mut(&mut self) -> &mut Children {
        match *self {
            Node::Internal(ref mut children) => Arc::make_mut(children),
            _ => panic!(),
        }
    }

    pub fn leaf_text(&self) -> &Text {
        match *self {
            Node::Leaf(ref text) => text,
            _ => panic!(),
        }
    }

    /// Note: `node_info` is the text info *for the node this is being called
    /// on*.  This is because node info for a child is stored in the parent.
    /// This makes it a little inconvenient to call, but is desireable for
    /// efficiency so that the info can be used for a cheaper update rather than
    /// being recomputed from scratch.
    ///
    ///
    /// On success, returns the new text info for the current node, and if a
    /// split was caused returns the right side of the split (the left remaining
    /// as the current node) and its text info.
    ///
    /// On failure, returns `Err(Error)`.
    ///
    /// Panics only if `text` is too large to handle.  Anything less than or
    /// equal to `MAX_TEXT_SIZE - 4` is guaranteed to be okay.
    pub fn insert_at_byte_idx(
        &mut self,
        byte_idx: usize,
        text: &str,
        node_info: TextInfo,
    ) -> Result<(TextInfo, Option<(TextInfo, Node)>)> {
        // TODO: use `node_info` to do an update of the node info rather
        // than recomputing from scratch.  This will be a bit delicate,
        // because it requires being aware of crlf splits.

        debug_assert!(text.len() <= (MAX_TEXT_SIZE - 4));

        match *self {
            Node::Leaf(ref mut leaf_text) => {
                debug_assert!(byte_idx <= leaf_text.len());

                if !leaf_text.is_char_boundary(byte_idx) {
                    return Err(NonCharBoundary);
                }

                let leaf_text = Arc::make_mut(leaf_text);
                if text.len() <= leaf_text.free_capacity() {
                    // Enough room to insert.
                    let new_info = leaf_text.insert_str_and_update_info(byte_idx, text, node_info);
                    Ok((new_info, None))
                } else {
                    // Not enough room to insert.  Need to split into two nodes.
                    let mut right_text = leaf_text.split(byte_idx);
                    let text_split_idx =
                        crate::find_char_boundary_l(leaf_text.free_capacity(), text.as_bytes());
                    leaf_text.append_str(&text[..text_split_idx]);
                    right_text.insert_str(0, &text[text_split_idx..]);
                    leaf_text.distribute(&mut right_text);
                    Ok((
                        leaf_text.text_info(),
                        Some((right_text.text_info(), Node::Leaf(Arc::new(right_text)))),
                    ))
                }
            }
            Node::Internal(ref mut children) => {
                let children = Arc::make_mut(children);

                // Find the child we care about.
                let (child_i, acc_byte_idx) = children.search_byte_idx_only(byte_idx);
                let info = children.info()[child_i];

                // Recurse into the child.
                let (l_info, residual) = children.nodes_mut()[child_i].insert_at_byte_idx(
                    byte_idx - acc_byte_idx,
                    text,
                    info,
                )?;
                children.info_mut()[child_i] = l_info;
                children.update_unbalance_flag(child_i);

                // Handle the residual node if there is one and return.
                if let Some((r_info, r_node)) = residual {
                    if children.len() < MAX_CHILDREN {
                        children.insert(child_i + 1, (r_info, r_node));
                        Ok((children.combined_text_info(), None))
                    } else {
                        let r = children.insert_split(child_i + 1, (r_info, r_node));
                        let r_info = r.combined_text_info();
                        Ok((
                            children.combined_text_info(),
                            Some((r_info, Node::Internal(Arc::new(r)))),
                        ))
                    }
                } else {
                    Ok((children.combined_text_info(), None))
                }
            }
        }
    }

    pub fn remove_byte_range(
        &mut self,
        byte_idx_range: [usize; 2],
        _node_info: TextInfo,
    ) -> Result<TextInfo> {
        // TODO: use `node_info` to do an update of the node info rather
        // than recomputing from scratch.  This will be a bit delicate,
        // because it requires being aware of crlf splits.

        debug_assert!(byte_idx_range[0] <= byte_idx_range[1]);

        match *self {
            Node::Leaf(ref mut leaf_text) => {
                debug_assert!(byte_idx_range[0] > 0 || byte_idx_range[1] < leaf_text.len());

                if byte_idx_range
                    .iter()
                    .any(|&i| !leaf_text.is_char_boundary(i))
                {
                    return Err(NonCharBoundary);
                }

                let leaf_text = Arc::make_mut(leaf_text);
                leaf_text.remove(byte_idx_range);

                Ok(leaf_text.text_info())
            }
            Node::Internal(ref mut children) => {
                let children = Arc::make_mut(children);

                // Find the start and end children of the range, and
                // their left-side byte indices within this node.
                let (start_child_i, start_child_left_byte_idx) =
                    children.search_byte_idx_only(byte_idx_range[0]);
                let (end_child_i, end_child_left_byte_idx) =
                    children.search_byte_idx_only(byte_idx_range[1]);

                // Text info of the the start and end children.
                let start_info = children.info()[start_child_i];
                let end_info = children.info()[end_child_i];

                // Compute the start index relative to the contents of the
                // start child, and the end index relative to the contents
                // of the end child.
                let start_byte_idx = byte_idx_range[0] - start_child_left_byte_idx;
                let end_byte_idx = byte_idx_range[1] - end_child_left_byte_idx;

                // Simple case: the removal is entirely within a single child.
                if start_child_i == end_child_i {
                    if start_byte_idx == 0 && end_byte_idx == start_info.bytes {
                        children.remove(start_child_i);
                    } else {
                        let new_info = children.nodes_mut()[start_child_i]
                            .remove_byte_range([start_byte_idx, end_byte_idx], start_info)?;
                        children.info_mut()[start_child_i] = new_info;
                        children.update_unbalance_flag(start_child_i);
                    }
                }
                // More complex case: the removal spans multiple children.
                else {
                    let remove_whole_start_child = start_byte_idx == 0;
                    let remove_whole_end_child = end_byte_idx == children.info()[end_child_i].bytes;

                    // Handle partial removal of leftmost child.
                    if !remove_whole_start_child {
                        let new_info = children.nodes_mut()[start_child_i]
                            .remove_byte_range([start_byte_idx, start_info.bytes], start_info)?;
                        children.info_mut()[start_child_i] = new_info;
                        children.update_unbalance_flag(start_child_i);
                    }

                    // Handle partial removal of rightmost child.
                    if !remove_whole_end_child {
                        let new_info = children.nodes_mut()[end_child_i]
                            .remove_byte_range([0, end_byte_idx], end_info)?;
                        children.info_mut()[end_child_i] = new_info;
                        children.update_unbalance_flag(end_child_i);
                    }

                    // Remove nodes that need to be completely removed.
                    let removal_start = if remove_whole_start_child {
                        start_child_i
                    } else {
                        start_child_i + 1
                    };
                    let removal_end = if remove_whole_end_child {
                        end_child_i + 1
                    } else {
                        end_child_i
                    };
                    if removal_start < removal_end {
                        children.remove_multiple([removal_start, removal_end]);
                    }
                }

                Ok(children.combined_text_info())
            }
        }
    }

    pub fn partial_rebalance(&mut self) {
        match *self {
            Node::Leaf(_) => {}

            Node::Internal(ref mut children) => {
                if let Some(child_i) = children.first_unbalanced_child_idx() {
                    let children = Arc::make_mut(children);

                    // First: dive deep.
                    if children.nodes()[child_i].is_subtree_unbalanced() {
                        children.nodes_mut()[child_i].partial_rebalance();
                        children.update_unbalance_flag(child_i);
                    }

                    // Then: do a rebalance at this level if needed.
                    if children.nodes()[child_i].is_directly_unbalanced() && children.len() > 1 {
                        if child_i < (children.len() - 1) {
                            children.merge_distribute(child_i, child_i + 1);
                        } else {
                            children.merge_distribute(child_i - 1, child_i);
                        }
                    }
                }
            }
        }
    }

    //---------------------------------------------------------
    // `Text` fetching.

    /// The internal implementation of `get_text_at_*()` further below.
    ///
    /// Returns the `Text` that contains the given index of the specified
    /// metric, and the left-side info of where it starts in the larger text of
    /// the node tree.
    ///
    /// - `metric_scanner`: a function that scans `Children` to find the
    ///   child that contains `metric_idx`, returning the child's index and
    ///   it's left-side accumulated text info within its sublings. See
    ///   `Children::search_*_idx()` for methods that do exactly this for
    ///   various metrics.  Note that the returned TextInfo should already
    ///   have split-CRLF compensation applied.
    /// - `metric_subtractor`: a simple function that subtracts the relevant
    ///   metric in a TextInfo from a usize.
    #[inline(always)]
    fn get_text_at_metric<F1, F2>(
        &self,
        metric_idx: usize,
        metric_scanner: F1,
        metric_subtractor: F2,
    ) -> (&Text, TextInfo)
    where
        F1: Fn(&Children, usize) -> (usize, TextInfo),
        F2: Fn(usize, &TextInfo) -> usize,
    {
        let mut node = self;
        let mut metric_idx = metric_idx;
        let mut left_info = TextInfo::new();

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (text, left_info);
                }
                Node::Internal(ref children) => {
                    let (child_i, acc_info) = metric_scanner(children, metric_idx);
                    left_info = left_info.concat(acc_info);
                    node = &children.nodes()[child_i];
                    metric_idx = metric_subtractor(metric_idx, &acc_info);
                }
            }
        }
    }

    /// Returns the `Text` that contains the given byte.
    ///
    /// See `get_text_at_metric()` for further documentation.
    pub fn get_text_at_byte(&self, byte_idx: usize) -> (&Text, TextInfo) {
        self.get_text_at_metric(
            byte_idx,
            |children, idx| children.search_byte_idx(idx),
            |idx, traversed_info| idx - traversed_info.bytes,
        )
    }

    /// Like `get_text_at_byte()` but only computes and returns byte
    /// info, not full text info.  It is also faster for that reason.
    ///
    /// Returns the text itself and the byte offset of the start of its left
    /// edge in the context of the whole text of the node tree.
    pub fn get_text_at_byte_fast(&self, byte_idx: usize) -> (&Text, usize) {
        let mut idx = byte_idx;
        let mut node = self;

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (text, byte_idx - idx);
                }
                Node::Internal(ref children) => {
                    let (child_i, byte_idx_offset) = children.search_byte_idx_only(idx);
                    node = &children.nodes()[child_i];
                    idx -= byte_idx_offset;
                }
            }
        }
    }

    /// Returns the `Text` that contains the given char.
    ///
    /// See `get_text_at_metric()` for further documentation.
    #[cfg(feature = "metric_chars")]
    pub fn get_text_at_char(&self, char_idx: usize) -> (&Text, TextInfo) {
        self.get_text_at_metric(
            char_idx,
            |children, idx| children.search_char_idx(idx),
            |idx, traversed_info| idx - traversed_info.chars,
        )
    }

    /// Returns the `Text` that contains the given utf16 code unit.
    ///
    /// See `get_text_at_metric()` for further documentation.
    #[cfg(feature = "metric_utf16")]
    pub fn get_text_at_utf16(&self, utf16_idx: usize) -> (&Text, TextInfo) {
        self.get_text_at_metric(
            utf16_idx,
            |children, idx| children.search_utf16_code_unit_idx(idx),
            |idx, traversed_info| idx - traversed_info.utf16,
        )
    }

    /// Returns the `Text` that contains the given line break.
    ///
    /// See `get_text_at_metric()` for further documentation.
    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    pub fn get_text_at_line_break(
        &self,
        line_break_idx: usize,
        line_type: LineType,
    ) -> (&Text, TextInfo) {
        self.get_text_at_metric(
            line_break_idx,
            |children, idx| children.search_line_break_idx(idx, line_type),
            |idx, traversed_info| idx - traversed_info.line_breaks(line_type),
        )
    }

    //---------------------------------------------------------
    // Misc.

    pub fn text_info_at_byte(&self, byte_idx: usize) -> TextInfo {
        let (text, left_info) = self.get_text_at_byte(byte_idx);

        let internal_byte_idx = byte_idx - left_info.bytes;
        left_info
            + TextInfo::from_str(&text.text()[..internal_byte_idx])
                .adjusted_by_next_is_lf(str_utils::byte_is_lf(text.text(), internal_byte_idx))
    }

    pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
        let (text, offset) = self.get_text_at_byte_fast(byte_idx);
        text.is_char_boundary(byte_idx - offset)
    }

    /// Returns whether splitting at `byte_idx` would split a CRLF pair, if such
    /// a split would be relevant to the line-counting metrics of `line_type`.
    ///
    /// Specifically, CRLF pairs are not relevant to LF-only line metrics, so
    /// for that line type this will always return false.  Otherwise it will
    /// return if a CRLF pair would be split.
    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    pub(crate) fn is_relevant_crlf_split(&self, byte_idx: usize, line_type: LineType) -> bool {
        // Silence unused parameter warning when relevant features are disabled.
        let _ = byte_idx;

        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => false,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            _ => {
                let (text, start_info) = self.get_text_at_byte(byte_idx);
                let idx = byte_idx - start_info.bytes;

                if idx == 0 {
                    start_info.ends_with_split_crlf(crate::str_utils::starts_with_lf(text.text()))
                } else {
                    str_utils::ends_with_cr(&text.text()[..idx])
                        && str_utils::starts_with_lf(&text.text()[idx..])
                }
            }
        }
    }

    //---------------------------------------------------------
    // Debugging/testing helpers.

    /// Checks that all leaf nodes are at the same depth.
    pub fn assert_equal_leaf_depth(&self) -> usize {
        match *self {
            Node::Leaf(_) => 1,
            Node::Internal(ref children) => {
                let first_depth = children.nodes()[0].assert_equal_leaf_depth();
                for node in &children.nodes()[1..] {
                    assert_eq!(node.assert_equal_leaf_depth(), first_depth);
                }
                first_depth + 1
            }
        }
    }

    /// Checks that there are no empty internal nodes in the tree.
    pub fn assert_no_empty_internal(&self) {
        match *self {
            Node::Leaf(_) => {}
            Node::Internal(ref children) => {
                assert!(children.len() > 0);
                for node in children.nodes() {
                    node.assert_no_empty_internal();
                }
            }
        }
    }

    /// Checks that there are no empty internal nodes in the tree.
    pub fn assert_no_empty_leaf(&self) {
        match *self {
            Node::Leaf(ref text) => {
                assert!(text.len() > 0);
            }
            Node::Internal(ref children) => {
                for node in children.nodes() {
                    node.assert_no_empty_leaf();
                }
            }
        }
    }

    /// Checks that all cached TextInfo in the tree is correct.
    pub fn assert_accurate_text_info(&self) -> TextInfo {
        match *self {
            Node::Leaf(ref text) => {
                // Freshly compute the relevant info from scratch.
                TextInfo::from_str(text.text())
            }
            Node::Internal(ref children) => {
                let mut acc_info = TextInfo::new();
                for (node, &info) in children.nodes().iter().zip(children.info().iter()) {
                    assert_eq!(info, node.assert_accurate_text_info());
                    acc_info = acc_info.concat(info);
                }

                acc_info
            }
        }
    }

    /// Checks that all the unbalance flags in the tree are correct.
    ///
    /// Note: the return value is not "success", it's used in the recursion.
    pub fn assert_accurate_unbalance_flags(&self) -> bool {
        match *self {
            Node::Leaf(ref text) => {
                // Freshly compute whether the leaf is undersized.
                text.len() < MIN_TEXT_SIZE
            }
            Node::Internal(ref children) => {
                let mut any_unbalanced = false;
                for i in 0..children.len() {
                    let unbalanced = children.nodes()[i].assert_accurate_unbalance_flags()
                        || children.nodes()[i].is_directly_unbalanced();
                    assert_eq!(unbalanced, children.is_node_unbalanced(i));
                    any_unbalanced |= unbalanced;
                }

                any_unbalanced
            }
        }
    }
}
