// use std::io;
// use std::iter::FromIterator;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use crate::{
    end_bound_to_num,
    iter::{Bytes, Chars, Chunks},
    rope_builder::RopeBuilder,
    slice::RopeSlice,
    start_bound_to_num,
    tree::{Children, Node, Text, TextInfo, MAX_TEXT_SIZE},
    Error::*,
    Result,
};

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
use crate::{iter::Lines, LineType};

/// A utf8 text rope.
///
/// The time complexity of nearly all edit and query operations on `Rope` are
/// worst-case `O(log N)` in the length of the rope.  `Rope` is designed to work
/// efficiently even for huge (in the gigabytes) and pathological (all on one
/// line) texts.
///
/// # Editing Operations
///
/// The editing operations on `Rope` are insertion and removal of text.  For
/// example:
///
/// ```
/// # use ropey::Rope;
/// #
/// let mut rope = Rope::from_str("Hello みんなさん!");
/// rope.remove(6..21);
/// rope.insert(6, "world");
///
/// assert_eq!(rope, "Hello world!");
/// ```
///
/// # Query Operations
///
/// `Rope` provides a rich set of efficient query functions, including querying
/// rope length in bytes/`char`s/lines, fetching individual `char`s or lines,
/// and converting between byte/`char`/line indices.
///
#[cfg_attr(
    feature = "metric_lines_lf_cr",
    doc = r##"
For example, to find the starting byte index of a given line:

```
# use ropey::Rope;
use ropey::LineType::LF_CR;

let rope = Rope::from_str("Hello みんなさん!\nHow are you?\nThis text has multiple lines!");

assert_eq!(rope.line_to_byte(0, LF_CR), 0);
assert_eq!(rope.line_to_byte(1, LF_CR), 23);
assert_eq!(rope.line_to_byte(2, LF_CR), 36);
```

# Slicing

You can take immutable slices of a `Rope` using `slice()`:

```
# use ropey::Rope;
#
let mut rope = Rope::from_str("Hello みんなさん!");
let middle = rope.slice(3..12);

assert_eq!(middle, "lo みん");
```
"##
)]
///
/// # Cloning
///
/// Cloning `Rope`s is extremely cheap, running in `O(1)` time and taking a
/// small constant amount of memory for the new clone, regardless of text size.
/// This is accomplished by data sharing between `Rope` clones.  The memory
/// used by clones only grows incrementally as the their contents diverge due
/// to edits.  All of this is thread safe, so clones can be sent freely
/// between threads.
///
/// The primary intended use-case for this feature is to allow asynchronous
/// processing of `Rope`s.  For example, saving a large document to disk in a
/// separate thread while the user continues to perform edits.
#[derive(Clone)]
pub struct Rope {
    pub(crate) root: Node,
    pub(crate) root_info: TextInfo,

    // Normally just set to the full range of `root`, but can be used to create
    // ropes that behave as "owned slices".
    pub(crate) owned_slice_byte_range: [usize; 2],
}

impl Rope {
    //---------------------------------------------------------
    // Constructors.

    /// Creates an empty `Rope`.
    #[inline]
    pub fn new() -> Self {
        Rope {
            root: Node::Leaf(Arc::new(Text::new())),
            root_info: TextInfo::new(),
            owned_slice_byte_range: [0; 2],
        }
    }

    /// Creates a `Rope` with the contents of `text`.
    #[inline]
    pub fn from_str(text: &str) -> Self {
        RopeBuilder::new().build_at_once(text)
    }

    //-----------------------------------------------------------------------
    // Edit methods

    /// Inserts `text` at byte index `byte_idx`.
    ///
    /// Runs in O(M log N) time, where N is the length of the `Rope` and M
    /// is the length of `text`.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::Rope;
    /// let mut rope = Rope::from_str("Hello!");
    /// rope.insert(5, " world");
    ///
    /// assert_eq!("Hello world!", rope);
    /// ```
    ///
    /// # Panics
    ///
    /// - If `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    /// - If `byte_idx` is not on a char boundary.
    #[inline]
    pub fn insert(&mut self, byte_idx: usize, text: &str) {
        match self.try_insert(byte_idx, text) {
            Ok(_) => {}
            Err(e) => e.panic_with_msg(),
        }
    }

