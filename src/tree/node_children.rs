#![allow(dead_code)]

use std;
use std::fmt;
use std::iter::{Iterator, Zip};
use std::mem;
use std::mem::ManuallyDrop;
use std::ptr;
use std::slice;
use std::sync::Arc;

use tree;
use tree::{Node, MAX_BYTES};
use str_utils::{nearest_internal_grapheme_boundary, prev_grapheme_boundary};
use tree::TextInfo;

const MAX_LEN: usize = tree::MAX_CHILDREN;

pub(crate) struct NodeChildren {
    nodes: ManuallyDrop<[Arc<Node>; MAX_LEN]>,
    info: [TextInfo; MAX_LEN],
    len: u8,
}

impl NodeChildren {
    /// Creates a new empty array.
    pub fn new() -> NodeChildren {
        NodeChildren {
            nodes: ManuallyDrop::new(unsafe { std::mem::uninitialized() }),
            info: unsafe { std::mem::uninitialized() },
            len: 0,
        }
    }

    /// Current length of the array.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns whether the array is full or not.
    pub fn is_full(&self) -> bool {
        (self.len as usize) == MAX_LEN
    }

    /// Returns a slice to the nodes array.
    pub fn nodes(&self) -> &[Arc<Node>] {
        &self.nodes[..(self.len as usize)]
    }

    /// Returns a mutable slice to the nodes array.
    pub fn nodes_mut(&mut self) -> &mut [Arc<Node>] {
        &mut self.nodes[..(self.len as usize)]
    }

    /// Returns a slice to the info array.
    pub fn info(&self) -> &[TextInfo] {
        &self.info[..(self.len as usize)]
    }

    /// Returns a mutable slice to the info array.
    pub fn info_mut(&mut self) -> &mut [TextInfo] {
        &mut self.info[..(self.len as usize)]
    }

    /// Returns mutable slices to both the nodes and info arrays.
    pub fn info_and_nodes_mut(&mut self) -> (&mut [TextInfo], &mut [Arc<Node>]) {
        (
            &mut self.info[..(self.len as usize)],
            &mut self.nodes[..(self.len as usize)],
        )
    }

    /// Updates the text info of the child at `idx`.
    pub fn update_child_info(&mut self, idx: usize) {
        self.info[idx] = self.nodes[idx].text_info();
    }

    /// Pushes an item into the end of the array.
    ///
    /// Increases length by one.  Panics if already full.
    pub fn push(&mut self, item: (TextInfo, Arc<Node>)) {
        assert!(self.len() < MAX_LEN);
        self.info[self.len as usize] = item.0;
        mem::forget(mem::replace(&mut self.nodes[self.len as usize], item.1));
        self.len += 1;
    }

    /// Pushes an element onto the end of the array, and then splits it in half,
    /// returning the right half.
    ///
    /// This works even when the array is full.
    pub fn push_split(&mut self, new_child: (TextInfo, Arc<Node>)) -> NodeChildren {
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
                        text1.push_str(text2);

                        if text1.len() <= tree::MAX_BYTES {
                            true
                        } else {
                            let split_pos = {
                                let pos = text1.len() - (text1.len() / 2);
                                nearest_internal_grapheme_boundary(text1, pos)
                            };
                            *text2 = text1.split_off(split_pos);
                            if text2.len() > 0 {
                                text1.shrink_to_fit();
                                false
                            } else {
                                true
                            }
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
            self.info[idx1] = self.nodes[idx1].text_info();
            return true;
        } else {
            self.info[idx1] = self.nodes[idx1].text_info();
            self.info[idx2] = self.nodes[idx2].text_info();
            return false;
        }
    }

