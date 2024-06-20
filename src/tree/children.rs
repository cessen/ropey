use std::sync::Arc;

use super::{node::Node, text_info::TextInfo, MAX_CHILDREN, MAX_TEXT_SIZE};

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
use crate::LineType;

/// Internal node of the Rope, with other nodes as children.
#[derive(Debug, Clone)]
pub(crate) struct Children(inner::ChildrenInternal);

impl Children {
    /// Creates a new empty child array.
    #[inline(always)]
    pub fn new() -> Self {
        Self(inner::ChildrenInternal::new())
    }

    /// Current length of the child array.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Access to the nodes array.
    #[inline(always)]
    pub fn nodes(&self) -> &[Node] {
        self.0.nodes()
    }

    /// Mutable access to the nodes array.
    #[inline(always)]
    pub fn nodes_mut(&mut self) -> &mut [Node] {
        self.0.nodes_mut()
    }

    /// Access to the info array.
    #[inline(always)]
    pub fn info(&self) -> &[TextInfo] {
        self.0.info()
    }

    /// Mutable access to the info array.
    #[inline(always)]
    pub fn info_mut(&mut self) -> &mut [TextInfo] {
        self.0.info_mut()
    }

    /// Mutable access to both the info and nodes arrays simultaneously.
    #[inline(always)]
    pub fn data_mut(&mut self) -> (&mut [TextInfo], &mut [Node]) {
        self.0.data_mut()
    }

    /// Pushes an item onto the end of the child array.
    ///
    /// Increases length by one.  Panics if already full.
    #[inline(always)]
    pub fn push(&mut self, item: (TextInfo, Node)) {
        self.0.push(item);
        self.update_unbalance_flag(self.len() - 1);
    }

    /// Pushes an element onto the end of the array, and then splits it in half,
    /// returning the right half.
    ///
    /// This works even when the array is full.
    pub fn push_split(&mut self, new_child: (TextInfo, Node)) -> Self {
        let r_count = (self.len() + 1) / 2;
        let l_count = (self.len() + 1) - r_count;

        let mut right = self.split_off(l_count);
        right.push(new_child);
        right
    }

    /// Merges two nodes together.
    ///
    /// Assumes the two nodes are adjecent to each other, with `idx1`
    /// preceding `idx2`.
    ///
    /// Note: will panic internally if there's too much data to
    /// combine into one node.
    pub fn merge(&mut self, idx1: usize, idx2: usize) {
        debug_assert_eq!(idx1 + 1, idx2);
        debug_assert!(idx2 < self.len());

        let ((info1, node1), (info2, node2)) = self.get_two_mut(idx1, idx2);
        match (node1, node2) {
            (&mut Node::Leaf(ref mut text1), &mut Node::Leaf(ref mut text2)) => {
                let text1 = Arc::make_mut(text1);
                text1.append_str(text2.text());
            }

            (&mut Node::Internal(ref mut children1), &mut Node::Internal(ref mut children2)) => {
                let children1 = Arc::make_mut(children1);
                let children2 = Arc::make_mut(children2);
                let children2_len = children2.len(); // Work around borrow checker.
                children1
                    .0
                    .steal_range_from(children1.len(), &mut children2.0, [0, children2_len]);
            }

            _ => panic!("Can't merge two nodes of different types."),
        }

        *info1 += *info2;
        self.remove(idx2);
        self.update_unbalance_flag(idx1);
    }

    /// Equally distributes the data between two nodes.
    ///
    /// Assumes the two nodes are adjecent to each other, with `idx1`
    /// preceding `idx2`.
    pub fn distribute(&mut self, idx1: usize, idx2: usize) {
        debug_assert_eq!(idx1 + 1, idx2);
        debug_assert!(idx2 < self.len());

        let ((info1, node1), (info2, node2)) = self.get_two_mut(idx1, idx2);
        match (node1, node2) {
            (Node::Leaf(ref mut text1), Node::Leaf(ref mut text2)) => {
                let text1 = Arc::make_mut(text1);
                let text2 = Arc::make_mut(text2);
                text1.distribute(text2);

                *info1 = text1.text_info();
                *info2 = text2.text_info();
            }

            (Node::Internal(ref mut children1), Node::Internal(ref mut children2)) => {
                let lhs = Arc::make_mut(children1);
                let rhs = Arc::make_mut(children2);
                let rhs_target_len = (lhs.len() + rhs.len()) / 2;
                if rhs.len() < rhs_target_len {
                    let start = lhs.len() + rhs.len() - rhs_target_len;
                    let lhs_len = lhs.len(); // Work around borrow checker.
                    rhs.0.steal_range_from(0, &mut lhs.0, [start, lhs_len]);
                } else if rhs.len() > rhs_target_len {
                    let end = rhs.len() - rhs_target_len;
                    lhs.0.steal_range_from(lhs.len(), &mut rhs.0, [0, end]);
                }

                *info1 = lhs.combined_text_info();
                *info2 = rhs.combined_text_info();
            }

            _ => panic!("Can't distribute data between two nodes of different types."),
        }
        self.update_unbalance_flag(idx1);
        self.update_unbalance_flag(idx2);
    }

