use std::ops::RangeBounds;

use crate::{
    end_bound_to_num,
    iter::{Bytes, Chars, Chunks},
    start_bound_to_num,
    tree::{Node, TextInfo},
    ChunkCursor,
    Error::*,
    Result, Rope,
};

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
use crate::{iter::Lines, LineType};

/// An immutable view into part of a `Rope`.
///
/// Just like standard `&str` slices, `RopeSlice`s behave as if the text in
/// their range is the only text that exists.  All indexing is relative to
/// the start of their range, and all iterators and methods that return text
/// truncate that text to the range of the slice.
///
/// In other words, the behavior of a `RopeSlice` is always identical to that
/// of a full `Rope` created from the same text range.  Nothing should be
/// surprising here.
#[derive(Copy, Clone)]
pub struct RopeSlice<'a> {
    root: &'a Node,
    root_info: &'a TextInfo,
    byte_range: [usize; 2],
}

impl<'a> RopeSlice<'a> {
    pub(crate) fn new(root: &'a Node, root_info: &'a TextInfo, byte_range: [usize; 2]) -> Self {
        // Special case for performance, since this actually comes up a fair bit.
        if byte_range[0] == 0 && byte_range[1] == root_info.bytes {
            return Self {
                root: root,
                root_info: root_info,
                byte_range: byte_range,
            };
        }

        // Find the deepest node that still contains the full range given.
        let mut start = byte_range[0];
        let mut end = byte_range[1];
        let mut node = root;
        let mut node_info = root_info;
        'outer: loop {
            match *node {
                Node::Leaf(_) => {
                    break;
                }

                Node::Internal(ref children) => {
                    let mut child_start_byte = 0;
                    for (i, info) in children.info().iter().enumerate() {
                        let child_end_byte = child_start_byte + info.bytes;
                        if start >= child_start_byte && end <= child_end_byte {
                            start -= child_start_byte;
                            end -= child_start_byte;
                            node = &children.nodes()[i];
                            node_info = &children.info()[i];
                            continue 'outer;
                        }
                        child_start_byte = child_end_byte;
                    }
                    break;
                }
            }
        }

        Self {
            root: node,
            root_info: node_info,
            byte_range: [start, end],
        }
    }

    //-------------------------------------------------
    // Slicing.

    /// Gets an immutable slice of the `RopeSlice`.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.
    ///
    /// Runs in O(log N) time.
    ///
    /// # Panics
    ///
    /// - If the start of the range is greater than the end.
    /// - If the end of the range is out of bounds (i.e. `end > len()`).
    /// - If the range ends are not on char boundaries.
    #[inline(always)]
    pub fn slice<R>(&self, byte_range: R) -> RopeSlice<'a>
    where
        R: RangeBounds<usize>,
    {
        match self.try_slice(byte_range) {
            Ok(slice) => slice,
            Err(e) => e.panic_with_msg(),
        }
    }

    // Methods shared between Rope and RopeSlice.
    crate::shared_impl::shared_main_impl_methods!('a);

    //---------------------------------------------------------
    // Utility methods needed by the shared impl macros in
    // `crate::shared_impl`.

    #[inline(always)]
    fn get_root(&self) -> &'a Node {
        self.root
    }

    #[allow(dead_code)] // Only used with some features.
    #[inline(always)]
    fn get_root_info(&self) -> &'a TextInfo {
        self.root_info
    }

    #[inline(always)]
    fn get_byte_range(&self) -> [usize; 2] {
        self.byte_range
    }
}

/// Non-panicking versions of some of `RopeSlice`'s methods.
impl<'a> RopeSlice<'a> {
    /// Non-panicking version of `slice()`.
    ///
    /// On failure this returns the cause of the failure.
    #[inline]
    pub fn try_slice<R>(&self, byte_range: R) -> Result<RopeSlice<'a>>
    where
        R: RangeBounds<usize>,
    {
        let start_idx = start_bound_to_num(byte_range.start_bound()).unwrap_or(0);
        let end_idx = end_bound_to_num(byte_range.end_bound()).unwrap_or_else(|| self.len());

        fn inner<'a>(
            slice: &RopeSlice<'a>,
            start_idx: usize,
            end_idx: usize,
        ) -> Result<RopeSlice<'a>> {
            if !slice.is_char_boundary(start_idx) || !slice.is_char_boundary(end_idx) {
                return Err(NonCharBoundary);
            }
            if start_idx > end_idx {
                return Err(InvalidRange);
            }
            if end_idx > slice.len() {
                return Err(OutOfBounds);
            }

