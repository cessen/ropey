#![allow(clippy::collapsible_if)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::inline_always)]
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::type_complexity)]

use std::ops::Bound;

mod shared_impl;

mod rope;
mod rope_builder;
mod slice;
mod str_utils;
mod tree;

pub mod iter;

pub use rope::Rope;
pub use rope_builder::RopeBuilder;
pub use slice::RopeSlice;
pub use tree::TextInfo;

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum LineType {
    #[cfg(feature = "metric_lines_lf")]
    LF,
    #[cfg(feature = "metric_lines_cr_lf")]
    CRLF,
    #[cfg(feature = "metric_lines_unicode")]
    All,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    NonCharBoundary,
    OutOfBounds,
    InvalidRange,
    CannotEditOwnedSlice,
}

impl Error {
    fn panic_with_msg(&self) -> ! {
        match *self {
            Error::NonCharBoundary => panic!("Index is a non-char boundary."),
            Error::OutOfBounds => panic!("Index out of bounds."),
            Error::InvalidRange => panic!("Invalid index range: end < start."),
            Error::CannotEditOwnedSlice => {
                panic!("Editing of owned slices is not permitted.")
            }
        }
    }
}

//=============================================================
// Utilities.

/// Starting from `byte_idx`, scans left to find the first byte index that is a
/// char boundary.  Note that this is inclusive of `byte_idx` (if it is already
/// a char boundary, `byte_idx` is returned).
///
/// Precondition: `text` must be a well-formed utf8 string.
///
/// Note for convenience, if `byte_idx > text.len()`, this simply returns
/// `text.len()`.
pub(crate) fn find_char_boundary_l(mut byte_idx: usize, text: &[u8]) -> usize {
    if byte_idx >= text.len() {
        return text.len();
    }

    while (text[byte_idx] >> 6) == 0b10 && byte_idx > 0 {
        byte_idx -= 1;
    }

    byte_idx
}

/// Starting from `byte_idx`, scans right to find the first byte index that is a
/// char boundary.  Note that this is inclusive of `byte_idx` (if it is already
/// a char boundary, `byte_idx` is returned).
///
/// Precondition: `text` must be a well-formed utf8 string.
///
/// Note for convenience, if `byte_idx > text.len()`, this simply returns
/// `text.len()`.
pub(crate) fn find_char_boundary_r(mut byte_idx: usize, text: &[u8]) -> usize {
    if byte_idx >= text.len() {
        return text.len();
    }

    while (text[byte_idx] >> 6) == 0b10 {
        byte_idx += 1;
    }

    byte_idx
}

//-------------------------------------------------------------
// Range handling utilities.

#[inline(always)]
pub(crate) fn start_bound_to_num(b: Bound<&usize>) -> Option<usize> {
    match b {
        Bound::Included(n) => Some(*n),
        Bound::Excluded(n) => Some(*n + 1),
        Bound::Unbounded => None,
    }
}

#[inline(always)]
pub(crate) fn end_bound_to_num(b: Bound<&usize>) -> Option<usize> {
    match b {
        Bound::Included(n) => Some(*n + 1),
        Bound::Excluded(n) => Some(*n),
        Bound::Unbounded => None,
    }
}
