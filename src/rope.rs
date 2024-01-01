// use std::io;
// use std::iter::FromIterator;
// use std::ops::RangeBounds;
use std::sync::Arc;

use crate::{
    iter::Chunks,
    rope_builder::RopeBuilder,
    tree::{Node, Text},
};

#[derive(Clone)]
pub struct Rope {
    pub(crate) root: Node,
}

impl Rope {
    //---------------------------------------------------------
    // Constructors.

    /// Creates an empty `Rope`.
    pub fn new() -> Self {
        Rope {
            root: Node::Leaf(Arc::new(Text::from_str(""))),
        }
    }

    /// Creates a `Rope` with the contents of `text`.
    pub fn from_str(text: &str) -> Self {
        RopeBuilder::new().build_at_once(text)
    }

    //---------------------------------------------------------
    // Queries.

    pub fn len_bytes(&self) -> usize {
        self.root.text_info().bytes as usize
    }

    pub fn len_chars(&self) -> usize {
        self.root.text_info().chars as usize
    }

    pub fn len_utf16(&self) -> usize {
        (self.root.text_info().chars + self.root.text_info().utf16_surrogates) as usize
    }

    pub fn len_lines_lf(&self) -> usize {
        (self.root.text_info().line_breaks_lf + 1) as usize
    }

    pub fn len_lines_cr_lf(&self) -> usize {
        (self.root.text_info().line_breaks_cr_lf + 1) as usize
    }

    pub fn len_lines_unicode(&self) -> usize {
        (self.root.text_info().line_breaks_unicode + 1) as usize
    }

    //---------------------------------------------------------
    // Iterators.

    pub fn chunks(&self) -> Chunks<'_> {
        Chunks::new(&self.root)
    }
}

impl std::default::Default for Rope {
    #[inline]
    fn default() -> Self {
        Self::new()
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

//==============================================================
// Other impls.

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
