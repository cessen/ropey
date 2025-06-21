//! Miscellaneous extra functionality.

pub mod esoterica {
    //! Esoteric functionality.
    //!
    //! **Warning:** the functions in this module expose you to esoterica of
    //! Ropey's internal data model, and take you off the beaten path of Ropey's
    //! intended API semantics.  They are nevertheless API promises and can be
    //! depended on to continue functioning as documented.  However, you should
    //! **read their documentation carefully** and make sure you fully understand
    //! exactly what they do/don't promise before using them.

    use std::sync::Arc;

    use crate::{slice::SliceInner, tree::Node, Rope, RopeSlice};

    /// Returns true if both ropes internally point to the same memory and share
    /// the same content.
    ///
    /// This function's API promises are very specific.  The following two things
    /// and *only* the following two things are guaranteed:
    ///
    /// 1. If rope `a` and rope `b` are *unmodified* clones of each other (i.e.
    ///    no edits have been made to either since cloning), then this function
    ///    returns true. This applies to both direct and indirect clones, since
    ///    cloning is transitive.
    /// 2. If the text contents of rope A and rope B are different, then this
    ///    function returns false.
    ///
    /// In all other cases, this function's return value is unspecified (may
    /// change between non-breaking releases) and should not be relied on for
    /// program correctness. An example of such a case: rope B is cloned from
    /// rope A, and then the same edit is made to both ropes.  They have then
    /// both been modified since cloning (not case 1), but they also compare
    /// equal (not case 2 either).
    ///
    /// Runs in O(1) time.
    pub fn ropes_are_instances(a: &Rope, b: &Rope) -> bool {
        if a.byte_range != b.byte_range {
            return false;
        }

        match (&a.root, &b.root) {
            (Node::Internal(a_root), Node::Internal(b_root)) => Arc::ptr_eq(a_root, b_root),
            (Node::Leaf(a_root), Node::Leaf(b_root)) => Arc::ptr_eq(a_root, b_root),
            _ => false,
        }
    }

    /// Disconnects a `RopeSlice` from its originating `Rope`, creating a new
    /// independent `Rope` in O(1) time.
    ///
    /// This function is like `Into<Rope>` (the normal way to make `Rope`s from
    /// `RopeSlice`s), but with the time/space complexity of `Rope` cloning.  In
    /// exchange for this efficiency, there is the possibility of failure under
    /// some circumstances.  On failure, returns `None`.
    ///
    /// Success is guaranteed for `RopeSlice`s created from `Rope`s or
    /// from `Rope`-derived `RopeSlice`s.  Whether this function succeeds
    /// on `RopeSlice`s constructed in other ways (namely `From<&str>`) is
    /// unspecified (may change between non-breaking releases) and should not be
    /// relied on for program correctness.
    ///
    /// Like `Rope` cloning, runs in O(1) time, and the resulting `Rope` shares
    /// its data with the originating `Rope`, taking up O(1) additional space
    /// until edits are made to either one.
    pub fn disconnect_slice(slice: RopeSlice) -> Option<Rope> {
        match slice {
            RopeSlice(SliceInner::Rope {
                root,
                root_info,
                byte_range,
            }) => Some(Rope {
                root: root.clone(),
                root_info: *root_info,
                byte_range: byte_range,
            }),

            RopeSlice(SliceInner::Str(_)) => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ropes_are_instances_01() {
            let r1 = Rope::from_str("Hello there!");
            let r2 = r1.clone();

            assert!(ropes_are_instances(&r1, &r2));
        }

        #[test]
        fn ropes_are_instances_02() {
            let r1 = Rope::from_str("Hello there!");
            let r2 = {
                let mut r2 = r1.clone();
                r2.insert(0, "a");
                r2
            };
            let r3 = {
                let s = r1.slice(1..);
                disconnect_slice(s).unwrap()
            };

            assert!(!ropes_are_instances(&r1, &r2));
            assert!(!ropes_are_instances(&r1, &r3));
        }

        #[test]
        fn disconnect_slice_01() {
            let r = Rope::from_str("Hello there!");

            let s1 = r.slice(..);
            let s2 = r.slice(1..);
            let s3 = r.slice(..8);
            let s4 = r.slice(1..8);
            assert_eq!(s1, "Hello there!");
            assert_eq!(s2, "ello there!");
            assert_eq!(s3, "Hello th");
            assert_eq!(s4, "ello th");

            let r1 = disconnect_slice(s1).unwrap();
            let r2 = disconnect_slice(s2).unwrap();
            let r3 = disconnect_slice(s3).unwrap();
            let r4 = disconnect_slice(s4).unwrap();
            assert_eq!(r1, "Hello there!");
            assert_eq!(r2, "ello there!");
            assert_eq!(r3, "Hello th");
            assert_eq!(r4, "ello th");
        }

        #[test]
        fn disconnect_slice_02() {
            let r = Rope::from_str("Hello there!");
            let s = r.slice(1..8);

            let mut r1 = disconnect_slice(s).unwrap();
            assert_eq!(r1, "ello th");

            r1.insert(0, "F");
            r1.insert(5, "w");
            r1.insert(9, "ing!");
            assert_eq!(r1, "Fellow thing!");
        }
    }
}