    /// Equi-distributes the children between the two child arrays,
    /// preserving ordering.
    pub fn distribute_with(&mut self, other: &mut NodeChildren) {
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
        if !self.nodes[0].is_leaf() || self.len() < 2 {
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
                    let split_idx_r = prev_grapheme_boundary(text_r, MAX_BYTES - text_l.len());
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
        assert!(self.len() > 0);
        self.len -= 1;
        (self.info[self.len as usize], unsafe {
            ptr::read(&self.nodes[self.len as usize])
        })
    }

    /// Inserts an item into the the array at the given index.
    ///
    /// Increases length by one.  Panics if already full.  Preserves ordering
    /// of the other items.
    pub fn insert(&mut self, idx: usize, item: (TextInfo, Arc<Node>)) {
        assert!(idx <= self.len());
        assert!(self.len() < MAX_LEN);

        let len = self.len as usize;
        unsafe {
            ptr::copy(
                self.nodes.as_ptr().offset(idx as isize),
                self.nodes.as_mut_ptr().offset((idx + 1) as isize),
                len - idx,
            );
            ptr::copy(
                self.info.as_ptr().offset(idx as isize),
                self.info.as_mut_ptr().offset((idx + 1) as isize),
                len - idx,
            );
        }

        self.info[idx] = item.0;
        mem::forget(mem::replace(&mut self.nodes[idx], item.1));

        self.len += 1;
    }

    /// Inserts an element into a the array, and then splits it in half, returning
    /// the right half.
    ///
    /// This works even when the array is full.
    pub fn insert_split(&mut self, idx: usize, item: (TextInfo, Arc<Node>)) -> NodeChildren {
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
        assert!(self.len() > 0);
        assert!(idx < self.len());

        let item = (self.info[idx], unsafe { ptr::read(&self.nodes[idx]) });

        let len = self.len as usize;
        unsafe {
            ptr::copy(
                self.nodes.as_ptr().offset(idx as isize + 1),
                self.nodes.as_mut_ptr().offset(idx as isize),
                len - idx - 1,
            );
            ptr::copy(
                self.info.as_ptr().offset(idx as isize + 1),
                self.info.as_mut_ptr().offset(idx as isize),
                len - idx - 1,
            );
        }

        self.len -= 1;
        return item;
    }

    /// Splits the array in two at `idx`, returning the right part of the split.
    ///
    /// TODO: implement this more efficiently.
    pub fn split_off(&mut self, idx: usize) -> NodeChildren {
        assert!(idx <= self.len());

        let mut other = NodeChildren::new();
        let count = self.len() - idx;
        for _ in 0..count {
            other.push(self.remove(idx));
        }

        other
    }

    /// Gets references to the nth item's node and info.
    pub fn i(&self, n: usize) -> (&TextInfo, &Arc<Node>) {
        assert!(n < self.len());
        (
            &self.info[self.len as usize],
            &self.nodes[self.len as usize],
        )
    }

    /// Gets mut references to the nth item's node and info.
    pub fn i_mut(&mut self, n: usize) -> (&mut TextInfo, &mut Arc<Node>) {
        assert!(n < self.len());
        (
            &mut self.info[self.len as usize],
            &mut self.nodes[self.len as usize],
        )
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
        let (info1, info2) = self.info.split_at_mut(split_idx);
        let (nodes1, nodes2) = self.nodes.split_at_mut(split_idx);

        (
            (&mut info1[idx1], &mut nodes1[idx1]),
            (&mut info2[idx2 - split_idx], &mut nodes2[idx2 - split_idx]),
        )
    }

    /// Creates an iterator over the array's items.
    pub fn iter(&self) -> Zip<slice::Iter<TextInfo>, slice::Iter<Arc<Node>>> {
        Iterator::zip(
            (&self.info[..(self.len as usize)]).iter(),
            (&self.nodes[..(self.len as usize)]).iter(),
        )
    }

    /// Creates an iterator over the array's items.
    pub fn iter_mut(&mut self) -> Zip<slice::IterMut<TextInfo>, slice::IterMut<Arc<Node>>> {
        Iterator::zip(
            (&mut self.info[..(self.len as usize)]).iter_mut(),
            (&mut self.nodes[..(self.len as usize)]).iter_mut(),
        )
    }

    pub fn combined_info(&self) -> TextInfo {
        self.info[..self.len()]
            .iter()
            .fold(TextInfo::new(), |a, b| a + *b)
    }

    pub fn search_combine_info<F: Fn(&TextInfo) -> bool>(&self, pred: F) -> (usize, TextInfo) {
        let mut accum = TextInfo::new();
        for (idx, inf) in self.info[..self.len()].iter().enumerate() {
            if pred(&(accum + *inf)) {
                return (idx, accum);
            } else {
                accum += *inf;
            }
        }
        panic!("Predicate is mal-formed and never evaluated true.")
    }

    /// Returns the child index and accumulated text info to the left of the
    /// child that contains the give char.
    ///
    /// One-past-the end is valid, and will return the last child.
    pub fn search_char_idx(&self, char_idx: usize) -> (usize, TextInfo) {
        assert!(self.len() > 0);

        let mut accum = TextInfo::new();
        let mut idx = 0;
        for info in self.info[0..(self.len() - 1)].iter() {
            let next_accum = accum + *info;
            if char_idx < next_accum.chars as usize {
                break;
            }
            accum = next_accum;
            idx += 1;
        }

        assert!(
            char_idx <= (accum.chars + self.info[idx].chars) as usize,
            "Index out of bounds."
        );

        (idx, accum)
    }

    /// Returns the child indices at the start and end of the given char
    /// range, and returns their accumulated text info as well.
    ///
    /// One-past-the end is valid, and corresponds to the last child.
    pub fn search_char_idx_range(
        &self,
        start_idx: usize,
        end_idx: usize,
    ) -> ((usize, TextInfo), (usize, TextInfo)) {
        assert!(start_idx <= end_idx);
        assert!(self.len() > 0);

        let mut accum = TextInfo::new();
        let mut idx = 0;

        // Find left child and info
        for info in self.info[..(self.len() - 1)].iter() {
            let next_accum = accum + *info;
            if start_idx < next_accum.chars as usize {
                break;
            }
            accum = next_accum;
            idx += 1;
        }
        let l_child_i = idx;
        let l_acc_info = accum;

        // Find right child and info
        for info in self.info[idx..(self.len() - 1)].iter() {
            let next_accum = accum + *info;
            if end_idx <= next_accum.chars as usize {
                break;
            }
            accum = next_accum;
            idx += 1;
        }

        assert!(
            end_idx <= (accum.chars + self.info[idx].chars) as usize,
            "Index out of bounds."
        );

        ((l_child_i, l_acc_info), (idx, accum))
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
            .field("len", &self.len)
            .field("info", &&self.info[0..self.len()])
            .field("nodes", &&self.nodes[0..self.len()])
            .finish()
    }
}

impl Drop for NodeChildren {
    fn drop(&mut self) {
        for node in &mut self.nodes[..self.len as usize] {
            let mptr: *mut Arc<Node> = node; // Make sure we have the right dereference
            unsafe { ptr::drop_in_place(mptr) };
        }
    }
}

impl Clone for NodeChildren {
    fn clone(&self) -> NodeChildren {
        let mut clone_array = NodeChildren::new();

        // Copy nodes... carefully.
        for (clone_arc, arc) in Iterator::zip(
            clone_array.nodes[..self.len()].iter_mut(),
            self.nodes[..self.len()].iter(),
        ) {
            mem::forget(mem::replace(clone_arc, Arc::clone(arc)));
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
            for (a, b) in Iterator::zip(clone_array.iter(), self.iter()) {
                assert_eq!(a.0, b.0);
                assert!(Arc::ptr_eq(a.1, b.1));
            }
        }

        clone_array
    }
}

//===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tree::{Node, NodeText, TextInfo};

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
        assert_eq!(0, (at_0_0.0).1.chars);
        assert_eq!(0, (at_0_0.1).1.chars);