    #[inline]
    pub fn insert_char(&mut self, byte_idx: usize, ch: char) {
        let mut buf = [0u8; 4];
        self.insert(byte_idx, ch.encode_utf8(&mut buf));
    }

    /// Removes the text in the given byte index range.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.
    ///
    /// Runs in O(M + log N) time, where N is the length of the `Rope` and M
    /// is the length of the range being removed.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::Rope;
    /// let mut rope = Rope::from_str("Hello world!");
    /// rope.remove(5..);
    ///
    /// assert_eq!("Hello", rope);
    /// ```
    ///
    /// # Panics
    ///
    /// - If the start of the range is greater than the end.
    /// - If the end of the range is out of bounds (i.e. `end > len_bytes()`).
    /// - If the range ends are not on char boundaries.
    #[inline]
    pub fn remove<R>(&mut self, byte_range: R)
    where
        R: RangeBounds<usize>,
    {
        match self.try_remove(byte_range) {
            Ok(_) => {}
            Err(e) => e.panic_with_msg(),
        }
    }

    //---------------------------------------------------------
    // Slicing.

    #[inline]
    pub fn slice<'a, R>(&'a self, byte_range: R) -> RopeSlice<'a>
    where
        R: RangeBounds<usize>,
    {
        match self.try_slice(byte_range) {
            Ok(slice) => slice,
            Err(e) => e.panic_with_msg(),
        }
    }

    //---------------------------------------------------------
    // Methods shared between Rope and RopeSlice.

    crate::shared_impl::shared_main_impl_methods!();

    //---------------------------------------------------------
    // Misc. internal methods.

    /// Iteratively replaces the root node with its child if it only has
    /// one child.
    pub(crate) fn pull_up_singular_nodes(&mut self) {
        while (!self.root.is_leaf()) && self.root.child_count() == 1 {
            let child = if let Node::Internal(ref children) = self.root {
                children.nodes()[0].clone()
            } else {
                unreachable!()
            };

            self.root = child;
        }
    }

    //---------------------------------------------------------
    // Debugging and testing helpers.

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_invariants(&self) {
        self.assert_equal_leaf_depth();
        self.assert_no_empty_internal();
        self.assert_no_empty_non_root_leaf();
        self.assert_accurate_text_info();
        self.assert_accurate_unbalance_flags();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_equal_leaf_depth(&self) {
        self.root.assert_equal_leaf_depth();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_no_empty_internal(&self) {
        self.root.assert_no_empty_internal();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_no_empty_non_root_leaf(&self) {
        if self.root.is_leaf() {
            // The root is allowed to be empty if it's a leaf.
            return;
        }
        self.root.assert_no_empty_leaf();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_accurate_text_info(&self) {
        assert!(self.root_info == self.root.assert_accurate_text_info());
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_accurate_unbalance_flags(&self) {
        self.root.assert_accurate_unbalance_flags();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    ///
    /// Attempts to fully rebalance the tree within `max_iterations`.
    ///
    /// Returns whether it fully rebalanced the tree and the actual number of
    /// iterations done.
    #[doc(hidden)]
    pub fn attempt_full_rebalance(&mut self, max_iterations: usize) -> (bool, usize) {
        let mut iter_count = 0;

        while self.root.is_subtree_unbalanced() {
            if iter_count >= max_iterations {
                return (false, iter_count);
            }

            self.root.partial_rebalance();
            self.pull_up_singular_nodes();
            iter_count += 1;
        }

        return (true, iter_count);
    }

    //---------------------------------------------------------
    // Utility methods needed by the shared impl macros in
    // `crate::shared_impl`.

    #[inline(always)]
    fn get_root(&self) -> &Node {
        &self.root
    }

    #[inline(always)]
    fn get_root_info(&self) -> &TextInfo {
        &self.root_info
    }

    #[inline(always)]
    fn get_full_info(&self) -> Option<&TextInfo> {
        if self.owned_slice_byte_range[0] == 0
            && self.owned_slice_byte_range[1] == self.root_info.bytes
        {
            Some(&self.root_info)
        } else {
            None
        }
    }

    #[inline(always)]
    fn get_byte_range(&self) -> [usize; 2] {
        self.owned_slice_byte_range
    }
}

//=============================================================
// Non-panicking versions.

/// Non-panicking versions of some of `Rope`'s methods.
impl Rope {
    pub fn try_insert(&mut self, byte_idx: usize, text: &str) -> Result<()> {
        // TODO: this probably isn't really what we want to do for owned slices.
        // Better would be to convert it to a proper rope by trimming the ends
        // first.
        if self.owned_slice_byte_range[0] != 0
            || self.owned_slice_byte_range[1] != self.root_info.bytes
        {
            return Err(CannotEditOwnedSlice);
        }

        if byte_idx > self.len_bytes() {
            return Err(OutOfBounds);
        }

        // The `Node` insertion method already checks if the byte index is
        // a non-char boundary and returns the appropriate error, but that
        // method never gets called if the text is empty.  So we need to check
        // that here.  This is a bit pedantic, because inserting nothing at a
        // non-char-boundary doesn't really mean anything.  But the behavior is
        // consistent this way, and might help catch bugs in client code.
        if text.is_empty() && !self.is_char_boundary(byte_idx) {
            return Err(NonCharBoundary);
        }

        // We have two cases here:
        //
        // 1. The insertion text is small enough to fit in a single node.
        // 2. The insertion text is larger than a single node can hold.
        //
        // Case #1 is easy to handle: it's just a standard insertion.  However,
        // case #2 needs more careful handling.  We handle case #2 by splitting
        // the insertion text into node-sized chunks and repeatedly inserting
        // them.
        //
        // In practice, both cases are rolled into one here, where case #1 is
        // just a special case that naturally falls out of the handling of
        // case #2.
        let mut text = text;
        while !text.is_empty() {
            // Split a chunk off from the end of the text.
            // We do this from the end instead of the front so that the repeated
            // insertions can keep re-using the same insertion point.
            //
            // NOTE: the chunks are at most `MAX_TEXT_SIZE - 4` rather than just
            // `MAX_TEXT_SIZE` to guarantee that nodes can split into node-sized
            // chunks even in the face of multi-byte chars that may prevent
            // splits at certain byte indices.  This is a subtle issue that in
            // practice only very rarely manifest, but causes panics when it
            // does.  Please do not remove that `- 4`!
            let split_idx = crate::find_char_boundary_r(
                text.len() - (MAX_TEXT_SIZE - 4).min(text.len()),
                text.as_bytes(),
            );
            let ins_text = &text[split_idx..];
            text = &text[..split_idx];

            // Do the insertion.
            let (new_root_info, residual) =
                self.root
                    .insert_at_byte_idx(byte_idx, ins_text, self.root_info)?;
            self.root_info = new_root_info;

            // Handle root split.
            if let Some((right_info, right_node)) = residual {
                let mut left_node = Node::Internal(Arc::new(Children::new()));
                std::mem::swap(&mut left_node, &mut self.root);

                let children = self.root.children_mut();
                children.push((self.root_info, left_node));
                children.push((right_info, right_node));
                self.root_info = children.combined_text_info();
            }

            // Do a rebalancing step.
            self.root.partial_rebalance();
            self.pull_up_singular_nodes();
        }

        self.owned_slice_byte_range[1] = self.root_info.bytes;

        Ok(())
    }

    #[inline]
    pub fn try_insert_char(&mut self, byte_idx: usize, ch: char) -> Result<()> {
        let mut buf = [0u8; 4];
        self.try_insert(byte_idx, ch.encode_utf8(&mut buf))
    }

    #[inline]
    pub fn try_remove<R>(&mut self, byte_range: R) -> Result<()>
    where
        R: RangeBounds<usize>,
    {
        // Inner function to avoid code duplication on code gen due to the
        // generic type of `byte_range`.
        fn inner(rope: &mut Rope, start: Bound<&usize>, end: Bound<&usize>) -> Result<()> {
            // TODO: this probably isn't really what we want to do for owned
            // slices. Better would be to convert it to a proper rope by
            // trimming the ends first.
            if rope.owned_slice_byte_range[0] != 0
                || rope.owned_slice_byte_range[1] != rope.root_info.bytes
            {
                return Err(CannotEditOwnedSlice);
            }

            let start_idx = start_bound_to_num(start).unwrap_or(0);
            let end_idx = end_bound_to_num(end).unwrap_or_else(|| rope.len_bytes());

            if start_idx > end_idx {
                return Err(InvalidRange);
            }

            if end_idx > rope.len_bytes() {
                return Err(OutOfBounds);
            }

            // Special case: if we're removing everything, just replace with a
            // fresh new rope.  This is to ensure the invariant that an empty
            // rope is always composed of a single empty leaf, which is not
            // ensured by the general removal code.
            if start_idx == 0 && end_idx == rope.len_bytes() {
                *rope = Rope::new();
                return Ok(());
            }

            let new_info = rope
                .root
                .remove_byte_range([start_idx, end_idx], rope.root_info)?;
            rope.root_info = new_info;

            // Do a rebalancing step.
            rope.root.partial_rebalance();
            rope.pull_up_singular_nodes();

            rope.owned_slice_byte_range[1] = rope.root_info.bytes;

            Ok(())
        }

        inner(self, byte_range.start_bound(), byte_range.end_bound())
    }

    #[inline]
    pub fn try_slice<'a, R>(&'a self, byte_range: R) -> Result<RopeSlice<'a>>
    where
        R: RangeBounds<usize>,
    {
        let start_idx = start_bound_to_num(byte_range.start_bound()).unwrap_or(0);
        let end_idx = end_bound_to_num(byte_range.end_bound()).unwrap_or_else(|| self.len_bytes());

        fn inner<'a>(rope: &'a Rope, start_idx: usize, end_idx: usize) -> Result<RopeSlice<'a>> {
            if !rope.is_char_boundary(start_idx) || !rope.is_char_boundary(end_idx) {
                return Err(NonCharBoundary);
            }
            if start_idx > end_idx {
                return Err(InvalidRange);
            }
            if end_idx > rope.len_bytes() {
                return Err(OutOfBounds);
            }

            let start_idx_real = rope.get_byte_range()[0] + start_idx;
            let end_idx_real = rope.get_byte_range()[0] + end_idx;

            Ok(RopeSlice::new(
                rope.get_root(),
                rope.get_root_info(),
                [start_idx_real, end_idx_real],
            ))
        }

        inner(self, start_idx, end_idx)
    }

    // Methods shared between Rope and RopeSlice.
    crate::shared_impl::shared_no_panic_impl_methods!();
}

//==============================================================
// Stdlib trait impls.
//
// Note: most impls are in `shared_impls.rs`.  The only ones here are the ones
// that need to distinguish between Rope and RopeSlice.

// Impls shared between Rope and RopeSlice.
crate::shared_impl::shared_std_impls!(Rope);

impl std::default::Default for Rope {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl std::cmp::PartialEq<RopeSlice<'_>> for Rope {
    fn eq(&self, other: &RopeSlice) -> bool {
        RopeSlice::from(self) == *other
    }
}

impl From<RopeSlice<'_>> for Rope {
    fn from(rs: RopeSlice) -> Rope {
        let mut rb = RopeBuilder::new();
        for chunk in rs.chunks() {
            rb.append(chunk);
        }
        rb.finish()
    }
}

impl From<String> for Rope {
    fn from(s: String) -> Rope {
        Rope::from_str(&s)
    }
}

impl<'a> From<&'a str> for Rope {
    fn from(s: &'a str) -> Rope {
        Rope::from_str(s)
    }
}

impl<'a> From<std::borrow::Cow<'a, str>> for Rope {
    #[inline]
    fn from(s: std::borrow::Cow<'a, str>) -> Self {
        Rope::from_str(&s)
    }
}

impl<'a> FromIterator<&'a str> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = &'a str>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(chunk);
        }
        builder.finish()
    }
}

