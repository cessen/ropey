//! Less commonly needed and/or esoteric functionality.
//!
//! **Warning:** the functions in this module expose you to esoterica of Ropey's
//! internal data model, and take you off the beaten path of Ropey's intended
//! API semantics.  They are nevertheless API promises and can be depended on to
//! continue functioning as documented.  However, you should **read their
//! documentation carefully** and make sure you fully understand exactly what
//! they do and don't promise before using them.

use std::sync::Arc;

use crate::{slice::SliceInner, tree::Node, Rope, RopeSlice};

/// Returns true if both ropes internally point to the same memory and share the
/// same content.
///
/// The API promises of this function are narrow and specific.  The following
/// two things and *only* the following two things are guaranteed:
///
/// 1. If rope A and rope B are *unmodified* clones of each other (i.e. no edits
///    have been made to either since cloning), then this function returns true.
///    This applies to both direct and indirect clones, since cloning is
///    transitive.
/// 2. If the text contents of rope A and rope B are different, then this
///    function returns false.
///
/// This function's return value may change between non-breaking releases in all
/// other cases.  For example: rope B is cloned from rope A, and then the same
/// edit is made to both ropes.  They have then both been modified since cloning
/// (not case 1), but they also compare equal (not case 2 either).
///
/// Runs in O(1) time.
pub fn ropes_are_instances(a: &Rope, b: &Rope) -> bool {
    if a.owned_slice_byte_range != b.owned_slice_byte_range {
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
/// exchange for this efficiency, there is the possibility of failure under some
/// circumstances.
///
/// Success is guaranteed for `RopeSlice`s of `Rope`s.  Whether this function
/// succeeds for `RopeSlice`s constructed in other ways (see e.g. `impl
/// From<&str> for RopeSlice`) is unspecified, and may change between
/// non-breaking releases.  On failure, returns `None`.
///
/// Like `Rope` cloning, runs in O(1) time, and the resulting `Rope` shares its
/// data with the `Rope` it comes from, taking up O(1) additional space until
/// edits are made to either one.
pub fn disconnect_slice(slice: RopeSlice) -> Option<Rope> {
    match slice {
        RopeSlice(SliceInner::Rope {
            root,
            root_info,
            byte_range,
        }) => Some(Rope {
            root: root.clone(),
            root_info: *root_info,
            owned_slice_byte_range: byte_range,
        }),

        RopeSlice(SliceInner::Str(_)) => None,
    }
}

// TODO: unit tests.
