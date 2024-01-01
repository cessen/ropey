// use std::io;
// use std::iter::FromIterator;
// use std::ops::RangeBounds;
use std::sync::Arc;

use crate::{
    iter::Chunks,
    rope_builder::RopeBuilder,
    tree::{Children, Node, Text, TextInfo, MAX_TEXT_SIZE},
};

#[derive(Clone)]
pub struct Rope {
    pub(crate) root: Node,
    pub(crate) root_info: TextInfo,
}

impl Rope {
    //---------------------------------------------------------
    // Constructors.

    /// Creates an empty `Rope`.
    pub fn new() -> Self {
        Rope {
            root: Node::Leaf(Arc::new(Text::from_str(""))),
            root_info: TextInfo::new(),
        }
    }

    /// Creates a `Rope` with the contents of `text`.
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
    /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`)
    /// or not a char boundary.
    #[inline]
    pub fn insert(&mut self, byte_idx: usize, text: &str) {
        assert!(byte_idx <= self.len_bytes());

        if text.is_empty() {
            return;
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
            // We do this from the end instead of the front so that
            // the repeated insertions can keep re-using the same
            // insertion point.  Note that the minus 4 is to guarantee
            // that nodes can split into node-sized chunks even in the
            // face of multi-byte chars that may prevent splits at
            // certain byte indices.
            let split_idx = crate::find_split(
                text.len() - (MAX_TEXT_SIZE - 4).min(text.len()),
                text.as_bytes(),
            );
            let ins_text = &text[split_idx..];
            text = &text[..split_idx];

            // Do the insertion.
            let (new_root_info, residual) =
                self.root
                    .insert_at_byte_idx(byte_idx, ins_text, self.root_info);
            self.root_info = new_root_info;

            // Handle root split.
            if let Some((right_info, right_node)) = residual {
                let mut left_node = Node::Internal(Arc::new(Children::new()));
                std::mem::swap(&mut left_node, &mut self.root);

                let children = self.root.children_mut();
                children.push((self.root_info, left_node));
                children.push((right_info, right_node));
                self.root_info = children.combined_info();
            }
        }
    }

    //---------------------------------------------------------
    // Queries.

    pub fn len_bytes(&self) -> usize {
        self.root_info.bytes as usize
    }

    pub fn len_chars(&self) -> usize {
        self.root_info.chars as usize
    }

    pub fn len_utf16(&self) -> usize {
        (self.root_info.chars + self.root_info.utf16_surrogates) as usize
    }

    pub fn len_lines_lf(&self) -> usize {
        (self.root_info.line_breaks_lf + 1) as usize
    }

    pub fn len_lines_cr_lf(&self) -> usize {
        (self.root_info.line_breaks_cr_lf + 1) as usize
    }

    pub fn len_lines_unicode(&self) -> usize {
        (self.root_info.line_breaks_unicode + 1) as usize
    }

    //---------------------------------------------------------
    // Iterators.

    pub fn chunks(&self) -> Chunks<'_> {
        Chunks::new(&self.root)
    }
}

//==============================================================
// Comparison impls.

impl<'a> std::cmp::PartialEq<&'a str> for Rope {
    #[inline]
    fn eq(&self, other: &&'a str) -> bool {
        if self.len_bytes() != other.len() {
            return false;
        }
        let other = other.as_bytes();

        let mut idx = 0;
        for chunk in self.chunks().flatten() {
            let chunk = chunk.as_bytes();
            if chunk != &other[idx..(idx + chunk.len())] {
                return false;
            }
            idx += chunk.len();
        }

        return true;
    }
}

impl<'a> std::cmp::PartialEq<Rope> for &'a str {
    #[inline]
    fn eq(&self, other: &Rope) -> bool {
        other == self
    }
}

//==============================================================
// Other impls.

impl std::default::Default for Rope {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Rope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list().entries(self.chunks()).finish()
    }
}

impl std::fmt::Display for Rope {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for chunk in self.chunks().flatten() {
            write!(f, "{}", chunk)?
        }
        Ok(())
    }
}