impl<'a> FromIterator<std::borrow::Cow<'a, str>> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = std::borrow::Cow<'a, str>>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(&chunk);
        }
        builder.finish()
    }
}

impl FromIterator<String> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = String>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(&chunk);
        }
        builder.finish()
    }
}

impl From<Rope> for std::borrow::Cow<'_, str> {
    /// Consumes the Rope, turning it into an owned `Cow<str>`.
    #[inline]
    fn from(r: Rope) -> Self {
        std::borrow::Cow::Owned(String::from(r))
    }
}

//=============================================================

#[cfg(test)]
mod tests {
    use std::hash::{Hash, Hasher};

    use crate::rope_builder::RopeBuilder;

    use super::*;

    // 127 bytes, 103 chars, 1 line
    const TEXT: &str = "Hello there!  How're you doing?  It's \
                        a fine day, isn't it?  Aren't you glad \
                        we're alive?  こんにちは、みんなさん！";

    // 124 bytes, 100 chars, 4 lines
    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    // 143 bytes, 107 chars, 111 utf16 code units, 1 line
    #[cfg(feature = "metric_utf16")]
    const TEXT_EMOJI: &str = "Hello there!🐸  How're you doing?🐸  It's \
                              a fine day, isn't it?🐸  Aren't you glad \
                              we're alive?🐸  こんにちは、みんなさん！";

