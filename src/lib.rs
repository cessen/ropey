#![allow(clippy::collapsible_if)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::inline_always)]
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::type_complexity)]

use std::ops::Bound;

pub mod iter;
mod rope;
mod rope_builder;
mod tree;

pub use rope::Rope;
pub use rope_builder::RopeBuilder;

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

//=============================================================
// Utilities.

/// Scans left from `byte_idx` to find a char boundary.
///
/// This is used to find an appropriate split position on utf8 strings.
///
/// Precondition: `text` must be a well-formed utf8 string.
///
/// Note for convenience, if `byte_idx > text.len()`, this simply
/// returns `text.len()`.
pub(crate) fn find_split_l(mut byte_idx: usize, text: &[u8]) -> usize {
    if byte_idx >= text.len() {
        return text.len();
    }

    while (text[byte_idx] >> 6) == 0b10 && byte_idx > 0 {
        byte_idx -= 1;
    }

    byte_idx
}

/// Scans right from `byte_idx` to find a char boundary.
///
/// This is used to find an appropriate split position on utf8 strings.
///
/// Precondition: `text` must be a well-formed utf8 string.
///
/// Note for convenience, if `byte_idx > text.len()`, this simply
/// returns `text.len()`.
pub(crate) fn find_split_r(mut byte_idx: usize, text: &[u8]) -> usize {
    if byte_idx >= text.len() {
        return text.len();
    }

    while (text[byte_idx] >> 6) == 0b10 {
        byte_idx += 1;
    }

    byte_idx
}

#[inline(always)]
pub(crate) fn starts_with_lf(text: &str) -> bool {
    text.as_bytes().get(0).map(|&b| b == 0x0A).unwrap_or(false)
}

#[inline(always)]
pub(crate) fn ends_with_cr(text: &str) -> bool {
    text.as_bytes().last().map(|&b| b == 0x0D).unwrap_or(false)
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

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_lf_01() {
        assert_eq!(false, starts_with_lf(""));
        assert_eq!(false, starts_with_lf("Hello!"));
        assert_eq!(true, starts_with_lf("\n"));
        assert_eq!(true, starts_with_lf("\nHello!"));
    }

    #[test]
    fn ends_with_cr_01() {
        assert_eq!(false, ends_with_cr(""));
        assert_eq!(false, ends_with_cr("Hello!"));
        assert_eq!(true, ends_with_cr("\r"));
        assert_eq!(true, ends_with_cr("Hello!\r"));
    }
}
