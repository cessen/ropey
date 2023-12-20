use super::{node::Node, text_info::TextInfo, MAX_CHILDREN};

/// Internal node of the Rope, with other nodes as children.
#[derive(Debug, Clone)]
pub(crate) struct Children(inner::ChildrenInternal);

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
        nodes: [MaybeUninit<Arc<Node>>; MAX_CHILDREN],
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
        pub fn nodes(&self) -> &[Arc<Node>] {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the nodes from `0..len` are guaranteed to be initialized
            unsafe { mem::transmute(&self.nodes[..(self.len())]) }
        }

        /// Mutable access to the nodes array.
        #[inline(always)]
        pub fn nodes_mut(&mut self) -> &mut [Arc<Node>] {
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
        pub fn data_mut(&mut self) -> (&mut [TextInfo], &mut [Arc<Node>]) {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the info from `0..len` are guaranteed to be initialized
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
        pub fn pop(&mut self) -> (TextInfo, Arc<Node>) {
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
        pub fn insert(&mut self, idx: usize, item: (TextInfo, Arc<Node>)) {
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
        pub fn remove(&mut self, idx: usize) -> (TextInfo, Arc<Node>) {
            assert!(self.len() > 0);
            assert!(idx < self.len());

            // Read out the item.
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
                unsafe { ptr::drop_in_place(node.as_mut_ptr()) };
            }
        }
    }

    impl Clone for ChildrenInternal {
        fn clone(&self) -> ChildrenInternal {
            // Create an empty ChildrenInternal first, then fill it
            let mut clone_array = ChildrenInternal::new();

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
