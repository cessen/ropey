use std::sync::Arc;

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
use crate::LineType;

use super::{Children, Text, TextInfo, MAX_CHILDREN};

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
        match &self {
            Node::Internal(children) => {
                let mut acc_info = TextInfo::new();
                for info in children.info() {
                    acc_info = acc_info.concat(*info);
                }
                acc_info
            }
            Node::Leaf(text) => text.text_info(),
        }
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            &Self::Internal(ref children) => children.nodes().is_empty(),
            &Self::Leaf(ref text) => text.len() == 0,
        }
    }

    #[inline(always)]
    pub(crate) fn is_internal(&self) -> bool {
        match self {
            &Self::Internal(_) => true,
            &Self::Leaf(_) => false,
        }
    }

    #[inline(always)]
    pub(crate) fn is_leaf(&self) -> bool {
        match self {
            &Self::Internal(_) => false,
            &Self::Leaf(_) => true,
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

    pub fn leaf_text_mut(&mut self) -> &mut Text {
        match *self {
            Node::Leaf(ref mut text) => Arc::make_mut(text),
            _ => panic!(),
        }
    }

    pub fn leaf_text_chunk(&self) -> &str {
        match *self {
            Node::Leaf(ref text) => text.text(),
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
    /// On non-panicing failure, returns Err(()).  This happens if and only if
    /// `byte_idx` is not on a char boundary.
    ///
    /// Panics:
    /// - If `byte_idx` is out of bounds.
    /// - If `text` is too large to handle.  Anything less than or equal to
    ///   `MAX_TEXT_SIZE - 4` is guaranteed to be okay.
    pub fn insert_at_byte_idx(
        &mut self,
        byte_idx: usize,
        text: &str,
        node_info: TextInfo,
    ) -> Result<(TextInfo, Option<(TextInfo, Node)>), ()> {
        // TODO: use `node_info` to do an update of the node info rather
        // than recomputing from scratch.  This will be a bit delicate,
        // because it requires being aware of crlf splits.

        match *self {
            Node::Leaf(ref mut leaf_text) => {
                if !leaf_text.is_char_boundary(byte_idx) {
                    // Not a char boundary, so early-out.
                    return Err(());
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
                        crate::find_split_l(leaf_text.free_capacity(), text.as_bytes());
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
    ) -> Result<TextInfo, ()> {
        // TODO: use `node_info` to do an update of the node info rather
        // than recomputing from scratch.  This will be a bit delicate,
        // because it requires being aware of crlf splits.

        match *self {
            Node::Leaf(ref mut leaf_text) => {
                debug_assert!(byte_idx_range[0] > 0 || byte_idx_range[1] < leaf_text.len());
                if byte_idx_range
                    .iter()
                    .any(|&i| !leaf_text.is_char_boundary(i))
                {
                    // Not a char boundary, so early-out.
                    return Err(());
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
                    }

                    // Handle partial removal of rightmost child.
                    if !remove_whole_end_child {
                        let new_info = children.nodes_mut()[end_child_i]
                            .remove_byte_range([0, end_byte_idx], end_info)?;
                        children.info_mut()[end_child_i] = new_info;
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

    //---------------------------------------------------------
    // `Text` fetching.

    /// The internal implementation of `get_text_at_*()` further below.
    ///
    /// Returns the `Text` that contains the given index of the specified
    /// metric.
    ///
    /// - `text_info`: if available, the text info of the node this is being
    ///   called on.  This is just an optimization: if provided, it avoids
    ///   having to recompute the info.
    /// - `metric_scanner`: a function that scans `Children` to find the
    ///   child that contains `metric_idx`, returning the child's index and
    ///   it's left-side accumulated text info within its sublings. See
    ///   `Children::search_*_idx()` for methods that do exactly this for
    ///   various metrics.  Note that the returned TextInfo should already
    ///   have split-CRLF compensation applied.
    /// - `metric_subtractor`: a simple function that subtracts the relevant
    ///   metric in a TextInfo from a usize.  This is usually just a simple
    ///   subtraction, but for utf16 is very slightly more involved due to
    ///   the way it's metric is stored in TextInfo.
    ///
    /// Returns `(left_side_info, Text, Text_info)`.  Both left_side_info and
    /// Text_info have already had split-CRLF compensation applied.
    #[inline(always)]
    fn get_text_at_metric<F1, F2>(
        &self,
        metric_idx: usize,
        text_info: Option<TextInfo>,
        metric_scanner: F1,
        metric_subtractor: F2,
    ) -> (TextInfo, &Text, TextInfo)
    where
        F1: Fn(&Children, usize) -> (usize, TextInfo),
        F2: Fn(usize, &TextInfo) -> usize,
    {
        let mut node = self;
        let mut metric_idx = metric_idx;
        let mut left_info = TextInfo::new();
        let mut text_info = if let Some(info) = text_info {
            info
        } else {
            self.text_info()
        };
        let mut next_chunk_starts_with_lf = false;

        loop {
            match *node {
                Node::Leaf(ref text) => {
                    return (
                        left_info,
                        text,
                        text_info.adjusted_by_next_is_lf(next_chunk_starts_with_lf),
                    );
                }
                Node::Internal(ref children) => {
                    let (child_i, acc_info) = metric_scanner(&children, metric_idx);
                    left_info = left_info.concat(acc_info);
                    text_info = children.info()[child_i];

                    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
                    if children.len() > (child_i + 1) {
                        next_chunk_starts_with_lf = children.info()[child_i + 1].starts_with_lf;
                    }

                    node = &children.nodes()[child_i];
                    metric_idx = metric_subtractor(metric_idx, &acc_info);
                }
            }
        }
    }

    /// Returns the `Text` that contains the given byte.
    ///
    /// See `get_text_at_metric()` for further documentation.
    pub fn get_text_at_byte(
        &self,
        byte_idx: usize,
        text_info: Option<TextInfo>,
    ) -> (TextInfo, &Text, TextInfo) {
        self.get_text_at_metric(
            byte_idx,
            text_info,
            |children, idx| children.search_byte_idx(idx),
            |idx, traversed_info| idx - traversed_info.bytes,
        )
    }

    /// Returns the `Text` that contains the given char.
    ///
    /// See `get_text_at_metric()` for further documentation.
    #[cfg(feature = "metric_chars")]
    pub fn get_text_at_char(
        &self,
        char_idx: usize,
        text_info: Option<TextInfo>,
    ) -> (TextInfo, &Text, TextInfo) {
        self.get_text_at_metric(
            char_idx,
            text_info,
            |children, idx| children.search_char_idx(idx),
            |idx, traversed_info| idx - traversed_info.chars,
        )
    }

    /// Returns the `Text` that contains the given utf16 code unit.
    ///
    /// See `get_text_at_metric()` for further documentation.
    #[cfg(feature = "metric_utf16")]
    pub fn get_text_at_utf16(
        &self,
        utf16_idx: usize,
        text_info: Option<TextInfo>,
    ) -> (TextInfo, &Text, TextInfo) {
        self.get_text_at_metric(
            utf16_idx,
            text_info,
            |children, idx| children.search_utf16_code_unit_idx(idx),
            |idx, traversed_info| idx - (traversed_info.chars + traversed_info.utf16_surrogates),
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
        text_info: Option<TextInfo>,
        line_type: LineType,
    ) -> (TextInfo, &Text, TextInfo) {
        self.get_text_at_metric(
            line_break_idx,
            text_info,
            |children, idx| children.search_line_break_idx(idx, line_type),
            |idx, traversed_info| idx - traversed_info.line_breaks(line_type),
        )
    }

    //---------------------------------------------------------
    // Debugging helpers.

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
}
