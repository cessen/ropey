#![allow(dead_code)]

use std;
use std::iter::{Iterator, Zip};
use std::mem;
use std::mem::ManuallyDrop;
use std::ptr;
use std::slice;
use std::sync::Arc;

use node;
use node::Node;
use text_info::TextInfo;

const MAX_LEN: usize = node::MAX_CHILDREN;

#[derive(Debug)]
pub(crate) struct ChildArray {
    nodes: ManuallyDrop<[Arc<Node>; MAX_LEN]>,
    info: [TextInfo; MAX_LEN],
    len: u8,
}

impl ChildArray {
    /// Creates a new empty array.
    pub fn new() -> ChildArray {
        ChildArray {
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

    /// Pushes an item into the end of the array.
    ///
    /// Increases length by one.  Panics if already full.
    pub fn push(&mut self, item: (TextInfo, Arc<Node>)) {
        assert!((self.len as usize) < MAX_LEN);
        self.info[self.len as usize] = item.0;
        mem::forget(mem::replace(&mut self.nodes[self.len as usize], item.1));
        self.len += 1;
    }

    /// Pushes an element onto the end of the array, and then splits it in half,
    /// returning the right half.
    ///
    /// This works even when the array is full.
    pub fn push_split(&mut self, new_child: (TextInfo, Arc<Node>)) -> ChildArray {
        let mut right = ChildArray::new();

        let r_count = (self.len() + 1) / 2;
        let l_count = (self.len() + 1) - r_count;

        for _ in l_count..self.len() {
            right.push(self.remove(l_count));
        }
        right.push(new_child);

        right
    }

    /// Pops an item off the end of the array and returns it.
    ///
    /// Decreases length by one.  Panics if already empty.
    pub fn pop(&mut self) -> (TextInfo, Arc<Node>) {
        assert!(self.len > 0);
        self.len -= 1;
        let item = (self.info[self.len as usize], unsafe {
            ptr::read(&self.nodes[self.len as usize])
        });
        item
    }

    /// Inserts an item into the the array at the given index.
    ///
    /// Increases length by one.  Panics if already full.  Preserves ordering
    /// of the other items.
    pub fn insert(&mut self, idx: usize, item: (TextInfo, Arc<Node>)) {
        assert!(idx <= (self.len as usize));
        assert!((self.len as usize) < MAX_LEN);

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
    pub fn insert_split(&mut self, idx: usize, item: (TextInfo, Arc<Node>)) -> ChildArray {
        assert!(self.len() > 0);
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
        assert!(idx < (self.len as usize));

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

    /// Gets references to the nth item's node and info.
    pub fn i(&self, n: usize) -> (&TextInfo, &Arc<Node>) {
        assert!(n < self.len as usize);
        (
            &self.info[self.len as usize],
            &self.nodes[self.len as usize],
        )
    }

    /// Gets mut references to the nth item's node and info.
    pub fn i_mut(&mut self, n: usize) -> (&mut TextInfo, &mut Arc<Node>) {
        assert!(n < self.len as usize);
        (
            &mut self.info[self.len as usize],
            &mut self.nodes[self.len as usize],
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
        self.info[..self.len()].iter().fold(
            TextInfo::new(),
            |a, b| a.combine(b),
        )
    }

    pub fn search_combine_info<F: Fn(&TextInfo) -> bool>(&self, pred: F) -> (usize, TextInfo) {
        let mut accum = TextInfo::new();
        for (idx, inf) in self.info[..self.len()].iter().enumerate() {
            if pred(&accum.combine(inf)) {
                return (idx, accum);
            } else {
                accum = accum.combine(inf);
            }
        }
        panic!("Predicate is mal-formed and never evaluated true.")
    }
}

impl Drop for ChildArray {
    fn drop(&mut self) {
        for node in &mut self.nodes[..self.len as usize] {
            let mptr: *mut Arc<Node> = node; // Make sure we have the right dereference
            unsafe { ptr::drop_in_place(mptr) };
        }
    }
}

impl Clone for ChildArray {
    fn clone(&self) -> ChildArray {
        let mut clone_array = ChildArray::new();

        // Copy nodes... carefully.
        for (clone_arc, arc) in Iterator::zip(
            clone_array.nodes[..self.len()].iter_mut(),
            self.nodes[..self.len()].iter(),
        )
        {
            mem::forget(mem::replace(clone_arc, arc.clone()));
        }

        // Copy TextInfo
        for (clone_info, info) in
            Iterator::zip(
                clone_array.info[..self.len()].iter_mut(),
                self.info[..self.len()].iter(),
            )
        {
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