        assert_eq!(1, (at_6_6.0).0);
        assert_eq!(1, (at_6_6.1).0);
        assert_eq!(6, (at_6_6.0).1.chars);
        assert_eq!(6, (at_6_6.1).1.chars);

        assert_eq!(2, (at_12_12.0).0);
        assert_eq!(2, (at_12_12.1).0);
        assert_eq!(12, (at_12_12.0).1.chars);
        assert_eq!(12, (at_12_12.1).1.chars);

        assert_eq!(2, (at_18_18.0).0);
        assert_eq!(2, (at_18_18.1).0);
        assert_eq!(12, (at_18_18.0).1.chars);
        assert_eq!(12, (at_18_18.1).1.chars);

        let at_0_6 = children.search_char_idx_range(0, 6);
        let at_6_12 = children.search_char_idx_range(6, 12);
        let at_12_18 = children.search_char_idx_range(12, 18);

        assert_eq!(0, (at_0_6.0).0);
        assert_eq!(0, (at_0_6.1).0);
        assert_eq!(0, (at_0_6.0).1.chars);
        assert_eq!(0, (at_0_6.1).1.chars);

        assert_eq!(1, (at_6_12.0).0);
        assert_eq!(1, (at_6_12.1).0);
        assert_eq!(6, (at_6_12.0).1.chars);
        assert_eq!(6, (at_6_12.1).1.chars);

        assert_eq!(2, (at_12_18.0).0);
        assert_eq!(2, (at_12_18.1).0);
        assert_eq!(12, (at_12_18.0).1.chars);
        assert_eq!(12, (at_12_18.1).1.chars);

        let at_5_7 = children.search_char_idx_range(5, 7);
        let at_11_13 = children.search_char_idx_range(11, 13);

        assert_eq!(0, (at_5_7.0).0);
        assert_eq!(1, (at_5_7.1).0);
        assert_eq!(0, (at_5_7.0).1.chars);
        assert_eq!(6, (at_5_7.1).1.chars);

        assert_eq!(1, (at_11_13.0).0);
        assert_eq!(2, (at_11_13.1).0);
        assert_eq!(6, (at_11_13.0).1.chars);
        assert_eq!(12, (at_11_13.1).1.chars);
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
}
