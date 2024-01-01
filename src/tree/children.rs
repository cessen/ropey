use std::{
    iter::{Iterator, Zip},
    slice,
    sync::Arc,
};

use super::{node::Node, text_info::TextInfo, MAX_CHILDREN, MAX_TEXT_SIZE};

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
        self.0.len() as usize
    }

    /// Returns whether the child array is full or not.
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.len() == MAX_CHILDREN
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

    /// Updates the text info of the child at `idx`.
    pub fn update_child_info(&mut self, idx: usize) {
        let (info, nodes) = self.0.data_mut();
        info[idx] = nodes[idx].text_info();
    }

    /// Pushes an item onto the end of the child array.
    ///
    /// Increases length by one.  Panics if already full.
    #[inline(always)]
    pub fn push(&mut self, item: (TextInfo, Node)) {
        self.0.push(item)
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

    /// Attempts to merge two nodes, and if it's too much data to merge
    /// equi-distributes the data between the two.
    ///
    /// Returns:
    ///
    /// - True: merge was successful.
    /// - False: merge failed, equidistributed instead.
    pub fn merge_distribute(&mut self, idx1: usize, idx2: usize) -> bool {
        assert!(idx1 < idx2);
        assert!(idx2 < self.len());
        let remove_right = {
            let ((_, node1), (_, node2)) = self.get_two_mut(idx1, idx2);
            match (node1, node2) {
                (&mut Node::Leaf(ref mut text1), &mut Node::Leaf(ref mut text2)) => {
                    let text1 = Arc::make_mut(text1);
                    let text2 = Arc::make_mut(text2);
                    if (text1.len() + text2.len()) <= MAX_TEXT_SIZE {
                        text1.append_text(text2);
                        true
                    } else {
                        text1.distribute(text2);
                        false
                    }
                }

                (
                    &mut Node::Internal(ref mut children1),
                    &mut Node::Internal(ref mut children2),
                ) => {
                    let children1 = Arc::make_mut(children1);
                    let children2 = Arc::make_mut(children2);
                    if (children1.len() + children2.len()) <= MAX_CHILDREN {
                        for _ in 0..children2.len() {
                            // TODO: performance.  This is quadratic with
                            // the number of children in `children2`.
                            children1.push(children2.remove(0));
                        }
                        true
                    } else {
                        children1.distribute(children2);
                        false
                    }
                }

                // TODO: possibly handle this case somehow?  Not sure if it's needed.
                _ => panic!("Siblings have different node types"),
            }
        };

        if remove_right {
            self.remove(idx2);
            self.update_child_info(idx1);
            true
        } else {
            self.update_child_info(idx1);
            self.update_child_info(idx2);
            false
        }
    }

    /// Equidistributes the children between the two child arrays,
    /// preserving ordering.
    pub fn distribute(&mut self, other: &mut Self) {
        let r_target_len = (self.len() + other.len()) / 2;
        while other.len() < r_target_len {
            other.insert(0, self.pop());
        }
        while other.len() > r_target_len {
            self.push(other.remove(0));
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
        self.0.insert(idx, item)
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

    /// Splits the array in two at `idx`, returning the right part of the split.
    pub fn split_off(&mut self, idx: usize) -> Self {
        assert!(idx <= self.len());

        // TODO: performance.  This is quadratic with
        // the number of children.
        let mut other = Children::new();
        let count = self.len() - idx;
        for _ in 0..count {
            other.push(self.remove(idx));
        }

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

    /// Creates an iterator over the array's items.
    #[inline(always)]
    pub fn iter(&self) -> Zip<slice::Iter<TextInfo>, slice::Iter<Node>> {
        Iterator::zip(self.info().iter(), self.nodes().iter())
    }

    #[inline(always)]
    pub fn combined_info(&self) -> TextInfo {
        self.info()
            .iter()
            .fold(TextInfo::new(), |acc, &next| acc.combine(next))
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// first child that matches the given predicate.
    ///
    /// If no child matches the predicate, the last child is returned.
    #[inline(always)]
    pub fn search_by<F>(&self, pred: F) -> (usize, TextInfo)
    where
        // (left-accumulated start info, left-accumulated end info)
        F: Fn(TextInfo, TextInfo) -> bool,
    {
        debug_assert!(self.len() > 0);

        let mut accum = TextInfo::new();
        let mut idx = 0;
        while idx < (self.len() - 1) {
            let next_accum = {
                let next_info = self.info()[idx];
                let next_next_info = self
                    .info()
                    .get(idx + 1)
                    .map(|ti| *ti)
                    .unwrap_or(TextInfo::new());
                accum + next_info.adjusted_by_next(next_next_info)
            };
            if pred(accum, next_accum) {
                break;
            }
            accum = next_accum;
            idx += 1;
        }

        (idx, accum)
    }

    /// Same as `search_byte_idx()` below, except that it only calulates the
    /// left-side-accumulated _byte_ index rather than the full text info.
    ///
    /// Return is (child_index, left_acc_byte_index)
    ///
    /// One-past-the end is valid, and will return the last child.
    #[inline(always)]
    pub fn search_byte_idx_only(&self, byte_idx: usize) -> (usize, usize) {
        debug_assert!(self.len() > 0);

        let mut accum_byte_idx = 0;
        let mut idx = 0;
        for info in self.info()[0..(self.len() - 1)].iter() {
            let next_accum = accum_byte_idx + info.bytes as usize;
            if byte_idx < next_accum {
                break;
            }
            accum_byte_idx = next_accum;
            idx += 1;
        }

        debug_assert!(
            byte_idx <= (accum_byte_idx + self.info()[idx].bytes as usize) as usize,
            "Index out of bounds."
        );

        (idx, accum_byte_idx)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// child that contains the given byte.
    ///
    /// One-past-the end is valid, and will return the last child.
    pub fn search_byte_idx(&self, byte_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) = self.search_by(|_, end| byte_idx < end.bytes as usize);

        debug_assert!(
            byte_idx <= (accum.bytes + self.info()[idx].bytes) as usize,
            "Index out of bounds."
        );

        (idx, accum)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// child that contains the given char.
    ///
    /// One-past-the end is valid, and will return the last child.
    #[cfg(feature = "metric_chars")]
    pub fn search_char_idx(&self, char_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) = self.search_by(|_, end| char_idx < end.chars as usize);

        debug_assert!(
            char_idx <= (accum.chars + self.info()[idx].chars) as usize,
            "Index out of bounds."
        );

        (idx, accum)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// child that contains the given utf16 code unit offset.
    ///
    /// One-past-the end is valid, and will return the last child.
    #[cfg(feature = "metric_utf16")]
    pub fn search_utf16_code_unit_idx(&self, utf16_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) =
            self.search_by(|_, end| utf16_idx < (end.chars + end.utf16_surrogates) as usize);

        debug_assert!(
            utf16_idx
                <= (accum.chars
                    + accum.utf16_surrogates
                    + self.info()[idx].chars
                    + self.info()[idx].utf16_surrogates) as usize,
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
    #[cfg(feature = "metric_lines_lf")]
    pub fn search_line_break_lf_idx(&self, line_break_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) = self.search_by(|_, end| line_break_idx <= end.line_breaks_lf as usize);

        debug_assert!(
            line_break_idx <= (accum.line_breaks_lf + self.info()[idx].line_breaks_lf + 1) as usize,
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
    #[cfg(feature = "metric_lines_cr_lf")]
    pub fn search_line_break_cr_lf_idx(&self, line_break_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) =
            self.search_by(|_, end| line_break_idx <= end.line_breaks_cr_lf as usize);

        debug_assert!(
            line_break_idx
                <= (accum.line_breaks_cr_lf + self.info()[idx].line_breaks_cr_lf + 1) as usize,
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
    #[cfg(feature = "metric_lines_unicode")]
    pub fn search_line_break_unicode_idx(&self, line_break_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) =
            self.search_by(|_, end| line_break_idx <= end.line_breaks_unicode as usize);

        debug_assert!(
            line_break_idx
                <= (accum.line_breaks_unicode + self.info()[idx].line_breaks_unicode + 1) as usize,
            "Index out of bounds."
        );

        (idx, accum)
    }
}

//===========================================================================

/// The unsafe guts of Children, exposed through a safe API.
///
/// Try to keep this as small as possible, and implement functionality on
/// Children via the safe APIs whenever possible.
///
/// It's split out this way because it was too easy to accidentally access the
/// fixed size arrays directly, leading to memory-unsafety bugs when accidentally
/// accessing elements that are semantically out of bounds.  This happened once,
/// and it was a pain to track down--as memory safety bugs often are.
mod inner {
    use super::{Node, TextInfo, MAX_CHILDREN};
    use std::fmt;
    use std::mem;
    use std::mem::MaybeUninit;
    use std::ptr;
    use std::sync::Arc;

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

        /// Pushes an item onto the end of the array.
        ///
        /// Increases length by one.  Panics if already full.
        #[inline(always)]
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
        #[inline(always)]
        pub fn pop(&mut self) -> (TextInfo, Node) {
            assert!(self.len() > 0);
            self.len -= 1;
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
        #[inline(always)]
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

            self.len += 1;
        }

        /// Removes the item at the given index from the the array.
        ///
        /// Decreases length by one.  Preserves ordering of the other items.
        #[inline(always)]
        pub fn remove(&mut self, idx: usize) -> (TextInfo, Node) {
            assert!(self.len() > 0);
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

            self.len -= 1;

            return item;
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
                for (a, b) in Iterator::zip(
                    (&clone_array.info[..clone_array.len()]).iter(),
                    (&self.info[..self.len()]).iter(),
                ) {
                    assert_eq!(unsafe { a.assume_init() }, unsafe { b.assume_init() },);
                }

                for (a, b) in Iterator::zip(
                    (&clone_array.nodes[..clone_array.len()]).iter(),
                    (&self.nodes[..clone_array.len()]).iter(),
                ) {
                    let a = unsafe { a.assume_init_ref() };
                    let b = unsafe { b.assume_init_ref() };
                    match (a, b) {
                        (Node::Internal(a_arc), Node::Internal(b_arc)) => {
                            assert!(Arc::ptr_eq(&a_arc, &b_arc));
                        }
                        (Node::Leaf(a_arc), Node::Leaf(b_arc)) => {
                            assert!(Arc::ptr_eq(&a_arc, &b_arc));
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
}
