use std::fmt;
use std::iter::{Iterator, Zip};
use std::slice;
use std::sync::Arc;

use crate::crlf;
use crate::tree::{self, Node, TextInfo, MAX_BYTES};

const MAX_LEN: usize = tree::MAX_CHILDREN;

/// A fixed-capacity vec of child Arc-pointers and child metadata.
///
/// The unsafe guts of this are implemented in NodeChildrenInternal
/// lower down in this file.
#[derive(Clone)]
#[repr(C)]
pub(crate) struct NodeChildren(inner::NodeChildrenInternal);

impl NodeChildren {
    /// Creates a new empty array.
    pub fn new() -> Self {
        NodeChildren(inner::NodeChildrenInternal::new())
    }

    /// Current length of the array.
    pub fn len(&self) -> usize {
        self.0.len() as usize
    }

    /// Returns whether the array is full or not.
    pub fn is_full(&self) -> bool {
        self.len() == MAX_LEN
    }

    /// Access to the nodes array.
    pub fn nodes(&self) -> &[Arc<Node>] {
        self.0.nodes()
    }

    /// Mutable access to the nodes array.
    pub fn nodes_mut(&mut self) -> &mut [Arc<Node>] {
        self.0.nodes_mut()
    }

    /// Access to the info array.
    pub fn info(&self) -> &[TextInfo] {
        self.0.info()
    }

    /// Mutable access to the info array.
    pub fn info_mut(&mut self) -> &mut [TextInfo] {
        self.0.info_mut()
    }

    /// Mutable access to both the info and nodes arrays simultaneously.
    pub fn data_mut(&mut self) -> (&mut [TextInfo], &mut [Arc<Node>]) {
        self.0.data_mut()
    }

    /// Updates the text info of the child at `idx`.
    pub fn update_child_info(&mut self, idx: usize) {
        let (info, nodes) = self.0.data_mut();
        info[idx] = nodes[idx].text_info();
    }

    /// Pushes an item into the end of the array.
    ///
    /// Increases length by one.  Panics if already full.
    pub fn push(&mut self, item: (TextInfo, Arc<Node>)) {
        self.0.push(item)
    }

    /// Pushes an element onto the end of the array, and then splits it in half,
    /// returning the right half.
    ///
    /// This works even when the array is full.
    pub fn push_split(&mut self, new_child: (TextInfo, Arc<Node>)) -> Self {
        let r_count = (self.len() + 1) / 2;
        let l_count = (self.len() + 1) - r_count;

        let mut right = self.split_off(l_count);
        right.push(new_child);
        right
    }

