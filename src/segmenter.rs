use std::fmt::Debug;
use std::marker::PhantomData;

use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

/// Trait for implementing segmentation strategies for `Rope`.
pub trait Segmenter: Debug + Copy + Clone {
    fn is_break(byte_idx: usize, text: &str) -> bool;
    fn seam_is_break(left: &str, right: &str) -> bool;
}

/// Default `Segmenter`.  Segments text according to the extended grapheme
/// cluster rules specified in
/// [Unicode Standard Annex #29](https://www.unicode.org/reports/tr29/)
#[derive(Debug, Copy, Clone)]
pub enum GraphemeSegmenter {}

impl Segmenter for GraphemeSegmenter {
    #[inline]
    fn is_break(byte_idx: usize, text: &str) -> bool {
        GraphemeCursor::new(byte_idx, text.len(), true)
            .is_boundary(text, 0)
            .unwrap()
    }

    #[inline]
    fn seam_is_break(left: &str, right: &str) -> bool {
        let tot_len = left.len() + right.len();
        let mut gc = GraphemeCursor::new(left.len(), tot_len, true);

        gc.next_boundary(right, left.len()).unwrap();
        let prev = {
            match gc.prev_boundary(right, left.len()) {
                Ok(pos) => pos,
                Err(GraphemeIncomplete::PrevChunk) => gc.prev_boundary(left, 0).unwrap(),
                _ => unreachable!(),
            }
        };

        if let Some(a) = prev {
            if a == left.len() {
                return true;
            }
        }

        return false;
    }
}

/// A `Segmenter` that will break anywhere.
#[derive(Debug, Copy, Clone)]
pub enum NullSegmenter {}

impl Segmenter for NullSegmenter {
    #[inline]
    fn is_break(_byte_idx: usize, _text: &str) -> bool {
        true
    }

    #[inline]
    fn seam_is_break(_left: &str, _right: &str) -> bool {
        true
    }
}

/// Internal-only segmenter that takes another segmenter and adds on top of
/// its segmentation that CRLF should never be broken.
/// Used by Rope to ensure that CRLF is never broken regardless of the
/// segmenter passed.
///
/// This also ensures that special cases like left/right edge or empty
/// `&str` slices get handled consistently.
#[derive(Debug, Copy, Clone)]
pub(crate) struct CRLFSegmenter<Seg: Segmenter> {
    _seg: PhantomData<Seg>,
}

impl<Seg: Segmenter> Segmenter for CRLFSegmenter<Seg> {
    #[inline]
    fn is_break(byte_idx: usize, text: &str) -> bool {
        debug_assert!(byte_idx <= text.len());
        debug_assert!(text.is_char_boundary(byte_idx));
        if byte_idx == 0 || byte_idx == text.len() {
            true
        } else {
            let bytes = text.as_bytes();
            let crlf_break = (bytes[byte_idx - 1] != 0x0D) | (bytes[byte_idx] != 0x0A);
            crlf_break && Seg::is_break(byte_idx, text)
        }
    }

    #[inline]
    fn seam_is_break(left: &str, right: &str) -> bool {
        debug_assert!(!left.is_empty() && !right.is_empty());
        let crlf_break = (left.as_bytes()[left.len() - 1] != 0x0D) | (right.as_bytes()[0] != 0x0A);
        crlf_break && Seg::seam_is_break(left, right)
    }
}

//===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_segmenter_01() {
        let text = "Hello world!\r\nHow's it going?";

        assert!(CRLFSegmenter::<NullSegmenter>::is_break(0, ""));
        assert!(CRLFSegmenter::<NullSegmenter>::is_break(0, text));
        assert!(CRLFSegmenter::<NullSegmenter>::is_break(12, text));
        assert!(!CRLFSegmenter::<NullSegmenter>::is_break(13, text));
        assert!(CRLFSegmenter::<NullSegmenter>::is_break(14, text));
        assert!(CRLFSegmenter::<NullSegmenter>::is_break(19, text));
    }

    #[test]
    fn crlf_segmenter_02() {
        let l = "Hello world!\r";
        let r = "\nHow's it going?";

        assert!(!CRLFSegmenter::<NullSegmenter>::seam_is_break(l, r));
        assert!(!CRLFSegmenter::<NullSegmenter>::seam_is_break(l, "\n"));
        assert!(!CRLFSegmenter::<NullSegmenter>::seam_is_break("\r", r));
        assert!(!CRLFSegmenter::<NullSegmenter>::seam_is_break("\r", "\n"));
        assert!(CRLFSegmenter::<NullSegmenter>::seam_is_break(r, l));
        assert!(CRLFSegmenter::<NullSegmenter>::seam_is_break("\n", "\r"));
    }
}
