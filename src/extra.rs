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

pub trait RopeExt {
    /// Returns true if this rope and `other` point to precisely the same
    /// in-memory data.
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
    fn is_instance(&self, other: &Self) -> bool;
}

impl RopeExt for Rope {
    #[inline]
    fn is_instance(&self, other: &Self) -> bool {
        match (&self.root, &other.root) {
            (Node::Internal(a), Node::Internal(b)) => Arc::ptr_eq(a, b),
            (Node::Leaf(a), Node::Leaf(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

pub trait RopeSliceExt {
    /// Creates a cheap, non-editable `Rope` from the `RopeSlice`.
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
    /// method.  However, a `Rope` from `Into<Rope>` will be a normal editable
    /// `Rope`, whereas `Rope`s produced from this method are read-only.
    ///
    /// **You probably don't need to use this method.**  Legitimate use cases
    /// for it are rare, and you should stick to normal `Rope`s and `RopeSlice`s
    /// when you can.
    ///
    /// Runs in O(1) time.  Space usage is constant unless the original `Rope`
    /// is edited, causing the otherwise shared contents to diverge.
    ///
    /// # Panics
    ///
    /// This method does not panic itself.  However, if edits are attempted
    /// on the resulting `Rope` with the panicking variants `insert()` and
    /// `remove()`, they will panic.
    fn to_owning_slice(&self) -> crate::Rope;
}

impl RopeSliceExt for RopeSlice<'_> {
    #[inline]
    fn to_owning_slice(&self) -> Rope {
        Rope {
            root: self.root.clone(),
            root_info: *self.root_info,
            owned_slice_byte_range: self.byte_range,
        }
    }
}
