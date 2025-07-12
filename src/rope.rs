use std::io;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use crate::{
    end_bound_to_num,
    iter::{Bytes, CharIndices, Chars, Chunks},
    rope_builder::RopeBuilder,
    slice::RopeSlice,
    start_bound_to_num, str_utils,
    tree::{Children, Node, Text, TextInfo, MAX_TEXT_SIZE},
    ChunkCursor,
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

assert_eq!(rope.line_to_byte_idx(0, LF_CR), 0);
assert_eq!(rope.line_to_byte_idx(1, LF_CR), 23);
assert_eq!(rope.line_to_byte_idx(2, LF_CR), 36);
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

    /// Specifies the sub-range of `root` to use as this rope's
    /// contents.  Normally just set to the full range of `root`, but
    /// [`crate::extra::disconnect_slice()`] uses this to create "disconnected
    /// slices".
    pub(crate) byte_range: [usize; 2],
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
            byte_range: [0; 2],
        }
    }

    /// Creates a `Rope` from a string slice.
    ///
    /// Runs in O(N) time.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        RopeBuilder::new().build_at_once(text)
    }

    //-----------------------------------------------------------------------
    // Convenience I/O methods.

    /// Creates a `Rope` from the output of a reader.
    ///
    /// This is a convenience function, and provides *no specific guarantees*
    /// about performance or internal implementation aside from the runtime
    /// complexity listed below.
    ///
    /// When more precise control over IO behavior, buffering, etc. is desired,
    /// you should handle IO yourself and use [`RopeBuilder`] to build the
    /// `Rope`.
    ///
    /// Runs in O(N) time.
    ///
    /// # Errors
    ///
    /// - If the reader returns an error, `from_reader` stops and returns
    ///   that error.
    /// - If non-utf8 data is encountered, an IO error with kind
    ///   `InvalidData` is returned.
    ///
    /// Note: some data from the reader is likely consumed even if there is
    /// an error.
    #[allow(unused_mut)]
    pub fn from_reader<T: io::Read>(mut reader: T) -> io::Result<Self> {
        const BUFFER_SIZE: usize = crate::tree::MAX_TEXT_SIZE * 4;
        let mut builder = RopeBuilder::new();
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut fill_idx = 0; // How much `buffer` is currently filled with valid data
        loop {
            match reader.read(&mut buffer[fill_idx..]) {
                Ok(read_count) => {
                    fill_idx += read_count;

                    // Determine how much of the buffer is valid utf8.
                    let valid_count = match std::str::from_utf8(&buffer[..fill_idx]) {
                        Ok(_) => fill_idx,
                        Err(e) => e.valid_up_to(),
                    };

                    // Append the valid part of the buffer to the rope.
                    if valid_count > 0 {
                        // The unsafe block here is reinterpreting the bytes as
                        // utf8.  This is safe because the bytes being
                        // reinterpreted have already been validated as utf8
                        // just above.
                        builder.append(unsafe {
                            std::str::from_utf8_unchecked(&buffer[..valid_count])
                        });
                    }

                    // Shift the un-read part of the buffer to the beginning.
                    if valid_count < fill_idx {
                        buffer.copy_within(valid_count..fill_idx, 0);
                    }
                    fill_idx -= valid_count;

                    if fill_idx == BUFFER_SIZE {
                        // Buffer is full and none of it could be consumed.  Utf8
                        // codepoints don't get that large, so it's clearly not
                        // valid text.
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "stream did not contain valid UTF-8",
                        ));
                    }

                    // If we're done reading
                    if read_count == 0 {
                        if fill_idx > 0 {
                            // We couldn't consume all data.
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "stream contained invalid UTF-8",
                            ));
                        } else {
                            return Ok(builder.finish());
                        }
                    }
                }

                Err(e) => {
                    // Read error
                    return Err(e);
                }
            }
        }
    }

    /// Writes the contents of the `Rope` to a writer.
    ///
    /// This is a convenience function, and provides *no specific guarantees*
    /// about performance or internal implementation aside from the runtime
    /// complexity listed below.
    ///
    /// When more precise control over IO behavior, buffering, etc. is
    /// desired, you should handle IO yourself and use the [`Chunks`]
    /// iterator to iterate through the `Rope`'s contents.
    ///
    /// Runs in O(N) time.
    ///
    /// # Errors
    ///
    /// - If the writer returns an error, `write_to` stops and returns that
    ///   error.
    ///
    /// Note: some data may have been written even if an error is returned.
    #[allow(unused_mut)]
    pub fn write_to<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        for chunk in self.chunks() {
            writer.write_all(chunk.as_bytes())?;
        }

        Ok(())
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
    /// - If `byte_idx` is out of bounds (i.e. `byte_idx > len()`).
    /// - If `byte_idx` is not on a char boundary.
    #[track_caller]
    #[inline]
    pub fn insert(&mut self, byte_idx: usize, text: &str) {
        match self.try_insert(byte_idx, text) {
            Ok(_) => {}
            Err(e) => panic!("{}", e),
        }
    }

    /// Inserts a single char `ch` at byte index `byte_idx`.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::Rope;
    /// let mut rope = Rope::from_str("Hello orld!");
    /// rope.insert_char(6, 'w');
    ///
    /// assert_eq!("Hello world!", rope);
    /// ```
    ///
    /// # Panics
    ///
    /// - If `byte_idx` is out of bounds (i.e. `byte_idx > len()`).
    /// - If `byte_idx` is not on a char boundary.
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
    /// - If the end of the range is out of bounds (i.e. `end > len()`).
    /// - If the range ends are not on char boundaries.
    #[track_caller]
    #[inline]
    pub fn remove<R>(&mut self, byte_range: R)
    where
        R: RangeBounds<usize>,
    {
        match self.try_remove(byte_range) {
            Ok(_) => {}
            Err(e) => panic!("{}", e),
        }
    }

    /// Converts a "disconnected slice" into a proper rope, in preparation for
    /// edits.
    fn trim_disconnected_slice(&mut self) {
        let trim_range_start = [0, self.byte_range[0]];
        let trim_range_end = [self.byte_range[1], self.root_info.bytes];

        // Note: unlike with normal removal, we don't have to worry about crlf
        // splits because we know we're trimming off the ends, not removing a
        // section in the middle.
        self.remove_core_impl(trim_range_end)
            .expect("Trimming to slice range should always succeed.");
        self.remove_core_impl(trim_range_start)
            .expect("Trimming to slice range should always succeed.");

        self.byte_range[0] = 0;
        self.byte_range[1] = self.root_info.bytes;
    }

    //---------------------------------------------------------
    // Slicing.

    /// Gets an immutable slice of the `Rope`.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::Rope;
    /// let rope = Rope::from_str("Hello world!");
    /// let slice = rope.slice(..5);
    ///
    /// assert_eq!("Hello", slice);
    /// ```
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// - If the start of the range is greater than the end.
    /// - If the end of the range is out of bounds (i.e. `end > len()`).
    /// - If the range ends are not on char boundaries.
    #[track_caller]
    #[inline]
    pub fn slice<R>(&self, byte_range: R) -> RopeSlice<'_>
    where
        R: RangeBounds<usize>,
    {
        match self.try_slice(byte_range) {
            Ok(slice) => slice,
            Err(e) => panic!("{}", e),
        }
    }

    //---------------------------------------------------------
    // Methods shared between Rope and RopeSlice.

    crate::shared_impl::shared_main_impl_methods!('_);

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
        self.assert_no_crlf_splits();
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
    pub fn assert_no_crlf_splits(&self) {
        let mut last_ends_with_cr = false;
        for chunk in self.chunks().filter(|c| !c.is_empty()) {
            if last_ends_with_cr && str_utils::starts_with_lf(chunk) {
                panic!("CRLF split found.");
            }
            last_ends_with_cr = str_utils::ends_with_cr(chunk);
        }
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
    fn get_str_text(&self) -> Option<&str> {
        None
    }

    #[inline(always)]
    fn get_root(&self) -> &Node {
        &self.root
    }

    #[inline(always)]
    fn get_root_info(&self) -> &TextInfo {
        &self.root_info
    }

    #[inline(always)]
    fn get_byte_range(&self) -> [usize; 2] {
        self.byte_range
    }
}

//=============================================================
// Non-panicking versions.

/// Non-panicking versions of some of `Rope`'s methods.
impl Rope {
    /// Non-panicking version of `insert()`.
    ///
    /// On failure this leaves the rope untouched and returns the cause of the
    /// failure.
    pub fn try_insert(&mut self, byte_idx: usize, text: &str) -> Result<()> {
        if byte_idx > self.len() {
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

        // If this is a "disconnected slice", rather than a normal rope, then
        // we need to first trim it to a normal rope before proceeding with
        // editing.
        if self.byte_range[0] != 0 || self.byte_range[1] != self.root_info.bytes {
            if !self.is_char_boundary(byte_idx) {
                // Don't bother if the edit is going to fail anyway.
                return Err(NonCharBoundary);
            }
            self.trim_disconnected_slice();
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
        //
        // Additionally, we handle a starting LF specially, to avoid creating
        // split CRLF pairs.
        let mut text = text;
        let starting_lf = if str_utils::starts_with_lf(text) {
            // Take out the starting LF for special handling later.
            text = &text[1..];
            true
        } else {
            false
        };
        while !text.is_empty() {
            // Split a chunk off from the end of the text.
            // We do this from the end instead of the front so that the repeated
            // insertions can keep re-using the same insertion point.
            //
            // NOTE: the chunks are at most `MAX_TEXT_SIZE - 4` rather than
            // just `MAX_TEXT_SIZE` to guarantee that nodes can split into
            // node-sized chunks even in the face of multi-byte chars and
            // CRLF pairs that may prevent splits at certain byte indices.
            // This is a subtle issue that in practice only very rarely
            // manifests, but causes panics when it does.  Please do not
            // remove that `- 4`!
            let split_idx = crate::find_appropriate_split_ceil(
                text.len() - (MAX_TEXT_SIZE - 4).min(text.len()),
                text,
            );
            let ins_text = &text[split_idx..];
            text = &text[..split_idx];

            // Do the insertion.
            self.insert_core_impl(byte_idx, ins_text, false)?;
        }

        if starting_lf {
            // Insert the starting LF with bias_left = true.  This ensures
            // that it gets inserted to the left of any chunk boundary, which
            // prevents by construction creating any split CRLF pairs.
            self.insert_core_impl(byte_idx, "\n", true)?;
        }

        // Do a rebalancing step.
        self.root.partial_rebalance();
        self.pull_up_singular_nodes();

        Ok(())
    }

    /// Non-panicking version of `insert_char()`.
    ///
    /// On failure this leaves the rope untouched and returns the cause of the
    /// failure.
    #[inline]
    pub fn try_insert_char(&mut self, byte_idx: usize, ch: char) -> Result<()> {
        let mut buf = [0u8; 4];
        self.try_insert(byte_idx, ch.encode_utf8(&mut buf))
    }

    /// Non-panicking version of `remove()`.
    ///
    /// On failure this leaves the rope untouched and returns the cause of the
    /// failure.
    #[inline]
    pub fn try_remove<R>(&mut self, byte_range: R) -> Result<()>
    where
        R: RangeBounds<usize>,
    {
        // Inner function to avoid code duplication on code gen due to the
        // generic type of `byte_range`.
        fn inner(rope: &mut Rope, start: Bound<&usize>, end: Bound<&usize>) -> Result<()> {
            let start_idx = start_bound_to_num(start).unwrap_or(0);
            let end_idx = end_bound_to_num(end).unwrap_or_else(|| rope.len());

            if start_idx > end_idx {
                return Err(InvalidRange);
            }

            if end_idx > rope.len() {
                return Err(OutOfBounds);
            }

            // Unlike with insertion, for removal we have to check if the
            // indices are char boundaries ahead of time, because the nature
            // of the removal code means it might do partial removals before it
            // discovers that one of the ends isn't a char boundary.
            if !rope.is_char_boundary(start_idx) || !rope.is_char_boundary(end_idx) {
                return Err(NonCharBoundary);
            }

            // If this is a "disconnected slice", rather than a normal rope,
            // then we need to first trim it to a normal rope before proceeding
            // with editing.
            if rope.byte_range[0] != 0 || rope.byte_range[1] != rope.root_info.bytes {
                rope.trim_disconnected_slice();
            }

            // Do the actual removal.
            let created_boundary = rope.remove_core_impl([start_idx, end_idx])?;

            if created_boundary {
                rope.fix_potential_crlf_split(start_idx);
            }

            // Do a rebalancing step.
            rope.root.partial_rebalance();
            rope.pull_up_singular_nodes();

            Ok(())
        }

        inner(self, byte_range.start_bound(), byte_range.end_bound())
    }

    /// Non-panicking version of `slice()`.
    ///
    /// On failure this returns the cause of the failure.
    #[inline]
    pub fn try_slice<R>(&self, byte_range: R) -> Result<RopeSlice<'_>>
    where
        R: RangeBounds<usize>,
    {
        let start_idx = start_bound_to_num(byte_range.start_bound()).unwrap_or(0);
        let end_idx = end_bound_to_num(byte_range.end_bound()).unwrap_or_else(|| self.len());

        fn inner(rope: &Rope, start_idx: usize, end_idx: usize) -> Result<RopeSlice<'_>> {
            if !rope.is_char_boundary(start_idx) || !rope.is_char_boundary(end_idx) {
                return Err(NonCharBoundary);
            }
            if start_idx > end_idx {
                return Err(InvalidRange);
            }
            if end_idx > rope.len() {
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
    crate::shared_impl::shared_no_panic_impl_methods!('_);

    //---------------------------------------------------------

    /// The core insertion procedure, without any checks (like the `text` length
    /// being small enough to handle with a single insertion), tree reblancing,
    /// CRLF split handling, etc.
    #[inline(always)]
    fn insert_core_impl(&mut self, byte_idx: usize, text: &str, bias_left: bool) -> Result<()> {
        debug_assert!(byte_idx <= self.len());
        debug_assert!(text.len() <= (MAX_TEXT_SIZE - 4));

        // Do the insertion.
        let (new_root_info, residual) =
            self.root
                .insert_at_byte_idx(byte_idx, text, bias_left, self.root_info)?;
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

        self.byte_range[1] = self.root_info.bytes;

        Ok(())
    }

    /// The core removal procedure, without any checks (like the range being
    /// well-formed), tree rebalancing, CRLF split handling, etc.
    ///
    /// NOTE: even when this fails, some removal may have happened.
    ///
    /// The returned bool is whether a fresh boundary was created.
    #[inline(always)]
    fn remove_core_impl(&mut self, byte_range: [usize; 2]) -> Result<bool> {
        debug_assert!(byte_range[0] <= byte_range[1]);
        debug_assert!(byte_range[1] <= self.root_info.bytes);

        // Special case: if we're removing everything, just replace with a
        // fresh new rope.  This is to ensure the invariant that an empty
        // rope is always composed of a single empty leaf, which is not
        // ensured by the general removal code.
        if byte_range[0] == 0 && byte_range[1] == self.root_info.bytes {
            *self = Rope::new();
            return Ok(false);
        }

        let (new_info, created_boundary) =
            self.root.remove_byte_range(byte_range, self.root_info)?;
        self.root_info = new_info;
        self.byte_range[1] = self.root_info.bytes;

        Ok(created_boundary)
    }

    fn fix_potential_crlf_split(&mut self, byte_idx: usize) {
        if byte_idx == 0 || byte_idx >= self.len() {
            return;
        }

        if self.byte(byte_idx - 1) == b'\r' && self.byte(byte_idx) == b'\n' {
            // First remove the LF.
            self.remove_core_impl([byte_idx, byte_idx + 1]).unwrap();

            // Then insert it again with a left bias, so it ends up in the same
            // chunk as the CR.
            self.insert_core_impl(byte_idx, "\n", true).unwrap();
        }
    }
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
        let (r, _) = make_rope_and_text_from_chunks(&["\n\r", "\r\n", "\n\r", "\r\n", "\n\r"]);

        {
            let mut r = r.clone();
            r.insert(0, "\r");
            r.assert_no_crlf_splits();
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(2, "\n");
            r.assert_no_crlf_splits();
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(4, "\r");
            r.assert_no_crlf_splits();
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(6, "\n");
            r.assert_no_crlf_splits();
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(8, "\r");
            r.assert_no_crlf_splits();
            r.assert_accurate_text_info();
        }
        {
            let mut r = r.clone();
            r.insert(10, "\n");
            r.assert_no_crlf_splits();
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

    // Removal failure should be atomic: either it fails with no modification,
    // or the whole intended modification completes.
    //
    // Caught by fuzz testing.
    #[test]
    fn try_remove_failure_01() {
        let mut r = Rope::from_str(include_str!("../fuzz/fuzz_targets/small.txt"));
        let r_original = r.clone();
        let result = r.try_remove(19..559);

        assert!(result.is_err());
        assert_eq!(r, r_original);
        r.assert_invariants();
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn byte_to_char_idx_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.byte_to_char_idx(0));
        assert_eq!(1, r.byte_to_char_idx(1));
        assert_eq!(2, r.byte_to_char_idx(2));

        assert_eq!(91, r.byte_to_char_idx(91));
        assert_eq!(91, r.byte_to_char_idx(92));
        assert_eq!(91, r.byte_to_char_idx(93));

        assert_eq!(92, r.byte_to_char_idx(94));
        assert_eq!(92, r.byte_to_char_idx(95));
        assert_eq!(92, r.byte_to_char_idx(96));

        assert_eq!(102, r.byte_to_char_idx(124));
        assert_eq!(102, r.byte_to_char_idx(125));
        assert_eq!(102, r.byte_to_char_idx(126));
        assert_eq!(103, r.byte_to_char_idx(127));
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn char_to_byte_idx_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.char_to_byte_idx(0));
        assert_eq!(1, r.char_to_byte_idx(1));
        assert_eq!(2, r.char_to_byte_idx(2));

        assert_eq!(91, r.char_to_byte_idx(91));
        assert_eq!(94, r.char_to_byte_idx(92));
        assert_eq!(97, r.char_to_byte_idx(93));
        assert_eq!(100, r.char_to_byte_idx(94));

        assert_eq!(124, r.char_to_byte_idx(102));
        assert_eq!(127, r.char_to_byte_idx(103));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_idx_01() {
        let r = Rope::from_str(TEXT_EMOJI);

        assert_eq!(0, r.byte_to_utf16_idx(0));

        assert_eq!(12, r.byte_to_utf16_idx(12));
        assert_eq!(12, r.byte_to_utf16_idx(13));
        assert_eq!(14, r.byte_to_utf16_idx(16));

        assert_eq!(33, r.byte_to_utf16_idx(35));
        assert_eq!(33, r.byte_to_utf16_idx(36));
        assert_eq!(35, r.byte_to_utf16_idx(39));

        assert_eq!(63, r.byte_to_utf16_idx(67));
        assert_eq!(63, r.byte_to_utf16_idx(70));
        assert_eq!(65, r.byte_to_utf16_idx(71));

        assert_eq!(95, r.byte_to_utf16_idx(101));
        assert_eq!(95, r.byte_to_utf16_idx(102));
        assert_eq!(97, r.byte_to_utf16_idx(105));

        assert_eq!(111, r.byte_to_utf16_idx(143));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_idx_01() {
        let r = Rope::from_str(TEXT_EMOJI);

        assert_eq!(0, r.utf16_to_byte_idx(0));

        assert_eq!(12, r.utf16_to_byte_idx(12));
        assert_eq!(16, r.utf16_to_byte_idx(14));

        assert_eq!(35, r.utf16_to_byte_idx(33));
        assert_eq!(39, r.utf16_to_byte_idx(35));

        assert_eq!(67, r.utf16_to_byte_idx(63));
        assert_eq!(71, r.utf16_to_byte_idx(65));

        assert_eq!(101, r.utf16_to_byte_idx(95));
        assert_eq!(105, r.utf16_to_byte_idx(97));

        assert_eq!(143, r.utf16_to_byte_idx(111));
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn byte_to_line_idx_01() {
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
            assert_eq!(l, r.byte_to_line_idx(b, LineType::LF));
            #[cfg(feature = "metric_lines_lf_cr")]
            assert_eq!(l, r.byte_to_line_idx(b, LineType::LF_CR));
            #[cfg(feature = "metric_lines_unicode")]
            assert_eq!(l, r.byte_to_line_idx(b, LineType::Unicode));
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn byte_to_line_idx_02() {
        let r = Rope::from_str("");

        #[cfg(feature = "metric_lines_lf")]
        assert_eq!(0, r.byte_to_line_idx(0, LineType::LF));
        #[cfg(feature = "metric_lines_lf_cr")]
        assert_eq!(0, r.byte_to_line_idx(0, LineType::LF_CR));
        #[cfg(feature = "metric_lines_unicode")]
        assert_eq!(0, r.byte_to_line_idx(0, LineType::Unicode));
    }

    #[cfg(feature = "metric_lines_lf")]
    #[test]
    #[should_panic]
    fn byte_to_line_idx_03() {
        let r = Rope::from_str(TEXT_LINES);
        r.byte_to_line_idx(125, LineType::LF);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[should_panic]
    fn byte_to_line_idx_04() {
        let r = Rope::from_str(TEXT_LINES);
        r.byte_to_line_idx(125, LineType::LF_CR);
    }

    #[cfg(feature = "metric_lines_unicode")]
    #[test]
    #[should_panic]
    fn byte_to_line_idx_05() {
        let r = Rope::from_str(TEXT_LINES);
        r.byte_to_line_idx(125, LineType::Unicode);
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn line_to_byte_idx_01() {
        let r = Rope::from_str(TEXT_LINES);
        let byte_to_line_idxs = &[[0, 0], [32, 1], [59, 2], [88, 3], [124, 4]];
        for [b, l] in byte_to_line_idxs.iter().copied() {
            #[cfg(feature = "metric_lines_lf")]
            assert_eq!(b, r.line_to_byte_idx(l, LineType::LF));
            #[cfg(feature = "metric_lines_lf_cr")]
            assert_eq!(b, r.line_to_byte_idx(l, LineType::LF_CR));
            #[cfg(feature = "metric_lines_unicode")]
            assert_eq!(b, r.line_to_byte_idx(l, LineType::Unicode));
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn line_to_byte_idx_02() {
        let r = Rope::from_str("");
        #[cfg(feature = "metric_lines_lf")]
        {
            assert_eq!(0, r.line_to_byte_idx(0, LineType::LF));
            assert_eq!(0, r.line_to_byte_idx(1, LineType::LF));
        }
        #[cfg(feature = "metric_lines_lf_cr")]
        {
            assert_eq!(0, r.line_to_byte_idx(0, LineType::LF_CR));
            assert_eq!(0, r.line_to_byte_idx(1, LineType::LF_CR));
        }
        #[cfg(feature = "metric_lines_unicode")]
        {
            assert_eq!(0, r.line_to_byte_idx(0, LineType::Unicode));
            assert_eq!(0, r.line_to_byte_idx(1, LineType::Unicode));
        }
    }

    #[cfg(feature = "metric_lines_lf")]
    #[test]
    #[should_panic]
    fn line_to_byte_idx_03() {
        let r = Rope::from_str(TEXT_LINES);
        r.line_to_byte_idx(5, LineType::LF);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[should_panic]
    fn line_to_byte_idx_04() {
        let r = Rope::from_str(TEXT_LINES);
        r.line_to_byte_idx(5, LineType::LF_CR);
    }

    #[cfg(feature = "metric_lines_unicode")]
    #[test]
    #[should_panic]
    fn line_to_byte_idx_05() {
        let r = Rope::from_str(TEXT_LINES);
        r.line_to_byte_idx(5, LineType::Unicode);
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