    /// Note: ensures that the chunks as given become individual leaf nodes in
    /// the rope.
    fn make_rope_and_text_from_chunks(chunks: &[&str]) -> (Rope, String) {
        let rope = {
            let mut rb = RopeBuilder::new();
            for chunk in chunks {
                rb._append_chunk_as_leaf(chunk);
            }
            rb.finish()
        };
        let text = {
            let mut text = String::new();
            for chunk in chunks {
                text.push_str(chunk);
            }
            text
        };

        (rope, text)
    }

    #[test]
    fn insert_01() {
        let mut r = Rope::from_str(TEXT);
        r.insert(3, "AA");

        assert_eq!(
            r,
            "HelAAlo there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_invariants();
    }

    #[test]
    fn insert_02() {
        let mut r = Rope::from_str(TEXT);
        r.insert(0, "AA");

        assert_eq!(
            r,
            "AAHello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_invariants();
    }

    #[test]
    fn insert_03() {
        let mut r = Rope::from_str(TEXT);
        r.insert(127, "AA");

        assert_eq!(
            r,
            "Hello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！AA"
        );

        r.assert_invariants();
    }

    #[test]
    fn insert_04() {
        let mut r = Rope::from_str(TEXT);
        r.insert(3, "");

        assert_eq!(
            r,
            "Hello there!  How're you doing?  It's \
             a fine day, isn't it?  Aren't you glad \
             we're alive?  こんにちは、みんなさん！"
        );

        r.assert_invariants();
    }

    #[test]
    fn insert_05() {
        let mut r = Rope::new();
        r.insert(0, "He");
        r.insert(2, "l");
        r.insert(3, "l");
        r.insert(4, "o w");
        r.insert(7, "o");
        r.insert(8, "rl");
        r.insert(10, "d!");
        r.insert(3, "zopter");

        assert_eq!("Helzopterlo world!", r);

        r.assert_invariants();
    }

    #[test]
    fn insert_06() {
        let mut r = Rope::new();
        r.insert(0, "こんいちは、みんなさん！");
        r.insert(21, "zopter");
        assert_eq!("こんいちは、みzopterんなさん！", r);

        r.assert_invariants();
    }

    #[test]
    fn insert_07() {
        let mut r = Rope::new();
        r.insert(0, "こ");
        r.insert(3, "ん");
        r.insert(6, "い");
        r.insert(9, "ち");
        r.insert(12, "は");
        r.insert(15, "、");
        r.insert(18, "み");
        r.insert(21, "ん");
        r.insert(24, "な");
        r.insert(27, "さ");
        r.insert(30, "ん");
        r.insert(33, "！");
        r.insert(21, "zopter");
        assert_eq!("こんいちは、みzopterんなさん！", r);

        r.assert_invariants();
    }

    #[test]
    #[should_panic]
    fn insert_08() {
        let mut r = Rope::from_str(TEXT);
        // Out of bounds.
        r.insert(128, "A");
    }

    #[test]
    #[should_panic]
    fn insert_09() {
        let mut r = Rope::from_str(TEXT);
        // Out of bounds.
        r.insert(128, "");
    }

    #[test]
    #[should_panic]
    fn insert_10() {
        let mut r = Rope::from_str(TEXT);
        // Non-char boundary.
        r.insert(126, "A");
    }

    #[test]
    #[should_panic]
    fn insert_11() {
        let mut r = Rope::from_str(TEXT);
        // Non-char boundary.
        r.insert(126, "");
    }

    #[test]
    fn insert_12() {
        let (r, _) = make_rope_and_text_from_chunks(&["\r", "\n\r", "\n\r", "\n"]);

        {
            let mut r = r.clone();
            r.insert(1, "\r");
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(1, "\n");
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(3, "\r");
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(3, "\n");
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(0, "\n");
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(r.len_bytes(), "\r");
            r.assert_accurate_text_info();
        }
    }

    #[test]
    fn remove_01() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(0..4);
        rope.remove(5..7);
        rope.remove(28..37);
        rope.remove(35..109);

        assert_eq!(rope, "o the!  How're you doing?  Ie day, ！");
    }

    #[test]
    fn remove_02() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(..42);

        assert_eq!(
            rope,
            "ne day, isn't it?  Aren't you glad we're \
             alive?  こんにちは、みんなさん！"
        );
    }