    /// Attempts to merge two nodes, and if it's too much data to merge
    /// equi-distributes the data between the two.
    ///
    /// Assumes the two nodes are adjecent to each other, with `idx1`
    /// preceding `idx2`.
    ///
    /// Returns:
    ///
    /// - True: merge was successful.
    /// - False: merge failed, equidistributed instead.
    pub fn merge_distribute(&mut self, idx1: usize, idx2: usize) -> bool {
        debug_assert_eq!(idx1 + 1, idx2);
        debug_assert!(idx2 < self.len());

        let do_merge = match (&self.nodes()[idx1], &self.nodes()[idx2]) {
            (Node::Leaf(ref text1), Node::Leaf(ref text2)) => {
                (text1.len() + text2.len()) <= MAX_TEXT_SIZE
            }

            (Node::Internal(ref children1), Node::Internal(ref children2)) => {
                (children1.len() + children2.len()) <= MAX_CHILDREN
            }

            _ => panic!("Siblings have different node types"),
        };

        if do_merge {
            self.merge(idx1, idx2);
            true
        } else {
            self.distribute(idx1, idx2);
            false
        }
    }

    /// Pops an item off the end of the array and returns it.
    ///
    /// Decreases length by one.  Panics if already empty.
    #[inline(always)]
    pub fn pop(&mut self) -> (TextInfo, Node) {
        self.0.pop()
    }

    /// Inserts an item into the the array at the given index.
    ///
    /// Increases length by one.  Panics if already full.  Preserves ordering
    /// of the other items.
    #[inline(always)]
    pub fn insert(&mut self, idx: usize, item: (TextInfo, Node)) {
        self.0.insert(idx, item);
        self.update_unbalance_flag(idx);
    }

    /// Inserts an element into a the array, and then splits it in half, returning
    /// the right half.
    ///
    /// This works even when the array is full.
    pub fn insert_split(&mut self, idx: usize, item: (TextInfo, Node)) -> Self {
        assert!(self.len() > 0);
        assert!(idx <= self.len());
        let extra = if idx < self.len() {
            let extra = self.pop();
            self.insert(idx, item);
            extra
        } else {
            item
        };

        self.push_split(extra)
    }

    /// Removes the item at the given index from the the array.
    ///
    /// Decreases length by one.  Preserves ordering of the other items.
    #[inline(always)]
    pub fn remove(&mut self, idx: usize) -> (TextInfo, Node) {
        self.0.remove(idx)
    }

    /// Removes the items in the given index range (right exclusive).
    ///
    /// Preserves ordering of the remaining items.
    #[inline(always)]
    pub fn remove_multiple(&mut self, idx_range: [usize; 2]) {
        self.0.remove_range(idx_range);
    }

    /// Splits the array in two at `idx`, returning the right part of the split.
    pub fn split_off(&mut self, idx: usize) -> Self {
        assert!(idx <= self.len());

        let mut other = Children::new();
        let self_len = self.len(); // Work around the borrow checker.
        other.0.steal_range_from(0, &mut self.0, [idx, self_len]);

        other
    }

    /// Fetches two children simultaneously, returning mutable references
    /// to their info and nodes.
    ///
    /// `idx1` must be less than `idx2`.
    pub fn get_two_mut(
        &mut self,
        idx1: usize,
        idx2: usize,
    ) -> ((&mut TextInfo, &mut Node), (&mut TextInfo, &mut Node)) {
        assert!(idx1 < idx2);
        assert!(idx2 < self.len());

        let split_idx = idx1 + 1;
        let (info, nodes) = self.data_mut();
        let (info1, info2) = info.split_at_mut(split_idx);
        let (nodes1, nodes2) = nodes.split_at_mut(split_idx);

        (
            (&mut info1[idx1], &mut nodes1[idx1]),
            (&mut info2[idx2 - split_idx], &mut nodes2[idx2 - split_idx]),
        )
    }

    #[inline(always)]
    pub fn is_node_unbalanced(&self, child_idx: usize) -> bool {
        self.0.is_node_unbalanced(child_idx)
    }

    #[inline(always)]
    pub fn is_any_unbalanced(&self) -> bool {
        self.0.is_any_unbalanced()
    }

    #[inline(always)]
    pub fn first_unbalanced_child_idx(&self) -> Option<usize> {
        self.0.first_unbalanced_child_idx()
    }