            let start_idx_real = slice.byte_range[0] + start_idx;
            let end_idx_real = slice.byte_range[0] + end_idx;

            Ok(RopeSlice::new(
                slice.root,
                slice.root_info,
                [start_idx_real, end_idx_real],
            ))
        }

        inner(self, start_idx, end_idx)
    }

    // Methods shared between Rope and RopeSlice.
    crate::shared_impl::shared_no_panic_impl_methods!('a);
}

//=============================================================

impl crate::extra::RopeExt for RopeSlice<'_> {
    #[inline]
    fn to_owning_slice(&self) -> Rope {
        Rope {
            root: self.root.clone(),
            root_info: *self.root_info,
            owned_slice_byte_range: self.byte_range,
        }
    }
}

//=============================================================
// Stdlib trait impls.
//
// Note: most impls are in `shared_impls.rs`.  The only ones here are the ones
// that need to distinguish between Rope and RopeSlice.

// Impls shared between Rope and RopeSlice.
crate::shared_impl::shared_std_impls!(RopeSlice<'_>);

impl std::cmp::PartialEq<Rope> for RopeSlice<'_> {
    fn eq(&self, other: &Rope) -> bool {
        *self == RopeSlice::from(other)
    }
}

impl<'a> From<&'a Rope> for RopeSlice<'a> {
    fn from(r: &Rope) -> RopeSlice {
        RopeSlice::new(&r.root, &r.root_info, [0, r.root_info.bytes])
    }
}

impl<'a> From<RopeSlice<'a>> for std::borrow::Cow<'a, str> {
    #[inline]
    fn from(r: RopeSlice<'a>) -> Self {
        match *r.root {
            Node::Leaf(ref text) => {
                std::borrow::Cow::Borrowed(&text.text()[r.byte_range[0]..r.byte_range[1]])
            }
            Node::Internal(_) => std::borrow::Cow::Owned(String::from(r)),
        }
    }
}

//=============================================================

#[cfg(test)]
mod tests {
    use std::hash::{Hash, Hasher};

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    use crate::LineType;

    use crate::{rope_builder::RopeBuilder, Rope, RopeSlice};

    // 127 bytes, 103 chars, 1 line
    const TEXT: &str = "Hello there!  How're you doing?  It's \
                        a fine day, isn't it?  Aren't you glad \
                        we're alive?  こんにちは、みんなさん！";

    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    // 143 bytes, 107 chars, 111 utf16 code units, 1 line
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
    fn reslice() {
        // This is a compile-time test, to make sure that lifetimes work
        // as expected when taking slices of slices.  The lifetime of a
        // slice-of-a-slice should depend on the original rope, not the slice it
        // was sliced from.
        let r = Rope::from_str(TEXT);
        let s = {
            let s1 = r.slice(4..32);
            s1.slice(2..24)
        };
        _ = s;
    }

    #[test]
    fn iterator_of_tmp_slice() {
        // This is a compile-time test, to make sure that lifetimes work as
        // expected when making iterators from slices, where the iterators live
        // longer than those slices.  The lifetime of such an iterator should
        // depend on the original rope, not the slice it was created from.
        let r = Rope::from_str(TEXT);
        let iterators = {
            let s1 = r.slice(4..32);
            (
                s1.bytes(),
                s1.bytes_at(1),
                s1.chars(),
                s1.chars_at(1),
                #[cfg(feature = "metric_lines_lf_cr")]
                s1.lines(LineType::LF_CR),
                #[cfg(feature = "metric_lines_lf_cr")]
                s1.lines_at(1, LineType::LF_CR),
                s1.chunks(),
                s1.chunks_at(1),
                s1.chunk_cursor(),
                s1.chunk_cursor_at(1),
            )
        };
        _ = iterators;
    }