    #[test]
    fn remove_03() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(42..);

        assert_eq!(rope, "Hello there!  How're you doing?  It's a fi");
    }

    #[test]
    fn remove_04() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(..);

        assert_eq!(rope, "");
    }

    #[test]
    fn remove_05() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(42..42);

        assert_eq!(rope, TEXT);
    }

    #[test]
    #[should_panic]
    fn remove_06() {
        let mut rope = Rope::from_str(TEXT);
        // Out of bounds.
        rope.remove(42..128);
    }

    #[test]
    #[should_panic]
    fn remove_07() {
        let mut rope = Rope::from_str(TEXT);
        // Out of bounds.
        rope.remove(128..128);
    }

    #[test]
    #[should_panic]
    fn remove_08() {
        let mut rope = Rope::from_str(TEXT);
        // Non-char boundary.
        rope.remove(42..126);
    }

    #[test]
    #[should_panic]
    fn remove_09() {
        let mut rope = Rope::from_str(TEXT);
        // Non-char boundary.
        rope.remove(126..127);
    }

    #[test]
    #[should_panic]
    fn remove_10() {
        let mut rope = Rope::from_str(TEXT);
        // Non-char boundary.
        rope.remove(126..126);
    }

    #[test]
    #[should_panic]
    fn remove_11() {
        let mut rope = Rope::from_str(TEXT);
        // Invalid range.
        rope.remove(42..21);
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn byte_to_char_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.byte_to_char(0));
        assert_eq!(1, r.byte_to_char(1));
        assert_eq!(2, r.byte_to_char(2));

        assert_eq!(91, r.byte_to_char(91));
        assert_eq!(91, r.byte_to_char(92));
        assert_eq!(91, r.byte_to_char(93));

        assert_eq!(92, r.byte_to_char(94));
        assert_eq!(92, r.byte_to_char(95));
        assert_eq!(92, r.byte_to_char(96));

        assert_eq!(102, r.byte_to_char(124));
        assert_eq!(102, r.byte_to_char(125));
        assert_eq!(102, r.byte_to_char(126));
        assert_eq!(103, r.byte_to_char(127));
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn char_to_byte_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.char_to_byte(0));
        assert_eq!(1, r.char_to_byte(1));
        assert_eq!(2, r.char_to_byte(2));

        assert_eq!(91, r.char_to_byte(91));
        assert_eq!(94, r.char_to_byte(92));
        assert_eq!(97, r.char_to_byte(93));
        assert_eq!(100, r.char_to_byte(94));

        assert_eq!(124, r.char_to_byte(102));
        assert_eq!(127, r.char_to_byte(103));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_01() {
        let r = Rope::from_str(TEXT_EMOJI);

        assert_eq!(0, r.byte_to_utf16(0));

        assert_eq!(12, r.byte_to_utf16(12));
        assert_eq!(12, r.byte_to_utf16(13));
        assert_eq!(14, r.byte_to_utf16(16));

        assert_eq!(33, r.byte_to_utf16(35));
        assert_eq!(33, r.byte_to_utf16(36));
        assert_eq!(35, r.byte_to_utf16(39));

        assert_eq!(63, r.byte_to_utf16(67));
        assert_eq!(63, r.byte_to_utf16(70));
        assert_eq!(65, r.byte_to_utf16(71));

        assert_eq!(95, r.byte_to_utf16(101));
        assert_eq!(95, r.byte_to_utf16(102));
        assert_eq!(97, r.byte_to_utf16(105));

        assert_eq!(111, r.byte_to_utf16(143));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_01() {
        let r = Rope::from_str(TEXT_EMOJI);

        assert_eq!(0, r.utf16_to_byte(0));

        assert_eq!(12, r.utf16_to_byte(12));
        assert_eq!(16, r.utf16_to_byte(14));

        assert_eq!(35, r.utf16_to_byte(33));
        assert_eq!(39, r.utf16_to_byte(35));

        assert_eq!(67, r.utf16_to_byte(63));
        assert_eq!(71, r.utf16_to_byte(65));

        assert_eq!(101, r.utf16_to_byte(95));
        assert_eq!(105, r.utf16_to_byte(97));

        assert_eq!(143, r.utf16_to_byte(111));
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn byte_to_line_01() {
        let r = Rope::from_str(TEXT_LINES);
        let byte_to_line_idxs = &[
            [0, 0],
            [1, 0],
            [31, 0],
            [32, 1],
            [33, 1],
            [58, 1],
            [59, 2],
            [60, 2],
            [87, 2],
            [88, 3],
            [89, 3],
            [124, 3],
        ];
        for [b, l] in byte_to_line_idxs.iter().copied() {
            #[cfg(feature = "metric_lines_lf")]
            assert_eq!(l, r.byte_to_line(b, LineType::LF));
            #[cfg(feature = "metric_lines_lf_cr")]
            assert_eq!(l, r.byte_to_line(b, LineType::LF_CR));
            #[cfg(feature = "metric_lines_unicode")]
            assert_eq!(l, r.byte_to_line(b, LineType::All));
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn byte_to_line_02() {
        let r = Rope::from_str("");

        #[cfg(feature = "metric_lines_lf")]
        assert_eq!(0, r.byte_to_line(0, LineType::LF));
        #[cfg(feature = "metric_lines_lf_cr")]
        assert_eq!(0, r.byte_to_line(0, LineType::LF_CR));
        #[cfg(feature = "metric_lines_unicode")]
        assert_eq!(0, r.byte_to_line(0, LineType::All));
    }

    #[cfg(feature = "metric_lines_lf")]
    #[test]
    #[should_panic]
    fn byte_to_line_03() {
        let r = Rope::from_str(TEXT_LINES);
        r.byte_to_line(125, LineType::LF);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[should_panic]
    fn byte_to_line_04() {
        let r = Rope::from_str(TEXT_LINES);
        r.byte_to_line(125, LineType::LF_CR);
    }

    #[cfg(feature = "metric_lines_unicode")]
    #[test]
    #[should_panic]
    fn byte_to_line_05() {
        let r = Rope::from_str(TEXT_LINES);
        r.byte_to_line(125, LineType::All);
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn line_to_byte_01() {
        let r = Rope::from_str(TEXT_LINES);
        let byte_to_line_idxs = &[[0, 0], [32, 1], [59, 2], [88, 3], [124, 4]];
        for [b, l] in byte_to_line_idxs.iter().copied() {
            #[cfg(feature = "metric_lines_lf")]
            assert_eq!(b, r.line_to_byte(l, LineType::LF));
            #[cfg(feature = "metric_lines_lf_cr")]
            assert_eq!(b, r.line_to_byte(l, LineType::LF_CR));
            #[cfg(feature = "metric_lines_unicode")]
            assert_eq!(b, r.line_to_byte(l, LineType::All));
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn line_to_byte_02() {
        let r = Rope::from_str("");
        #[cfg(feature = "metric_lines_lf")]
        {
            assert_eq!(0, r.line_to_byte(0, LineType::LF));
            assert_eq!(0, r.line_to_byte(1, LineType::LF));
        }
        #[cfg(feature = "metric_lines_lf_cr")]
        {
            assert_eq!(0, r.line_to_byte(0, LineType::LF_CR));
            assert_eq!(0, r.line_to_byte(1, LineType::LF_CR));
        }
        #[cfg(feature = "metric_lines_unicode")]
        {
            assert_eq!(0, r.line_to_byte(0, LineType::All));
            assert_eq!(0, r.line_to_byte(1, LineType::All));
        }
    }

    #[cfg(feature = "metric_lines_lf")]
    #[test]
    #[should_panic]
    fn line_to_byte_03() {
        let r = Rope::from_str(TEXT_LINES);
        r.line_to_byte(5, LineType::LF);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[should_panic]
    fn line_to_byte_04() {
        let r = Rope::from_str(TEXT_LINES);
        r.line_to_byte(5, LineType::LF_CR);
    }

    #[cfg(feature = "metric_lines_unicode")]
    #[test]
    #[should_panic]
    fn line_to_byte_05() {
        let r = Rope::from_str(TEXT_LINES);
        r.line_to_byte(5, LineType::All);
    }

    #[test]
    fn hash_01() {
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        let r1 = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk_as_leaf("Hello ");
            rb._append_chunk_as_leaf("world!");
            rb.finish()
        };
        let r2 = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk_as_leaf("Hell");
            rb._append_chunk_as_leaf("o world!");
            rb.finish()
        };

        r1.hash(&mut h1);
        r2.hash(&mut h2);

        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn hash_02() {
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        let r1 = Rope::from_str("Hello there!");
        let r2 = Rope::from_str("Hello there.");

        r1.hash(&mut h1);
        r2.hash(&mut h2);

        assert_ne!(h1.finish(), h2.finish());
    }

    #[test]
    fn hash_03() {
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        let r = Rope::from_str("Hello there!");
        let s = [Rope::from_str("Hello "), Rope::from_str("there!")];

        r.hash(&mut h1);
        Rope::hash_slice(&s, &mut h2);

        assert_ne!(h1.finish(), h2.finish());
    }
}
