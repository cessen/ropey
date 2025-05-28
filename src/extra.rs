pub trait RopeExt {
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

    // fn is_instance(&self, other: &Self) -> bool;
}