    /// Attempts to merge two nodes, and if it's too much data to merge
    /// equi-distributes it between the two.
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
            let node1 = Arc::make_mut(node1);
            let node2 = Arc::make_mut(node2);
            match *node1 {
                Node::Leaf(ref mut text1) => {
                    if let Node::Leaf(ref mut text2) = *node2 {
                        if (text1.len() + text2.len()) <= tree::MAX_BYTES {
                            text1.push_str(text2);
                            true
                        } else {
                            let right = text1.push_str_split(text2);
                            *text2 = right;
                            false
                        }
                    } else {
                        panic!("Siblings have different node types");
                    }
                }

                Node::Internal(ref mut children1) => {
                    if let Node::Internal(ref mut children2) = *node2 {
                        if (children1.len() + children2.len()) <= MAX_LEN {
                            for _ in 0..children2.len() {
                                children1.push(children2.remove(0));
                            }
                            true
                        } else {
                            children1.distribute_with(children2);
                            false
                        }
                    } else {
                        panic!("Siblings have different node types");
                    }
                }
            }
        };

        if remove_right {
            self.remove(idx2);
            self.update_child_info(idx1);
            return true;
        } else {
            self.update_child_info(idx1);
            self.update_child_info(idx2);
            return false;
        }
    }

    /// Equi-distributes the children between the two child arrays,
    /// preserving ordering.
    pub fn distribute_with(&mut self, other: &mut Self) {
        let r_target_len = (self.len() + other.len()) / 2;
        while other.len() < r_target_len {
            other.insert(0, self.pop());
        }
        while other.len() > r_target_len {
            self.push(other.remove(0));
        }
    }

    /// If the children are leaf nodes, compacts them to take up the fewest
    /// nodes.
    pub fn compact_leaves(&mut self) {
        if !self.nodes()[0].is_leaf() || self.len() < 2 {
            return;
        }

        let mut i = 1;
        while i < self.len() {
            if (self.nodes()[i - 1].leaf_text().len() + self.nodes()[i].leaf_text().len())
                <= MAX_BYTES
            {
                // Scope to contain borrows
                {
                    let ((_, node_l), (_, node_r)) = self.get_two_mut(i - 1, i);
                    let text_l = Arc::make_mut(node_l).leaf_text_mut();
                    let text_r = node_r.leaf_text();
                    text_l.push_str(text_r);
                }
                self.remove(i);
            } else if self.nodes()[i - 1].leaf_text().len() < MAX_BYTES {
                // Scope to contain borrows
                {
                    let ((_, node_l), (_, node_r)) = self.get_two_mut(i - 1, i);
                    let text_l = Arc::make_mut(node_l).leaf_text_mut();
                    let text_r = Arc::make_mut(node_r).leaf_text_mut();
                    let split_idx_r = crlf::prev_break(MAX_BYTES - text_l.len(), text_r.as_bytes());
                    text_l.push_str(&text_r[..split_idx_r]);
                    text_r.truncate_front(split_idx_r);
                }
                i += 1;
            } else {
                i += 1;
            }
        }

        for i in 0..self.len() {
            self.update_child_info(i);
        }
    }

    /// Pops an item off the end of the array and returns it.
    ///
    /// Decreases length by one.  Panics if already empty.
    pub fn pop(&mut self) -> (TextInfo, Arc<Node>) {
        self.0.pop()
    }

    /// Inserts an item into the the array at the given index.
    ///
    /// Increases length by one.  Panics if already full.  Preserves ordering
    /// of the other items.
    pub fn insert(&mut self, idx: usize, item: (TextInfo, Arc<Node>)) {
        self.0.insert(idx, item)
    }

    /// Inserts an element into a the array, and then splits it in half, returning
    /// the right half.
    ///
    /// This works even when the array is full.
    pub fn insert_split(&mut self, idx: usize, item: (TextInfo, Arc<Node>)) -> Self {
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
    pub fn remove(&mut self, idx: usize) -> (TextInfo, Arc<Node>) {
        self.0.remove(idx)
    }

    /// Splits the array in two at `idx`, returning the right part of the split.
    ///
    /// TODO: implement this more efficiently.
    pub fn split_off(&mut self, idx: usize) -> Self {
        assert!(idx <= self.len());

        let mut other = NodeChildren::new();
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
    ) -> (
        (&mut TextInfo, &mut Arc<Node>),
        (&mut TextInfo, &mut Arc<Node>),
    ) {
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
    pub fn iter(&self) -> Zip<slice::Iter<TextInfo>, slice::Iter<Arc<Node>>> {
        Iterator::zip(self.info().iter(), self.nodes().iter())
    }

    #[allow(clippy::needless_range_loop)]
    pub fn combined_info(&self) -> TextInfo {
        let info = self.info();
        let mut acc = TextInfo::new();

        // Doing this with an explicit loop is notably faster than
        // using an iterator in this case.
        for i in 0..info.len() {
            acc += info[i];
        }

        acc
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
        for info in self.info()[0..(self.len() - 1)].iter() {
            let next_accum = accum + *info;
            if pred(accum, next_accum) {
                break;
            }
            accum = next_accum;
            idx += 1;
        }

        (idx, accum)
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

    /// Same as `search_char_idx()` above, except that it only calulates the
    /// left-side-accumulated _char_ index rather than the full text info.
    ///
    /// Return is (child_index, left_acc_char_index)
    ///
    /// One-past-the end is valid, and will return the last child.
    #[inline(always)]
    pub fn search_char_idx_only(&self, char_idx: usize) -> (usize, usize) {
        debug_assert!(self.len() > 0);

        let mut accum_char_idx = 0;
        let mut idx = 0;
        for info in self.info()[0..(self.len() - 1)].iter() {
            let next_accum = accum_char_idx + info.chars as usize;
            if char_idx < next_accum {
                break;
            }
            accum_char_idx = next_accum;
            idx += 1;
        }

        debug_assert!(
            char_idx <= (accum_char_idx + self.info()[idx].chars as usize) as usize,
            "Index out of bounds."
        );

        (idx, accum_char_idx)
    }

    /// Returns the child index and left-side-accumulated text info of the
    /// child that contains the given line break.
    ///
    /// Beginning of the rope is considered index 0, although is not
    /// considered a line break for the returned left-side-accumulated
    /// text info.
    ///
    /// One-past-the end is valid, and will return the last child.
    pub fn search_line_break_idx(&self, line_break_idx: usize) -> (usize, TextInfo) {
        let (idx, accum) = self.search_by(|_, end| line_break_idx <= end.line_breaks as usize);

        debug_assert!(
            line_break_idx <= (accum.line_breaks + self.info()[idx].line_breaks + 1) as usize,
            "Index out of bounds."
        );

        (idx, accum)
    }

    /// Returns the child indices at the start and end of the given char
    /// range, and returns their left-side-accumulated char indices as well.
    ///
    /// Return is:
    /// (
    ///     (left_node_index, left_acc_left_side_char_index),
    ///     (right_node_index, right_acc_left_side_char_index),
    /// )
    ///
    /// One-past-the end is valid, and corresponds to the last child.
    #[inline(always)]
    pub fn search_char_idx_range(
        &self,
        start_idx: usize,
        end_idx: usize,
    ) -> ((usize, usize), (usize, usize)) {
        debug_assert!(start_idx <= end_idx);
        debug_assert!(self.len() > 0);

        let mut accum_char_idx = 0;
        let mut idx = 0;

        // Find left child and info
        for info in self.info()[..(self.len() - 1)].iter() {
            let next_accum = accum_char_idx + info.chars as usize;
            if start_idx < next_accum {
                break;
            }
            accum_char_idx = next_accum;
            idx += 1;
        }
        let l_child_i = idx;
        let l_acc_info = accum_char_idx;

        // Find right child and info
        for info in self.info()[idx..(self.len() - 1)].iter() {
            let next_accum = accum_char_idx + info.chars as usize;
            if end_idx <= next_accum {
                break;
            }
            accum_char_idx = next_accum;
            idx += 1;
        }

        #[cfg(any(test, debug_assertions))]
        assert!(
            end_idx <= accum_char_idx + self.info()[idx].chars as usize,
            "Index out of bounds."
        );

        ((l_child_i, l_acc_info), (idx, accum_char_idx))
    }

    // Debug function, to help verify tree integrity
    pub fn is_info_accurate(&self) -> bool {
        for (info, node) in self.info().iter().zip(self.nodes().iter()) {
            if *info != node.text_info() {
                return false;
            }
        }
        true
    }
}

impl fmt::Debug for NodeChildren {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("NodeChildren")
            .field("len", &self.len())
            .field("info", &&self.info())
            .field("nodes", &&self.nodes())
            .finish()
    }
}

//===========================================================================

/// The unsafe guts of NodeChildren, exposed through a safe API.
///
/// Try to keep this as small as possible, and implement functionality on
/// NodeChildren via the safe APIs whenever possible.
///
/// It's split out this way because it was too easy to accidentally access the
/// fixed size arrays directly, leading to memory-unsafety bugs when accidentally
/// accessing elements that are semantically out of bounds.  This happened once,
/// and it was a pain to track down--as memory safety bugs often are.
mod inner {
    use super::{Node, TextInfo, MAX_LEN};
    use std::mem;
    use std::mem::MaybeUninit;
    use std::ptr;
    use std::sync::Arc;

    /// This is essentially a fixed-capacity, stack-allocated `Vec`.  However,
    /// it actually containts _two_ arrays rather than just one, but which
    /// share a length.
    #[repr(C)]
    pub(crate) struct NodeChildrenInternal {
        /// An array of the child nodes.
        /// INVARIANT: The nodes from 0..len must be initialized
        nodes: [MaybeUninit<Arc<Node>>; MAX_LEN],
        /// An array of the child node text infos
        /// INVARIANT: The nodes from 0..len must be initialized
        info: [MaybeUninit<TextInfo>; MAX_LEN],
        len: u8,
    }

    impl NodeChildrenInternal {
        /// Creates a new empty array.
        #[inline(always)]
        pub fn new() -> NodeChildrenInternal {
            // SAFETY: Uninit data is valid for arrays of MaybeUninit.
            // len is zero, so it's ok for all of them to be uninit
            NodeChildrenInternal {
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
        pub fn nodes(&self) -> &[Arc<Node>] {
            // SAFETY: MaybeUninit<T> is layout compatible with T, and
            // the nodes from 0..len are guaranteed to be initialized
            unsafe { mem::transmute(&self.nodes[..(self.len())]) }
        }

        /// Mutable access to the nodes array.
        #[inline(always)]
        pub fn nodes_mut(&mut self) -> &mut [Arc<Node>] {
            // SAFETY: MaybeUninit<T> is layout compatible with T, and
            // the nodes from 0..len are guaranteed to be initialized
            unsafe { mem::transmute(&mut self.nodes[..(self.len as usize)]) }
        }

        /// Access to the info array.
        #[inline(always)]
        pub fn info(&self) -> &[TextInfo] {
            // SAFETY: MaybeUninit<T> is layout compatible with T, and
            // the info from 0..len are guaranteed to be initialized
            unsafe { mem::transmute(&self.info[..(self.len())]) }
        }

        /// Mutable access to the info array.
        #[inline(always)]
        pub fn info_mut(&mut self) -> &mut [TextInfo] {
            // SAFETY: MaybeUninit<T> is layout compatible with T, and
            // the info from 0..len are guaranteed to be initialized
            unsafe { mem::transmute(&mut self.info[..(self.len as usize)]) }
        }

        /// Mutable access to both the info and nodes arrays simultaneously.
        #[inline(always)]
        pub fn data_mut(&mut self) -> (&mut [TextInfo], &mut [Arc<Node>]) {
            // SAFETY: MaybeUninit<T> is layout compatible with T, and
            // the info from 0..len are guaranteed to be initialized
            (
                unsafe { mem::transmute(&mut self.info[..(self.len as usize)]) },
                unsafe { mem::transmute(&mut self.nodes[..(self.len as usize)]) },
            )
        }

        /// Pushes an item into the end of the array.
        ///
        /// Increases length by one.  Panics if already full.
        #[inline(always)]
        pub fn push(&mut self, item: (TextInfo, Arc<Node>)) {
            assert!(self.len() < MAX_LEN);
            self.info[self.len()] = MaybeUninit::new(item.0);
            self.nodes[self.len as usize] = MaybeUninit::new(item.1);
            // We have just initialized both info and node and 0..=len, so we can increase it
            self.len += 1;
        }

        /// Pops an item off the end of the array and returns it.
        ///
        /// Decreases length by one.  Panics if already empty.
        #[inline(always)]
        pub fn pop(&mut self) -> (TextInfo, Arc<Node>) {
            assert!(self.len() > 0);
            self.len -= 1;
            // SAFETY: before this, len was long enough to guarantee that both must be init
            // We just decreased the length, guaranteeing that the elements will never be read again
            (unsafe { self.info[self.len()].assume_init() }, unsafe {
                ptr::read(&self.nodes[self.len()]).assume_init()
            })
        }

        /// Inserts an item into the the array at the given index.
        ///
        /// Increases length by one.  Panics if already full.  Preserves ordering
        /// of the other items.
        #[inline(always)]
        pub fn insert(&mut self, idx: usize, item: (TextInfo, Arc<Node>)) {
            assert!(idx <= self.len());
            assert!(self.len() < MAX_LEN);

            let len = self.len();
            // This unsafe code simply shifts the elements of the arrays over
            // to make space for the new inserted value.  The `.info` array
            // shifting can be done with a safe call to `copy_within()`.
            // However, the `.nodes` array shift cannot, because of the
            // specific drop semantics needed for safety.
            unsafe {
                let ptr = self.nodes.as_mut_ptr();
                ptr::copy(ptr.add(idx), ptr.add(idx + 1), len - idx);
            }
            self.info.copy_within(idx..len, idx + 1);

            // We have just made space for the two new elements, so insert them
            self.info[idx] = MaybeUninit::new(item.0);
            self.nodes[idx] = MaybeUninit::new(item.1);
            // Now that all elements from 0..=len are initialized, we can increase the length
            self.len += 1;
        }

        /// Removes the item at the given index from the the array.
        ///
        /// Decreases length by one.  Preserves ordering of the other items.
        #[inline(always)]
        pub fn remove(&mut self, idx: usize) -> (TextInfo, Arc<Node>) {
            assert!(self.len() > 0);
            assert!(idx < self.len());

            // Read out the elements, they must not be touched again. We copy the elements
            // after them into them, and decrease the length at the end
            let item = (unsafe { self.info[idx].assume_init() }, unsafe {
                ptr::read(&self.nodes[idx]).assume_init()
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

            // Now that the gap is filled, decrease the length
            self.len -= 1;

            return item;
        }
    }

    impl Drop for NodeChildrenInternal {
        fn drop(&mut self) {
            // The `.nodes` array contains `MaybeUninit` wrappers, which need
            // to be manually dropped if valid.  We drop only the valid ones
            // here.
            for node in &mut self.nodes[..self.len as usize] {
                unsafe { ptr::drop_in_place(node.as_mut_ptr()) };
            }
        }
    }

    impl Clone for NodeChildrenInternal {
        fn clone(&self) -> NodeChildrenInternal {
            // Create an empty NodeChildrenInternal first, then fill it
            let mut clone_array = NodeChildrenInternal::new();

            // Copy nodes... carefully.
            for (clone_arc, arc) in Iterator::zip(
                clone_array.nodes[..self.len()].iter_mut(),
                self.nodes[..self.len()].iter(),
            ) {
                *clone_arc = MaybeUninit::new(Arc::clone(unsafe { &*arc.as_ptr() }));
            }

            // Copy TextInfo
            for (clone_info, info) in Iterator::zip(
                clone_array.info[..self.len()].iter_mut(),
                self.info[..self.len()].iter(),
            ) {
                *clone_info = *info;
            }

            // Set length
            clone_array.len = self.len;

            // Some sanity checks for debug builds
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
                    assert!(Arc::ptr_eq(unsafe { &*a.as_ptr() }, unsafe {
                        &*b.as_ptr()
                    },));
                }
            }

            clone_array
        }
    }
}

//===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{Node, NodeText, TextInfo};
    use std::sync::Arc;

    #[test]
    fn search_char_idx_01() {
        let mut children = NodeChildren::new();
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("Hello "))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("there "))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("world!"))),
        ));

        children.update_child_info(0);
        children.update_child_info(1);
        children.update_child_info(2);

        assert_eq!(0, children.search_char_idx(0).0);
        assert_eq!(0, children.search_char_idx(1).0);
        assert_eq!(0, children.search_char_idx(0).1.chars);
        assert_eq!(0, children.search_char_idx(1).1.chars);

        assert_eq!(0, children.search_char_idx(5).0);
        assert_eq!(1, children.search_char_idx(6).0);
        assert_eq!(0, children.search_char_idx(5).1.chars);
        assert_eq!(6, children.search_char_idx(6).1.chars);

        assert_eq!(1, children.search_char_idx(11).0);
        assert_eq!(2, children.search_char_idx(12).0);
        assert_eq!(6, children.search_char_idx(11).1.chars);
        assert_eq!(12, children.search_char_idx(12).1.chars);

        assert_eq!(2, children.search_char_idx(17).0);
        assert_eq!(2, children.search_char_idx(18).0);
        assert_eq!(12, children.search_char_idx(17).1.chars);
        assert_eq!(12, children.search_char_idx(18).1.chars);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn search_char_idx_02() {
        let mut children = NodeChildren::new();
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("Hello "))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("there "))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("world!"))),
        ));

        children.update_child_info(0);
        children.update_child_info(1);
        children.update_child_info(2);

        children.search_char_idx(19);
    }

    #[test]
    fn search_char_idx_range_01() {
        let mut children = NodeChildren::new();
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("Hello "))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("there "))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("world!"))),
        ));

        children.update_child_info(0);
        children.update_child_info(1);
        children.update_child_info(2);

        let at_0_0 = children.search_char_idx_range(0, 0);
        let at_6_6 = children.search_char_idx_range(6, 6);
        let at_12_12 = children.search_char_idx_range(12, 12);
        let at_18_18 = children.search_char_idx_range(18, 18);

        assert_eq!(0, (at_0_0.0).0);
        assert_eq!(0, (at_0_0.1).0);
        assert_eq!(0, (at_0_0.0).1);
        assert_eq!(0, (at_0_0.1).1);

        assert_eq!(1, (at_6_6.0).0);
        assert_eq!(1, (at_6_6.1).0);
        assert_eq!(6, (at_6_6.0).1);
        assert_eq!(6, (at_6_6.1).1);

        assert_eq!(2, (at_12_12.0).0);
        assert_eq!(2, (at_12_12.1).0);
        assert_eq!(12, (at_12_12.0).1);
        assert_eq!(12, (at_12_12.1).1);

        assert_eq!(2, (at_18_18.0).0);
        assert_eq!(2, (at_18_18.1).0);
        assert_eq!(12, (at_18_18.0).1);
        assert_eq!(12, (at_18_18.1).1);

        let at_0_6 = children.search_char_idx_range(0, 6);
        let at_6_12 = children.search_char_idx_range(6, 12);
        let at_12_18 = children.search_char_idx_range(12, 18);

        assert_eq!(0, (at_0_6.0).0);
        assert_eq!(0, (at_0_6.1).0);
        assert_eq!(0, (at_0_6.0).1);
        assert_eq!(0, (at_0_6.1).1);

        assert_eq!(1, (at_6_12.0).0);
        assert_eq!(1, (at_6_12.1).0);
        assert_eq!(6, (at_6_12.0).1);
        assert_eq!(6, (at_6_12.1).1);

        assert_eq!(2, (at_12_18.0).0);
        assert_eq!(2, (at_12_18.1).0);
        assert_eq!(12, (at_12_18.0).1);
        assert_eq!(12, (at_12_18.1).1);

        let at_5_7 = children.search_char_idx_range(5, 7);
        let at_11_13 = children.search_char_idx_range(11, 13);

        assert_eq!(0, (at_5_7.0).0);
        assert_eq!(1, (at_5_7.1).0);
        assert_eq!(0, (at_5_7.0).1);
        assert_eq!(6, (at_5_7.1).1);

        assert_eq!(1, (at_11_13.0).0);
        assert_eq!(2, (at_11_13.1).0);
        assert_eq!(6, (at_11_13.0).1);
        assert_eq!(12, (at_11_13.1).1);
    }

    #[test]
    #[should_panic]
    fn search_char_idx_range_02() {
        let mut children = NodeChildren::new();
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("Hello "))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("there "))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("world!"))),
        ));

        children.update_child_info(0);
        children.update_child_info(1);
        children.update_child_info(2);

        children.search_char_idx_range(18, 19);
    }

    #[test]
    fn search_line_break_idx_01() {
        let mut children = NodeChildren::new();
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("Hello\n"))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("\nthere\n"))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("world!\n"))),
        ));

        children.update_child_info(0);
        children.update_child_info(1);
        children.update_child_info(2);

        assert_eq!(0, children.search_line_break_idx(0).0);
        assert_eq!(0, children.search_line_break_idx(0).1.line_breaks);

        assert_eq!(0, children.search_line_break_idx(1).0);
        assert_eq!(0, children.search_line_break_idx(1).1.line_breaks);

        assert_eq!(1, children.search_line_break_idx(2).0);
        assert_eq!(1, children.search_line_break_idx(2).1.line_breaks);

        assert_eq!(1, children.search_line_break_idx(3).0);
        assert_eq!(1, children.search_line_break_idx(3).1.line_breaks);

        assert_eq!(2, children.search_line_break_idx(4).0);
        assert_eq!(3, children.search_line_break_idx(4).1.line_breaks);

        assert_eq!(2, children.search_line_break_idx(5).0);
        assert_eq!(3, children.search_line_break_idx(5).1.line_breaks);
    }

    #[test]
    fn search_line_break_idx_02() {
        let mut children = NodeChildren::new();
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("Hello\n"))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("there"))),
        ));
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str("world!"))),
        ));

        children.update_child_info(0);
        children.update_child_info(1);
        children.update_child_info(2);

        assert_eq!(0, children.search_line_break_idx(0).0);
        assert_eq!(0, children.search_line_break_idx(0).1.line_breaks);

        assert_eq!(0, children.search_line_break_idx(1).0);
        assert_eq!(0, children.search_line_break_idx(1).1.line_breaks);

        assert_eq!(2, children.search_line_break_idx(2).0);
        assert_eq!(1, children.search_line_break_idx(2).1.line_breaks);
    }

    #[test]
    fn search_line_break_idx_03() {
        let mut children = NodeChildren::new();
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str(""))),
        ));

        children.update_child_info(0);

        assert_eq!(0, children.search_line_break_idx(0).0);
        assert_eq!(0, children.search_line_break_idx(0).1.line_breaks);

        assert_eq!(0, children.search_line_break_idx(1).0);
        assert_eq!(0, children.search_line_break_idx(1).1.line_breaks);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn search_line_break_idx_04() {
        let mut children = NodeChildren::new();
        children.push((
            TextInfo::new(),
            Arc::new(Node::Leaf(NodeText::from_str(""))),
        ));

        children.update_child_info(0);

        assert_eq!(0, children.search_line_break_idx(0).0);
        assert_eq!(0, children.search_line_break_idx(0).1.line_breaks);

        assert_eq!(0, children.search_line_break_idx(1).0);
        assert_eq!(0, children.search_line_break_idx(1).1.line_breaks);

        assert_eq!(0, children.search_line_break_idx(2).0);
        assert_eq!(0, children.search_line_break_idx(2).1.line_breaks);
    }
}