    #[inline(always)]
    pub fn update_unbalance_flag(&mut self, child_idx: usize) {
        let child = &self.nodes()[child_idx];

        if child.is_subtree_unbalanced() || child.is_directly_unbalanced() {
            self.0.set_unbalance_flag(child_idx);
        } else {
            self.0.clear_unbalance_flag(child_idx);
        }
    }

    #[inline(always)]
    pub fn combined_text_info(&self) -> TextInfo {
        self.info()
            .iter()
            .fold(TextInfo::new(), |acc, &next| acc + next)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// first child that matches the given predicate.
    ///
    /// If no child matches the predicate, the last child is returned.
    ///
    /// The returned TextInfo has already had split-CRLF compensation
    /// applied.
    #[cfg(any(
        feature = "metric_chars",
        feature = "metric_utf16",
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[inline(always)]
    pub fn search_by<F>(&self, pred: F) -> (usize, TextInfo)
    where
        // (left-accumulated start info, left-accumulated end info)
        F: Fn(TextInfo) -> bool,
    {
        debug_assert!(self.len() > 0);

        let mut accum = TextInfo::new();
        let mut idx = 0;
        while idx < (self.len() - 1) {
            let next_accum = accum + self.info()[idx];
            if pred(next_accum) {
                break;
            }
            accum = next_accum;
            idx += 1;
        }

        (idx, accum)
    }

    /// Same as `search_byte_idx()` below, except that it only calculates the
    /// left-side-accumulated _byte_ index rather than the full text info.
    ///
    /// This also has an additional parameter: `bias_left`.  This should be false
    /// if you're looking for the node that actually contains the give byte.
    /// However, if you're searching for *gap* (e.g. when inserting) and want to
    /// traverse into nodes to the left of the boundary when there's a choice,
    /// set it to true.
    ///
    /// Return is (child_index, left_acc_byte_index)
    ///
    /// One-past-the end is valid, and will return the last child.
    ///
    /// The returned TextInfo has already had split-CRLF compensation
    /// applied.
    pub fn search_byte_idx_only(&self, byte_idx: usize, bias_left: bool) -> (usize, usize) {
        debug_assert!(self.len() > 0);

        let mut accum_byte_idx = 0;
        let mut idx = 0;
        for info in self.info()[0..(self.len() - 1)].iter() {
            let next_accum = accum_byte_idx + info.bytes;
            if byte_idx < next_accum || (bias_left && byte_idx == next_accum) {
                break;
            }
            accum_byte_idx = next_accum;
            idx += 1;
        }

        debug_assert!(
            byte_idx <= (accum_byte_idx + self.info()[idx].bytes),
            "Index out of bounds."
        );

        (idx, accum_byte_idx)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// child that contains the given byte.
    ///
    /// One-past-the end is valid, and will return the last child.
    ///
    /// The returned TextInfo has already had split-CRLF compensation
    /// applied.
    #[cfg(any(
        feature = "metric_chars",
        feature = "metric_utf16",
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    pub fn search_byte_idx(&self, byte_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) = self.search_by(|end| byte_idx < end.bytes);

        debug_assert!(
            byte_idx <= (accum.bytes + self.info()[idx].bytes),
            "Index out of bounds."
        );

        (idx, accum)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// child that contains the given char.
    ///
    /// One-past-the end is valid, and will return the last child.
    ///
    /// The returned TextInfo has already had split-CRLF compensation
    /// applied.
    #[cfg(feature = "metric_chars")]
    pub fn search_char_idx(&self, char_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) = self.search_by(|end| char_idx < end.chars);

        debug_assert!(
            char_idx <= (accum.chars + self.info()[idx].chars),
            "Index out of bounds."
        );

        (idx, accum)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// child that contains the given utf16 code unit offset.
    ///
    /// One-past-the end is valid, and will return the last child.
    ///
    /// The returned TextInfo has already had split-CRLF compensation
    /// applied.
    #[cfg(feature = "metric_utf16")]
    pub fn search_utf16_code_unit_idx(&self, utf16_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) = self.search_by(|end| utf16_idx < end.utf16);

        debug_assert!(
            utf16_idx <= (accum.utf16 + self.info()[idx].utf16),
            "Index out of bounds."
        );

        (idx, accum)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// child that contains the given line break.
    ///
    /// Beginning of the rope is considered index 0, although is not
    /// considered a line break for the returned left-side-accumulated
    /// text info.
    ///
    /// One-past-the end is valid, and will return the last child.
    ///
    /// The returned TextInfo has already had split-CRLF compensation
    /// applied.
    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    pub fn search_line_break_idx(
        &self,
        line_break_idx: usize,
        line_type: LineType,
    ) -> (usize, TextInfo) {
        let (idx, accum) = self.search_by(|end| line_break_idx <= end.line_breaks(line_type));

        debug_assert!(
            {
                let end_info = accum + self.info()[idx];
                line_break_idx <= end_info.line_breaks(line_type) + 1
            },
            "Index out of bounds."
        );

        (idx, accum)
    }
}

//===========================================================================

/// The unsafe guts of Children, exposed through a safe API.
///
/// Try to keep this as small as possible, and implement functionality on
/// `Children` via the safe APIs whenever possible.
mod inner {
    use super::{Node, TextInfo, MAX_CHILDREN};
    use std::{
        fmt,
        mem::{self, MaybeUninit},
        ptr,
    };

    #[inline(always)]
    fn range_bitmask(range_low: usize, range_high: usize) -> u32 {
        let a = (!0u32) << range_low;
        let b = (!0u32) << range_high;
        a & !b
    }

    /// This is essentially a fixed-capacity, stack-allocated `Vec`.  However,
    /// it actually containts _two_ arrays rather than just one, but which
    /// share a length.
    #[repr(C)]
    pub(crate) struct ChildrenInternal {
        /// An array of the child nodes.
        /// INVARIANT: The nodes from `0..len` must be initialized
        nodes: [MaybeUninit<Node>; MAX_CHILDREN],
        /// An array of the child node text infos
        /// INVARIANT: The nodes from `0..len` must be initialized
        info: [MaybeUninit<TextInfo>; MAX_CHILDREN],
        subtree_unbalance_flags: u32,
        len: u8,
    }

    impl ChildrenInternal {
        /// Creates a new empty array.
        #[inline(always)]
        pub fn new() -> ChildrenInternal {
            // SAFETY: Uninit data is valid for arrays of MaybeUninit.
            // `len` is zero, so it's ok for all of them to be uninit
            ChildrenInternal {
                nodes: unsafe { MaybeUninit::uninit().assume_init() },
                info: unsafe { MaybeUninit::uninit().assume_init() },
                subtree_unbalance_flags: 0,
                len: 0,
            }
        }

        /// Current length of the array.
        #[inline(always)]
        pub fn len(&self) -> usize {
            self.len as usize
        }

        /// Access to the nodes array.
        #[inline(always)]
        pub fn nodes(&self) -> &[Node] {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the nodes from `0..len` are guaranteed to be initialized
            unsafe { mem::transmute(&self.nodes[..(self.len())]) }
        }

        /// Mutable access to the nodes array.
        #[inline(always)]
        pub fn nodes_mut(&mut self) -> &mut [Node] {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the nodes from `0..len` are guaranteed to be initialized
            unsafe { mem::transmute(&mut self.nodes[..(self.len as usize)]) }
        }

        /// Access to the info array.
        #[inline(always)]
        pub fn info(&self) -> &[TextInfo] {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the info from `0..len` are guaranteed to be initialized
            unsafe { mem::transmute(&self.info[..(self.len())]) }
        }

        /// Mutable access to the info array.
        #[inline(always)]
        pub fn info_mut(&mut self) -> &mut [TextInfo] {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the info from `0..len` are guaranteed to be initialized
            unsafe { mem::transmute(&mut self.info[..(self.len as usize)]) }
        }

        /// Mutable access to both the info and nodes arrays simultaneously.
        #[inline(always)]
        pub fn data_mut(&mut self) -> (&mut [TextInfo], &mut [Node]) {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the info from `0..len` are guaranteed to be initialized
            (
                unsafe { mem::transmute(&mut self.info[..(self.len as usize)]) },
                unsafe { mem::transmute(&mut self.nodes[..(self.len as usize)]) },
            )
        }

        #[inline(always)]
        pub fn is_node_unbalanced(&self, idx: usize) -> bool {
            debug_assert!(idx < self.len());
            self.subtree_unbalance_flags & (1 << idx) != 0
        }

        #[inline(always)]
        pub fn is_any_unbalanced(&self) -> bool {
            self.subtree_unbalance_flags != 0
        }

        #[inline(always)]
        pub fn first_unbalanced_child_idx(&self) -> Option<usize> {
            if self.is_any_unbalanced() {
                Some(self.subtree_unbalance_flags.trailing_zeros() as usize)
            } else {
                None
            }
        }

        #[inline(always)]
        pub fn set_unbalance_flag(&mut self, idx: usize) {
            debug_assert!(idx < self.len());
            self.subtree_unbalance_flags |= 1 << idx;
        }

        #[inline(always)]
        pub fn clear_unbalance_flag(&mut self, idx: usize) {
            debug_assert!(idx < self.len());
            self.subtree_unbalance_flags &= !(1 << idx);
        }

        /// Pushes an item onto the end of the array.
        ///
        /// Increases length by one.  Panics if already full.
        pub fn push(&mut self, item: (TextInfo, Node)) {
            assert!(self.len() < MAX_CHILDREN);
            self.info[self.len()] = MaybeUninit::new(item.0);
            self.nodes[self.len as usize] = MaybeUninit::new(item.1);
            // We have just initialized both `info` and `node` within
            // `0..=len`, so we can increase `len`.
            self.len += 1;
        }

        /// Pops an item off the end of the array and returns it.
        ///
        /// Decreases length by one.  Panics if already empty.
        pub fn pop(&mut self) -> (TextInfo, Node) {
            assert!(self.len() > 0);
            self.len -= 1;

            self.subtree_unbalance_flags &= !(1 << self.len());

            // SAFETY: before this, `len` was long enough to guarantee that
            // both `info` and `node` must be init.  We just decreased the
            // length, guaranteeing that the elements will never be read again.
            (unsafe { self.info[self.len()].assume_init() }, unsafe {
                ptr::read(&self.nodes[self.len()]).assume_init()
            })
        }

        /// Inserts an item into the the array at the given index.
        ///
        /// Increases length by one.  Panics if already full.  Preserves ordering
        /// of the other items.
        pub fn insert(&mut self, idx: usize, item: (TextInfo, Node)) {
            assert!(idx <= self.len());
            assert!(self.len() < MAX_CHILDREN);

            let len = self.len();

            // Shift over the elements in `nodes` and `info` to make room for
            // the new inserted item.  The `info` array shifting can be done
            // with a safe call to `copy_within()`.  The `nodes` array needs
            // unsafe code because of the specific drop semantics needed for
            // safety.
            unsafe {
                let ptr = self.nodes.as_mut_ptr();
                ptr::copy(ptr.add(idx), ptr.add(idx + 1), len - idx);
            }
            self.info.copy_within(idx..len, idx + 1);

            // Put the new items in.
            self.info[idx] = MaybeUninit::new(item.0);
            self.nodes[idx] = MaybeUninit::new(item.1);

            // The unbalance flags.
            let low_mask = range_bitmask(0, idx);
            let high_mask = range_bitmask(idx, self.len());
            self.subtree_unbalance_flags = ((self.subtree_unbalance_flags & high_mask) << 1)
                | (self.subtree_unbalance_flags & low_mask);

            self.len += 1;
        }

        /// Removes the item at the given index from the the array.
        ///
        /// Decreases length by one.  Preserves ordering of the other items.
        pub fn remove(&mut self, idx: usize) -> (TextInfo, Node) {
            assert!(idx < self.len());

            // Read out the item.
            // SAFETY: we know that both the info and node are initialized
            // because of the asserts above.  It's okay to use `assume_init_read()`
            // for the node, because that slot will either be overwritten by
            // another node or will be marked invalid, so this behaves as a move,
            // not a copy.
            let item = (unsafe { self.info[idx].assume_init() }, unsafe {
                self.nodes[idx].assume_init_read()
            });

            let len = self.len();
            // This unsafe code simply shifts the elements of the arrays over
            // to fill in the gap left by the removed element.  The `.info`
            // array shifting can be done with a safe call to `copy_within()`.
            // However, the `.nodes` array shift cannot, because of the
            // specific drop semantics needed for safety.
            unsafe {
                let ptr = self.nodes.as_mut_ptr();
                ptr::copy(ptr.add(idx + 1), ptr.add(idx), len - idx - 1);
            }
            self.info.copy_within((idx + 1)..len, idx);

            // The unbalance flags.
            let low_mask = range_bitmask(0, idx);
            let high_mask = range_bitmask(idx + 1, self.len());
            self.subtree_unbalance_flags = ((self.subtree_unbalance_flags & high_mask) >> 1)
                | (self.subtree_unbalance_flags & low_mask);

            self.len -= 1;

            return item;
        }

        /// Removes a range of items from `self`.
        ///
        /// Panics if the range is out of bounds.
        pub fn remove_range(&mut self, range: [usize; 2]) {
            assert!(range[0] <= range[1]);
            assert!(range[1] <= self.len());

            // Step 1: run `drop()` on the nodes to be removed.
            for node in &mut self.nodes[range[0]..range[1]] {
                // SAFETY: we know these nodes are initialized because they're
                // at indices < `self.len`.  By dropping them they become
                // invalid, but they will be overwritten or put out of range in
                // the next step.
                unsafe { node.assume_init_drop() };
            }

            // Step 2: shift items over to fill in the gap.
            {
                let range_len = range[1] - range[0];

                // Nodes.
                // SAFETY: this acts as a move, and together with reducing
                // `self.len` fills in the gap from step 1.
                unsafe {
                    let ptr = self.nodes.as_mut_ptr();
                    ptr::copy(
                        ptr.add(range[1]),
                        ptr.add(range[0]),
                        self.len as usize - range[1],
                    );
                }

                // Text info.
                self.info.copy_within(range[1]..self.len as usize, range[0]);

                // Move the unbalance flags.
                let low_mask = range_bitmask(0, range[0]);
                let high_mask = range_bitmask(range[1], self.len());
                self.subtree_unbalance_flags = ((self.subtree_unbalance_flags & high_mask)
                    >> (range[1] - range[0]))
                    | (self.subtree_unbalance_flags & low_mask);

                self.len -= range_len as u8;
            }
        }

        /// Removes a range of items from `other`, and inserts them into `self`.
        ///
        /// Panics if there isn't enough room to insert, or if the insert index
        /// or from-range are out of bounds.
        pub fn steal_range_from(
            &mut self,
            to_idx: usize,
            other: &mut Self,
            from_range: [usize; 2],
        ) {
            assert!(to_idx <= self.len());
            assert!(from_range[0] <= from_range[1]);
            assert!(from_range[1] <= other.len());

            let from_len = from_range[1] - from_range[0];
            assert!(from_len <= (MAX_CHILDREN - self.len()));

            let to_end_idx = to_idx + from_len;

            // Step 1: make room in `self` for the items.
            {
                // Nodes.
                // SAFETY: this acts as a move.  A gap is left with stale data
                // in it, but step 2 overwrites that gap with valid data.
                unsafe {
                    let ptr = self.nodes.as_mut_ptr();
                    ptr::copy(
                        ptr.add(to_idx),
                        ptr.add(to_end_idx),
                        self.len as usize - to_idx,
                    );
                }

                // Text info.
                self.info.copy_within(to_idx..self.len as usize, to_end_idx);

                // The unbalance flags.
                let low_mask = range_bitmask(0, to_idx);
                let high_mask = range_bitmask(to_idx, self.len());
                self.subtree_unbalance_flags = ((self.subtree_unbalance_flags & high_mask)
                    << (from_range[1] - from_range[0]))
                    | (self.subtree_unbalance_flags & low_mask);

                self.len += from_len as u8;
            }

            // Step 2: move the items from other to self.
            {
                // Nodes.
                // SAFETY: this acts as a move, and fills in the gap in `self`
                // from step 1. However, it now leaves a gap of stale data in
                // `other` where all the items we just moved over were.  Step 3
                // fills in that gap.
                unsafe {
                    let ptr_other = other.nodes.as_ptr();
                    let ptr_self = self.nodes.as_mut_ptr();
                    ptr::copy(ptr_other.add(from_range[0]), ptr_self.add(to_idx), from_len);
                }

                // Text info.
                self.info[to_idx..to_end_idx]
                    .copy_from_slice(&other.info[from_range[0]..from_range[1]]);

                // The unbalance flags.
                let mask = range_bitmask(from_range[0], from_range[1]);
                if from_range[0] <= to_idx {
                    self.subtree_unbalance_flags |=
                        (other.subtree_unbalance_flags & mask) << (to_idx - from_range[0]);
                } else {
                    self.subtree_unbalance_flags |=
                        (other.subtree_unbalance_flags & mask) >> (from_range[0] - to_idx);
                }
            }

            // Step 3: shift over the items in `other` to fill the gap.
            {
                // Nodes.
                // SAFETY: this acts as a move, and fills in the gap in `other`
                // from step 2. `other.len` is then adjusted so there is no gap
                // left at the end.
                unsafe {
                    let ptr = other.nodes.as_mut_ptr();
                    ptr::copy(
                        ptr.add(from_range[1]),
                        ptr.add(from_range[0]),
                        other.len as usize - from_range[1],
                    );
                }

                // Text info.
                other
                    .info
                    .copy_within(from_range[1]..other.len as usize, from_range[0]);

                // The unbalance flags.
                let low_mask = range_bitmask(0, from_range[0]);
                let high_mask = range_bitmask(from_range[1], other.len());
                other.subtree_unbalance_flags = ((other.subtree_unbalance_flags & high_mask)
                    >> (from_range[1] - from_range[0]))
                    | (other.subtree_unbalance_flags & low_mask);

                other.len -= from_len as u8;
            }
        }
    }

    impl Drop for ChildrenInternal {
        fn drop(&mut self) {
            // The `.nodes` array contains `MaybeUninit` wrappers, which need
            // to be manually dropped if valid.  We drop only the valid ones
            // here.
            for node in &mut self.nodes[..self.len as usize] {
                unsafe { node.assume_init_drop() };
            }
        }
    }

    impl Clone for ChildrenInternal {
        fn clone(&self) -> ChildrenInternal {
            // Create an empty ChildrenInternal first, then fill it.
            let mut clone_array = ChildrenInternal::new();

            // Copy Nodes carefully.
            for (dst_node, src_node) in Iterator::zip(
                clone_array.nodes[..self.len()].iter_mut(),
                self.nodes[..self.len()].iter(),
            ) {
                dst_node.write(unsafe { &*src_node.as_ptr() }.clone());
            }

            // Copy TextInfo.
            for (dst_info, src_info) in Iterator::zip(
                clone_array.info[..self.len()].iter_mut(),
                self.info[..self.len()].iter(),
            ) {
                *dst_info = *src_info;
            }

            // Set length.
            clone_array.len = self.len;

            // Some sanity checks for debug builds.
            #[cfg(debug_assertions)]
            {
                use std::sync::Arc;

                for (a, b) in Iterator::zip(
                    clone_array.info[..clone_array.len()].iter(),
                    self.info[..self.len()].iter(),
                ) {
                    assert_eq!(unsafe { a.assume_init() }, unsafe { b.assume_init() },);
                }

                for (a, b) in Iterator::zip(
                    clone_array.nodes[..clone_array.len()].iter(),
                    self.nodes[..clone_array.len()].iter(),
                ) {
                    let a = unsafe { a.assume_init_ref() };
                    let b = unsafe { b.assume_init_ref() };
                    match (a, b) {
                        (Node::Internal(ref a_arc), Node::Internal(ref b_arc)) => {
                            assert!(Arc::ptr_eq(a_arc, b_arc));
                        }
                        (Node::Leaf(ref a_arc), Node::Leaf(ref b_arc)) => {
                            assert!(Arc::ptr_eq(a_arc, b_arc));
                        }
                        _ => panic!("Cloned node is not the same type as its source."),
                    }
                }
            }

            clone_array
        }
    }

    impl fmt::Debug for ChildrenInternal {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct("ChildrenInternal")
                .field("nodes", &&self.nodes())
                .field("info", &&self.info())
                .field("len", &self.len())
                .finish()
        }
    }

    #[cfg(test)]
    mod tests {
        use std::sync::Arc;

        use super::*;
        use crate::tree::Text;

        // Generates a unique string with unique text info for any usize.
        fn i_to_s(i: usize) -> String {
            let mut s = String::with_capacity(i);
            let tmp = i.to_string();
            for _ in 0..(i + 1) {
                s.push_str(&tmp);
            }
            s
        }

        fn make_info_and_node(text: &str) -> (TextInfo, Node) {
            (
                TextInfo::from_str(text),
                Node::Leaf(Arc::new(Text::from_str(text))),
            )
        }

        fn make_children_full(unbalance_flag: bool) -> ChildrenInternal {
            let mut children = ChildrenInternal::new();
            for i in 0..MAX_CHILDREN {
                children.push(make_info_and_node(&i_to_s(i)));
                if unbalance_flag {
                    children.set_unbalance_flag(i);
                }
            }

            children
        }

        fn make_children_half_full(unbalance_flag: bool) -> ChildrenInternal {
            let mut children = ChildrenInternal::new();
            for i in 0..(MAX_CHILDREN / 2) {
                children.push(make_info_and_node(&i_to_s(i)));
                if unbalance_flag {
                    children.set_unbalance_flag(i);
                }
            }

            children
        }

        #[test]
        fn push_01() {
            let mut children = ChildrenInternal::new();
            for i in 0..MAX_CHILDREN {
                children.push(make_info_and_node(&i_to_s(i)));
            }
            for i in 0..MAX_CHILDREN {
                assert_eq!(children.info()[i].bytes, i_to_s(i).len());
                assert_eq!(children.nodes()[i].leaf_text(), i_to_s(i).as_str());
            }
        }

        #[test]
        fn pop_01() {
            let mut children = make_children_full(true);

            for i in (0..MAX_CHILDREN).rev() {
                let (info, node) = children.pop();

                assert_eq!(children.len(), i);
                assert_eq!(info.bytes, i_to_s(i).len());
                assert_eq!(node.leaf_text(), i_to_s(i).as_str());
                assert_eq!(!((!0) << i), children.subtree_unbalance_flags);
            }
        }

        #[test]
        fn insert_01() {
            let mut children = make_children_half_full(true);

            children.insert(1, make_info_and_node("a"));
            children.insert(children.len(), make_info_and_node("b"));
            children.insert(0, make_info_and_node("c"));

            for i in 0..MAX_CHILDREN {
                let text: String = match i {
                    0 => "c".into(),
                    2 => "a".into(),
                    i if i == (children.len() - 1) => "b".into(),
                    i if i < 2 => i_to_s(i - 1),
                    _ => i_to_s(i - 2),
                };

                assert_eq!(children.info()[i].bytes, text.len());
                assert_eq!(children.nodes()[i].leaf_text(), text.as_str());
            }

            let mut expected_bits = !((!0) << children.len());
            expected_bits &= !(1 << 0);
            expected_bits &= !(1 << 2);
            expected_bits &= !(1 << (children.len() - 1));

            assert_eq!(expected_bits, children.subtree_unbalance_flags);
        }

        #[test]
        fn insert_02() {
            let mut children = make_children_full(false);
            children.pop();
            for i in 0..children.len() {
                if (i % 2) == 0 {
                    children.set_unbalance_flag(i);
                }
            }

            children.insert(2, make_info_and_node("a"));

            for i in 0..children.len() {
                let unbalanced = children.is_node_unbalanced(i);
                if i < 2 {
                    assert_eq!((i % 2) == 0, unbalanced);
                } else if i == 2 {
                    assert_eq!(false, unbalanced);
                } else {
                    assert_eq!((i % 2) != 0, unbalanced);
                }
            }
        }

        #[test]
        fn remove_01() {
            let mut children = make_children_full(true);

            let last_i = children.len() - 1;

            let last = children.remove(last_i);
            let first = children.remove(0);
            let middle = children.remove(1);

            assert_eq!(children.len(), MAX_CHILDREN - 3);

            assert_eq!(last.0.bytes, i_to_s(last_i).len());
            assert_eq!(last.1.leaf_text(), i_to_s(last_i).as_str());

            assert_eq!(first.0.bytes, i_to_s(0).len());
            assert_eq!(first.1.leaf_text(), i_to_s(0).as_str());

            assert_eq!(middle.0.bytes, i_to_s(2).len());
            assert_eq!(middle.1.leaf_text(), i_to_s(2).as_str());

            assert_eq!(!((!0) << children.len()), children.subtree_unbalance_flags);
        }

        #[test]
        fn remove_02() {
            let mut children = make_children_full(false);
            for i in 0..children.len() {
                if (i % 2) == 0 {
                    children.set_unbalance_flag(i);
                }
            }

            children.remove(2);

            assert_eq!(
                0,
                children.subtree_unbalance_flags & ((!0) << children.len()),
            );

            for i in 0..children.len() {
                let unbalanced = children.is_node_unbalanced(i);
                if i < 2 {
                    assert_eq!((i % 2) == 0, unbalanced);
                } else {
                    assert_eq!((i % 2) != 0, unbalanced);
                }
            }
        }

        #[test]
        fn remove_range_01() {
            let ranges = &[[1, 1], [0, 2], [1, 3], [2, MAX_CHILDREN]];

            for &range in ranges {
                let mut children = make_children_full(true);
                let range_len = range[1] - range[0];

                children.remove_range(range);
                assert_eq!(children.len(), MAX_CHILDREN - range_len);
                for i in 0..children.len() {
                    let original_i = if i < range[0] { i } else { i + range_len };
                    let text = i_to_s(original_i);

                    assert_eq!(children.info()[i].bytes, text.len());
                    assert_eq!(children.nodes()[i].leaf_text(), text.as_str());
                }

                assert_eq!(!((!0) << children.len()), children.subtree_unbalance_flags);
            }
        }

        #[test]
        fn steal_range_from_01() {
            let idxs = &[0, 1, MAX_CHILDREN / 2];
            let ranges = &[[1, 1], [0, 2], [1, 3], [MAX_CHILDREN - 2, MAX_CHILDREN]];

            for &idx in idxs {
                for &range in ranges {
                    let mut children_to = make_children_half_full(true);
                    let mut children_from = make_children_full(true);
                    let range_len = range[1] - range[0];

                    children_to.steal_range_from(idx, &mut children_from, range);

                    // Verify `children_from`.
                    assert_eq!(children_from.len(), MAX_CHILDREN - range_len);
                    for i in 0..children_from.len() {
                        let original_i = if i < range[0] { i } else { i + range_len };
                        let text = i_to_s(original_i);

                        assert_eq!(children_from.info()[i].bytes, text.len());
                        assert_eq!(children_from.nodes()[i].leaf_text(), text.as_str());
                    }

                    // Verify `children_to`.
                    assert_eq!(children_to.len(), MAX_CHILDREN / 2 + range_len);
                    for i in 0..children_to.len() {
                        let original_i = if i < idx {
                            i
                        } else if i < (idx + range_len) {
                            i - idx + range[0]
                        } else {
                            i - range_len
                        };
                        let text = i_to_s(original_i);

                        assert_eq!(children_to.info()[i].bytes, text.len());
                        assert_eq!(children_to.nodes()[i].leaf_text(), text.as_str());
                    }

                    assert_eq!(
                        !((!0) << children_from.len()),
                        children_from.subtree_unbalance_flags
                    );
                    assert_eq!(
                        !((!0) << children_to.len()),
                        children_to.subtree_unbalance_flags
                    );
                }
            }
        }
    }
}