    #[test]
    fn len_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(7..97);
        assert_eq!(s.len(), 90);
    }

    #[test]
    fn len_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);
        assert_eq!(s.len(), 0);
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn len_chars_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(7..97);
        assert_eq!(s.len_chars(), 86);
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn len_chars_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);
        assert_eq!(s.len_chars(), 0);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn len_lines_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..97);
        assert_eq!(s.len_lines(LineType::LF_CR), 3);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn len_lines_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);
        assert_eq!(s.len_lines(LineType::LF_CR), 1);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn len_utf16_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(..);
        assert_eq!(s.len_utf16(), 103);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn len_utf16_02() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);
        assert_eq!(s.len_utf16(), 111);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn len_utf16_03() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(16..39);
        assert_eq!(s.len_utf16(), 21);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn len_utf16_04() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        assert_eq!(s.len_utf16(), 2);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn len_utf16_05() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        assert_eq!(s.len_utf16(), 0);
    }

    #[test]
    fn is_char_boundary_01() {
        let r = Rope::from_str(TEXT);
        assert!(r.is_char_boundary(0));
        assert!(r.is_char_boundary(127));

        let s = r.slice(7..103);
        let t = &TEXT[7..103];
        for i in 0..s.len() {
            assert_eq!(t.is_char_boundary(i), s.is_char_boundary(i));
        }
    }

    #[test]
    fn floor_char_boundary_01() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(3..137);

        assert_eq!(0, s.floor_char_boundary(0));
        assert_eq!(1, s.floor_char_boundary(1));
        assert_eq!(2, s.floor_char_boundary(2));

        assert_eq!(9, s.floor_char_boundary(9));
        assert_eq!(9, s.floor_char_boundary(10));
        assert_eq!(9, s.floor_char_boundary(11));
        assert_eq!(9, s.floor_char_boundary(12));
        assert_eq!(13, s.floor_char_boundary(13));

        assert_eq!(104, s.floor_char_boundary(104));
        assert_eq!(104, s.floor_char_boundary(105));
        assert_eq!(104, s.floor_char_boundary(106));
        assert_eq!(107, s.floor_char_boundary(107));

        assert_eq!(134, s.floor_char_boundary(134));
    }

    #[test]
    fn ceil_char_boundary_01() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(3..137);

        assert_eq!(0, s.ceil_char_boundary(0));
        assert_eq!(1, s.floor_char_boundary(1));
        assert_eq!(2, s.floor_char_boundary(2));

        assert_eq!(9, s.ceil_char_boundary(9));
        assert_eq!(13, s.ceil_char_boundary(10));
        assert_eq!(13, s.ceil_char_boundary(11));
        assert_eq!(13, s.ceil_char_boundary(12));
        assert_eq!(13, s.ceil_char_boundary(13));

        assert_eq!(104, s.ceil_char_boundary(104));
        assert_eq!(107, s.ceil_char_boundary(105));
        assert_eq!(107, s.ceil_char_boundary(106));
        assert_eq!(107, s.ceil_char_boundary(107));

        assert_eq!(134, s.ceil_char_boundary(134));
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn byte_to_char_idx_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(88..124);

        assert_eq!("?  こんにちは、みんなさん", s);

        assert_eq!(0, s.byte_to_char_idx(0));
        assert_eq!(1, s.byte_to_char_idx(1));
        assert_eq!(2, s.byte_to_char_idx(2));

        assert_eq!(3, s.byte_to_char_idx(3));
        assert_eq!(3, s.byte_to_char_idx(4));
        assert_eq!(3, s.byte_to_char_idx(5));

        assert_eq!(4, s.byte_to_char_idx(6));
        assert_eq!(4, s.byte_to_char_idx(7));
        assert_eq!(4, s.byte_to_char_idx(8));

        assert_eq!(13, s.byte_to_char_idx(33));
        assert_eq!(13, s.byte_to_char_idx(34));
        assert_eq!(13, s.byte_to_char_idx(35));
        assert_eq!(14, s.byte_to_char_idx(36));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_idx_01() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        assert_eq!(0, s.byte_to_utf16_idx(0));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn byte_to_utf16_idx_02() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        s.byte_to_utf16_idx(1);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_idx_03() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        assert_eq!(0, s.byte_to_utf16_idx(0));
        assert_eq!(0, s.byte_to_utf16_idx(1));
        assert_eq!(0, s.byte_to_utf16_idx(2));
        assert_eq!(0, s.byte_to_utf16_idx(3));
        assert_eq!(2, s.byte_to_utf16_idx(4));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn byte_to_utf16_idx_04() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        s.byte_to_utf16_idx(5);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_idx_05() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);

        assert_eq!(0, s.byte_to_utf16_idx(0));

        assert_eq!(12, s.byte_to_utf16_idx(12));
        assert_eq!(14, s.byte_to_utf16_idx(16));

        assert_eq!(33, s.byte_to_utf16_idx(35));
        assert_eq!(35, s.byte_to_utf16_idx(39));

        assert_eq!(63, s.byte_to_utf16_idx(67));
        assert_eq!(65, s.byte_to_utf16_idx(71));

        assert_eq!(95, s.byte_to_utf16_idx(101));
        assert_eq!(97, s.byte_to_utf16_idx(105));

        assert_eq!(99, s.byte_to_utf16_idx(107));
        assert_eq!(100, s.byte_to_utf16_idx(110));

        assert_eq!(110, s.byte_to_utf16_idx(140));
        assert_eq!(111, s.byte_to_utf16_idx(143));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn char_to_utf16_06() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);
        s.byte_to_utf16_idx(144);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_idx_07() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..137);

        assert_eq!(0, s.byte_to_utf16_idx(0));

        assert_eq!(11, s.byte_to_utf16_idx(11));
        assert_eq!(13, s.byte_to_utf16_idx(15));

        assert_eq!(32, s.byte_to_utf16_idx(34));
        assert_eq!(34, s.byte_to_utf16_idx(38));

        assert_eq!(62, s.byte_to_utf16_idx(66));
        assert_eq!(64, s.byte_to_utf16_idx(70));

        assert_eq!(94, s.byte_to_utf16_idx(100));
        assert_eq!(96, s.byte_to_utf16_idx(104));

        assert_eq!(98, s.byte_to_utf16_idx(106));
        assert_eq!(99, s.byte_to_utf16_idx(109));

        assert_eq!(107, s.byte_to_utf16_idx(133));
        assert_eq!(108, s.byte_to_utf16_idx(136));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn byte_to_utf16_idx_08() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..137);
        s.byte_to_utf16_idx(137);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn byte_to_line_idx_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..112);

        assert_eq!(
            "'s a fine day, isn't it?\nAren't you glad \
             we're alive?\nこんにちは、みん",
            s,
        );

        assert_eq!(0, s.byte_to_line_idx(0, LineType::LF_CR));
        assert_eq!(0, s.byte_to_line_idx(1, LineType::LF_CR));

        assert_eq!(0, s.byte_to_line_idx(24, LineType::LF_CR));
        assert_eq!(1, s.byte_to_line_idx(25, LineType::LF_CR));
        assert_eq!(1, s.byte_to_line_idx(26, LineType::LF_CR));

        assert_eq!(1, s.byte_to_line_idx(53, LineType::LF_CR));
        assert_eq!(2, s.byte_to_line_idx(54, LineType::LF_CR));
        assert_eq!(2, s.byte_to_line_idx(57, LineType::LF_CR));

        assert_eq!(2, s.byte_to_line_idx(78, LineType::LF_CR));
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn byte_to_line_idx_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(50..50);
        assert_eq!(0, s.byte_to_line_idx(0, LineType::LF_CR));
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn byte_to_line_idx_03() {
        let r = Rope::from_str("Hi there\nstranger!");
        let s = r.slice(0..9);
        assert_eq!(0, s.byte_to_line_idx(0, LineType::LF_CR));
        assert_eq!(0, s.byte_to_line_idx(8, LineType::LF_CR));
        assert_eq!(1, s.byte_to_line_idx(9, LineType::LF_CR));
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[should_panic]
    fn byte_to_line_idx_04() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..112);
        s.byte_to_line_idx(79, LineType::LF_CR);
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn char_to_byte_idx_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(88..124);

        assert_eq!("?  こんにちは、みんなさん", s);

        assert_eq!(0, s.char_to_byte_idx(0));
        assert_eq!(1, s.char_to_byte_idx(1));
        assert_eq!(2, s.char_to_byte_idx(2));

        assert_eq!(3, s.char_to_byte_idx(3));
        assert_eq!(6, s.char_to_byte_idx(4));
        assert_eq!(33, s.char_to_byte_idx(13));
        assert_eq!(36, s.char_to_byte_idx(14));
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn line_to_byte_idx_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..112);

        assert_eq!(
            "'s a fine day, isn't it?\nAren't you glad \
             we're alive?\nこんにちは、みん",
            s,
        );

        assert_eq!(0, s.line_to_byte_idx(0, LineType::LF_CR));
        assert_eq!(25, s.line_to_byte_idx(1, LineType::LF_CR));
        assert_eq!(54, s.line_to_byte_idx(2, LineType::LF_CR));
        assert_eq!(78, s.line_to_byte_idx(3, LineType::LF_CR));
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn line_to_byte_idx_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);

        assert_eq!(0, s.line_to_byte_idx(0, LineType::LF_CR));
        assert_eq!(0, s.line_to_byte_idx(1, LineType::LF_CR));
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[should_panic]
    fn line_to_byte_idx_03() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        s.line_to_byte_idx(4, LineType::LF_CR);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[should_panic]
    fn line_to_byte_idx_04() {
        let r = Rope::from_str("\n\n\n\n");
        let s = r.slice(1..3);

        s.line_to_byte_idx(4, LineType::LF_CR);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_idx_01() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        assert_eq!(0, s.utf16_to_byte_idx(0));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn utf16_to_byte_idx_02() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        s.utf16_to_byte_idx(1);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_idx_03() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        assert_eq!(0, s.utf16_to_byte_idx(0));
        assert_eq!(0, s.utf16_to_byte_idx(1));
        assert_eq!(4, s.utf16_to_byte_idx(2));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn utf16_to_byte_idx_04() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        s.utf16_to_byte_idx(3);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_idx_05() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);

        assert_eq!(0, s.utf16_to_byte_idx(0));

        assert_eq!(12, s.utf16_to_byte_idx(12));
        assert_eq!(16, s.utf16_to_byte_idx(14));

        assert_eq!(35, s.utf16_to_byte_idx(33));
        assert_eq!(39, s.utf16_to_byte_idx(35));

        assert_eq!(67, s.utf16_to_byte_idx(63));
        assert_eq!(71, s.utf16_to_byte_idx(65));

        assert_eq!(101, s.utf16_to_byte_idx(95));
        assert_eq!(105, s.utf16_to_byte_idx(97));

        assert_eq!(107, s.utf16_to_byte_idx(99));
        assert_eq!(110, s.utf16_to_byte_idx(100));

        assert_eq!(140, s.utf16_to_byte_idx(110));
        assert_eq!(143, s.utf16_to_byte_idx(111));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn utf16_to_byte_idx_06() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);
        s.utf16_to_byte_idx(112);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_idx_07() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..137);

        assert_eq!(0, s.utf16_to_byte_idx(0));

        assert_eq!(11, s.utf16_to_byte_idx(11));
        assert_eq!(15, s.utf16_to_byte_idx(13));

        assert_eq!(34, s.utf16_to_byte_idx(32));
        assert_eq!(38, s.utf16_to_byte_idx(34));

        assert_eq!(66, s.utf16_to_byte_idx(62));
        assert_eq!(70, s.utf16_to_byte_idx(64));

        assert_eq!(100, s.utf16_to_byte_idx(94));
        assert_eq!(104, s.utf16_to_byte_idx(96));

        assert_eq!(106, s.utf16_to_byte_idx(98));
        assert_eq!(109, s.utf16_to_byte_idx(99));

        assert_eq!(133, s.utf16_to_byte_idx(107));
        assert_eq!(136, s.utf16_to_byte_idx(108));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn utf16_to_byte_idx_08() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..137);
        s.utf16_to_byte_idx(109);
    }

    #[test]
    fn byte_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..118);

        assert_eq!(s.byte(0), b't');
        assert_eq!(s.byte(10), b' ');

        // UTF-8 encoding of 'な'.
        assert_eq!(s.byte(s.len() - 3), 0xE3);
        assert_eq!(s.byte(s.len() - 2), 0x81);
        assert_eq!(s.byte(s.len() - 1), 0xAA);
    }

    #[test]
    #[should_panic]
    fn byte_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..118);
        s.byte(s.len());
    }

    #[test]
    #[should_panic]
    fn byte_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(42..42);
        s.byte(0);
    }

    #[test]
    fn char_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..118);

        // t's \
        // a fine day, isn't it?  Aren't you glad \
        // we're alive?  こんにちは、みんな

        assert_eq!(s.char(0), 't');
        assert_eq!(s.char(10), ' ');
        assert_eq!(s.char(18), 'n');
        assert_eq!(s.char(81), 'な');
    }

    #[test]
    #[should_panic]
    fn char_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..118);
        s.char(s.len());
    }

    #[test]
    #[should_panic]
    fn char_03() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);
        s.char(0);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn line_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..112);
        // "'s a fine day, isn't it?\nAren't you glad \
        //  we're alive?\nこんにちは、みん"

        let l0 = s.line(0, LineType::LF_CR);
        assert_eq!(l0, "'s a fine day, isn't it?\n");
        assert_eq!(l0.len(), 25);
        assert_eq!(l0.len_lines(LineType::LF_CR), 2);

        let l1 = s.line(1, LineType::LF_CR);
        assert_eq!(l1, "Aren't you glad we're alive?\n");
        assert_eq!(l1.len(), 29);
        assert_eq!(l1.len_lines(LineType::LF_CR), 2);

        let l2 = s.line(2, LineType::LF_CR);
        assert_eq!(l2, "こんにちは、みん");
        assert_eq!(l2.len(), 24);
        assert_eq!(l2.len_lines(LineType::LF_CR), 1);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn line_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..59);
        // "'s a fine day, isn't it?\n"

        assert_eq!(s.line(0, LineType::LF_CR), "'s a fine day, isn't it?\n");
        assert_eq!(s.line(1, LineType::LF_CR), "");
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn line_03() {
        let r = Rope::from_str("Hi\nHi\nHi\nHi\nHi\nHi\n");
        let s = r.slice(1..17);

        assert_eq!(s.line(0, LineType::LF_CR), "i\n");
        assert_eq!(s.line(1, LineType::LF_CR), "Hi\n");
        assert_eq!(s.line(2, LineType::LF_CR), "Hi\n");
        assert_eq!(s.line(3, LineType::LF_CR), "Hi\n");
        assert_eq!(s.line(4, LineType::LF_CR), "Hi\n");
        assert_eq!(s.line(5, LineType::LF_CR), "Hi");
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn line_04() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);

        assert_eq!(s.line(0, LineType::LF_CR), "");
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    #[should_panic]
    fn line_05() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);
        s.line(3, LineType::LF_CR);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn line_06() {
        let r = Rope::from_str("1\n2\n3\n4\n5\n6\n7\n8");
        let s = r.slice(1..11);
        // "\n2\n3\n4\n5\n6"

        assert_eq!(s.line(0, LineType::LF_CR).len_lines(LineType::LF_CR), 2);
        assert_eq!(s.line(1, LineType::LF_CR).len_lines(LineType::LF_CR), 2);
        assert_eq!(s.line(2, LineType::LF_CR).len_lines(LineType::LF_CR), 2);
        assert_eq!(s.line(3, LineType::LF_CR).len_lines(LineType::LF_CR), 2);
        assert_eq!(s.line(4, LineType::LF_CR).len_lines(LineType::LF_CR), 2);
        assert_eq!(s.line(5, LineType::LF_CR).len_lines(LineType::LF_CR), 1);
    }

    #[cfg(feature = "metric_lines_lf")]
    #[test]
    fn trailing_line_break_idx_lf_01() {
        use LineType::LF;
        let r = Rope::from_str("Hello\u{2029}\u{2028}\u{85}\u{0C}\u{0B}\r\r\n\n");

        assert_eq!(Some(18), r.slice(..19).trailing_line_break_idx(LF));
        assert_eq!(Some(16), r.slice(..18).trailing_line_break_idx(LF));
        assert_eq!(None, r.slice(..16).trailing_line_break_idx(LF));
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn trailing_line_break_idx_lf_cr_01() {
        use LineType::LF_CR;
        let r = Rope::from_str("Hello\u{2029}\u{2028}\u{85}\u{0C}\u{0B}\r\r\n\n");

        assert_eq!(Some(18), r.slice(..19).trailing_line_break_idx(LF_CR));
        assert_eq!(Some(16), r.slice(..18).trailing_line_break_idx(LF_CR));
        assert_eq!(Some(15), r.slice(..16).trailing_line_break_idx(LF_CR));
        assert_eq!(None, r.slice(..15).trailing_line_break_idx(LF_CR));
    }

    #[cfg(feature = "metric_lines_unicode")]
    #[test]
    fn trailing_line_break_idx_unicode_01() {
        use LineType::All;
        let r = Rope::from_str("Hello\u{2029}\u{2028}\u{85}\u{0C}\u{0B}\r\r\n\n");

        assert_eq!(Some(18), r.slice(..19).trailing_line_break_idx(All));
        assert_eq!(Some(16), r.slice(..18).trailing_line_break_idx(All));
        assert_eq!(Some(15), r.slice(..16).trailing_line_break_idx(All));
        assert_eq!(Some(14), r.slice(..15).trailing_line_break_idx(All));
        assert_eq!(Some(13), r.slice(..14).trailing_line_break_idx(All));
        assert_eq!(Some(11), r.slice(..13).trailing_line_break_idx(All));
        assert_eq!(Some(8), r.slice(..11).trailing_line_break_idx(All));
        assert_eq!(Some(5), r.slice(..8).trailing_line_break_idx(All));
        assert_eq!(None, r.slice(..5).trailing_line_break_idx(All));
    }

    fn test_chunk(s: RopeSlice, text: &str) {
        let mut current_byte = 0;
        let mut seen_bytes = 0;
        let mut prev_byte = 0;
        for i in 0..s.len() {
            let (chunk, start_byte) = s.chunk(i);

            if start_byte != prev_byte || i == 0 {
                current_byte = seen_bytes;
                seen_bytes += chunk.len();

                prev_byte = start_byte;
            }

            assert_eq!(start_byte, current_byte);
            assert_eq!(chunk, &text[current_byte..seen_bytes]);
        }

        assert_eq!(seen_bytes, text.len());
    }

    #[test]
    fn chunk_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..112);
        let text = &TEXT_LINES[34..112];
        // "'s a fine day, isn't it?\nAren't you glad \
        //  we're alive?\nこんにちは、みん"

        test_chunk(s, text);
    }

    #[test]
    fn chunk_02() {
        // Make sure splitting LF_CR pairs works properly.

        let (r, text) = make_rope_and_text_from_chunks(&[
            "\r\n\r\n\r\n",
            "\r\n\r\n\r",
            "\n\r\n\r\n\r",
            "\n\r\n\r\n\r\n",
            "\r\n\r\n\r\n",
        ]);

        for si in 0..=r.len() {
            test_chunk(r.slice(si..), &text[si..]);
        }
    }

    // #[test]
    // fn chunk_at_char() {
    //     let r = Rope::from_str(TEXT_LINES);
    //     let s = r.slice(34..96);
    //     let text = &TEXT_LINES[34..112];
    //     // "'s a fine day, isn't it?\nAren't you glad \
    //     //  we're alive?\nこんにちは、みん"

    //     let mut t = text;
    //     let mut prev_chunk = "";
    //     for i in 0..s.len_chars() {
    //         let (chunk, b, c, l) = s.chunk_at_char(i);
    //         assert_eq!(b, char_to_byte_idx_idx(text, c));
    //         assert_eq!(l, char_to_line_idx(text, c));
    //         if chunk != prev_chunk {
    //             assert_eq!(chunk, &t[..chunk.len()]);
    //             t = &t[chunk.len()..];
    //             prev_chunk = chunk;
    //         }

    //         let c1 = text.chars().nth(i).unwrap();
    //         let c2 = {
    //             let i2 = i - c;
    //             chunk.chars().nth(i2).unwrap()
    //         };
    //         assert_eq!(c1, c2);
    //     }
    //     assert_eq!(t.len(), 0);
    // }

    // #[test]
    // fn chunk_at_line_break() {
    //     let r = Rope::from_str(TEXT_LINES);
    //     let s = r.slice(34..96);
    //     let text = &TEXT_LINES[34..112];
    //     // "'s a fine day, isn't it?\nAren't you glad \
    //     //  we're alive?\nこんにちは、みん"

    //     // First chunk
    //     {
    //         let (chunk, b, c, l) = s.chunk_at_line_break(0);
    //         assert_eq!(chunk, &text[..chunk.len()]);
    //         assert_eq!(b, 0);
    //         assert_eq!(c, 0);
    //         assert_eq!(l, 0);
    //     }

    //     // Middle chunks
    //     for i in 1..s.len_lines() {
    //         let (chunk, b, c, l) = s.chunk_at_line_break(i);
    //         assert_eq!(chunk, &text[b..(b + chunk.len())]);
    //         assert_eq!(c, byte_to_char_idx(text, b));
    //         assert_eq!(l, byte_to_line_idx(text, b));
    //         assert!(l < i);
    //         assert!(i <= byte_to_line_idx(text, b + chunk.len()));
    //     }

    //     // Last chunk
    //     {
    //         let (chunk, b, c, l) = s.chunk_at_line_break(s.len_lines());
    //         assert_eq!(chunk, &text[(text.len() - chunk.len())..]);
    //         assert_eq!(chunk, &text[b..]);
    //         assert_eq!(c, byte_to_char_idx(text, b));
    //         assert_eq!(l, byte_to_line_idx(text, b));
    //     }
    // }

    #[test]
    fn slice_01() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(..);

        let s2 = s1.slice(..);

        assert_eq!(TEXT, s2);
    }

    #[test]
    fn slice_02() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(50..118);

        let s2 = s1.slice(3..25);

        assert_eq!(&TEXT[53..75], s2);
    }

    #[test]
    fn slice_03() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(50..118);

        let s2 = s1.slice(7..65);

        assert_eq!(&TEXT[57..115], s2);
    }

    #[test]
    fn slice_04() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(50..118);

        let s2 = s1.slice(21..21);

        assert_eq!("", s2);
    }

    #[test]
    #[should_panic]
    fn slice_05() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(50..118);

        #[allow(clippy::reversed_empty_ranges)]
        s.slice(21..20); // Wrong ordering on purpose.
    }

    #[test]
    #[should_panic]
    fn slice_06() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(50..85);

        s.slice(35..36);
    }

    #[test]
    #[should_panic]
    fn slice_07() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(50..118);

        // Not a char boundary.
        s.slice(..43);
    }

    #[test]
    #[should_panic]
    fn slice_08() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(50..118);

        // Not a char boundary.
        s.slice(43..);
    }

    #[test]
    fn eq_str_01() {
        let r = Rope::from_str(TEXT);
        let slice = r.slice(..);

        assert_eq!(slice, TEXT);
        assert_eq!(TEXT, slice);
    }

    #[test]
    fn eq_str_02() {
        let r = Rope::from_str(TEXT);
        let slice = r.slice(0..20);

        assert_ne!(slice, TEXT);
        assert_ne!(TEXT, slice);
    }

    #[test]
    fn eq_str_03() {
        let mut r = Rope::from_str(TEXT);
        r.remove(20..21);
        r.insert(20, "z");
        let slice = r.slice(..);

        assert_ne!(slice, TEXT);
        assert_ne!(TEXT, slice);
    }

    #[test]
    fn eq_string_01() {
        let r = Rope::from_str(TEXT);
        let slice = r.slice(..);
        let s: String = TEXT.into();

        assert_eq!(slice, s);
        assert_eq!(s, slice);
    }

    #[test]
    fn eq_rope_slice_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);

        assert_eq!(s, s);
    }

    #[test]
    fn eq_rope_slice_02() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(43..97);
        let s2 = r.slice(43..97);

        assert_eq!(s1, s2);
    }

    #[test]
    fn eq_rope_slice_03() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(43..43);
        let s2 = r.slice(43..45);

        assert_ne!(s1, s2);
    }

    #[test]
    fn eq_rope_slice_04() {
        let r = Rope::from_str(TEXT);
        let s1 = r.slice(43..45);
        let s2 = r.slice(43..43);

        assert_ne!(s1, s2);
    }

    #[test]
    fn eq_rope_slice_05() {
        let r = Rope::from_str("");
        let s = r.slice(0..0);

        assert_eq!(s, s);
    }

    #[test]
    fn cmp_rope_slice_01() {
        let r1 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
        let r2 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
        let s1 = r1.slice(..);
        let s2 = r2.slice(..);

        assert_eq!(s1.cmp(&s2), std::cmp::Ordering::Equal);
        assert_eq!(s1.slice(..24).cmp(&s2), std::cmp::Ordering::Less);
        assert_eq!(s1.cmp(&s2.slice(..24)), std::cmp::Ordering::Greater);
    }

    #[test]
    fn cmp_rope_slice_02() {
        let r1 = Rope::from_str("abcdefghijklmnzpqrstuvwxyz");
        let r2 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
        let s1 = r1.slice(..);
        let s2 = r2.slice(..);

        assert_eq!(s1.cmp(&s2), std::cmp::Ordering::Greater);
        assert_eq!(s2.cmp(&s1), std::cmp::Ordering::Less);
    }

    #[test]
    fn to_string_01() {
        let r = Rope::from_str(TEXT);
        let slc = r.slice(..);
        let s: String = slc.into();

        assert_eq!(r, s);
        assert_eq!(slc, s);
    }

    #[test]
    fn to_string_02() {
        let r = Rope::from_str(TEXT);
        let slc = r.slice(0..24);
        let s: String = slc.into();

        assert_eq!(slc, s);
    }

    #[test]
    fn to_string_03() {
        let r = Rope::from_str(TEXT);
        let slc = r.slice(13..89);
        let s: String = slc.into();

        assert_eq!(slc, s);
    }

    #[test]
    fn to_string_04() {
        let r = Rope::from_str(TEXT);
        let slc = r.slice(13..41);
        let s: String = slc.into();

        assert_eq!(slc, s);
    }

    #[test]
    fn to_cow_01() {
        use std::borrow::Cow;
        let r = Rope::from_str(TEXT);
        let s = r.slice(13..83);
        let cow: Cow<str> = s.into();

        assert_eq!(s, cow);
    }

    #[test]
    fn to_cow_02() {
        use std::borrow::Cow;
        let r = Rope::from_str(TEXT);
        let s = r.slice(13..14);
        let cow: Cow<str> = r.slice(13..14).into();

        assert!(s.root.is_leaf());

        // Make sure it's borrowed.
        if let Cow::Owned(_) = cow {
            panic!("Small Cow conversions should result in a borrow.");
        }

        assert_eq!(s, cow);
    }

    #[test]
    fn hash_01() {
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        let r = Rope::from_str("Hello there!");
        let s = r.slice(..);

        r.hash(&mut h1);
        s.hash(&mut h2);

        assert_eq!(h1.finish(), h2.finish());
    }

    // Iterator tests are in the iter module
}
