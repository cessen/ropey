//! Less commonly needed and/or esoteric functionality.
//!
//! As a general rule, the functions provided by this module should be
//! treated with a little suspicion.  There are legitimate uses for them, which
//! is why they're provided.  But their use should be treated as at least a *bit*
//! of a code smell.
//!
//! Additionally, the functionality here has a worse benefit-to-footgun ratio
//! than the rest of Ropey, and should be used carefully even when it is
//! legitimately needed.

use std::sync::Arc;

use crate::{tree::Node, Rope, RopeSlice};

/// Returns true if both ropes point to precisely the same in-memory data.
///
/// This happens when one of the ropes is a clone of the other and
/// neither have been modified since then.  Because clones initially
/// share all the same data, it can be useful to check if they still
/// point to precisely the same memory as a way of determining
/// whether they are both still unmodified.
///
/// Note: this is distinct from checking for equality: two ropes can
/// have the same *contents* (equal) but be stored in different
/// memory locations (not instances).  Importantly, two clones that
/// post-cloning are modified identically will *not* be instances
/// anymore, even though they will have equal contents.
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

/// Creates a cheap, non-editable `Rope` from a `RopeSlice`.
///
/// The resulting `Rope` is guaranteed to not take up any additional
/// space itself beyond a small constant size, instead referencing the
/// original data.  The difference between this and a `RopeSlice` is that
/// this co-owns the data with the original `Rope` just like a `Rope`
/// clone would, and thus can be passed around freely (e.g. across thread
/// boundaries).  Additionally, its existence doesn't prevent the original
/// `Rope` from being edited, dropped, etc.
///
/// This is distinct from using `Into<Rope>` on a `RopeSlice`, which edits
/// the resulting `Rope`'s data to trim it to the range of the slice, which
/// is both more expensive and results in space overhead compared to this
/// function.  However, a `Rope` from `Into<Rope>` will be a normal editable
/// `Rope`, whereas `Rope`s produced from this function are read-only.
///
/// **You probably don't need to use this function.**  Legitimate use cases
/// for it are rare, and you should stick to normal `Rope`s and `RopeSlice`s
/// when you can.
///
/// Runs in O(1) time.  Space usage is constant unless the original `Rope`
/// is edited, causing the otherwise shared contents to diverge.
///
/// # Panics
///
/// This function does not panic itself.  However, if edits are attempted
/// on the resulting `Rope` with the panicking variants `insert()` and
/// `remove()`, they will panic.
pub fn slice_to_owning_slice(slice: RopeSlice) -> Rope {
    Rope {
        root: slice.root.clone(),
        root_info: *slice.root_info,
        owned_slice_byte_range: slice.byte_range,
    }
}

// TODO: unit tests.
