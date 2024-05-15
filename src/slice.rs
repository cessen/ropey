use std::ops::RangeBounds;

use crate::{end_bound_to_num, start_bound_to_num, Rope};

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
use crate::LineType;

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
    rope: &'a Rope,
    byte_range: [usize; 2],
}

impl<'a> RopeSlice<'a> {
    //---------------------------------------------------------
    // Constructors.

    #[inline(always)]
    pub(crate) fn new(rope: &'a Rope, byte_range: [usize; 2]) -> Self {
        Self {
            rope: rope,
            byte_range: byte_range,
        }
    }

    //---------------------------------------------------------
    // Queries.

    #[inline(always)]
    pub fn len_bytes(&self) -> usize {
        self.byte_range[1] - self.byte_range[0]
    }

    #[cfg(feature = "metric_chars")]
    #[inline]
    pub fn len_chars(&self) -> usize {
        let char_start_idx = self.rope.byte_to_char(self.byte_range[0]);
        let char_end_idx = self.rope.byte_to_char(self.byte_range[1]);
        char_end_idx - char_start_idx
    }

    #[cfg(feature = "metric_utf16")]
    #[inline]
    pub fn len_utf16(&self) -> usize {
        let utf16_start_idx = self.rope.byte_to_utf16(self.byte_range[0]);
        let utf16_end_idx = self.rope.byte_to_utf16(self.byte_range[1]);
        utf16_end_idx - utf16_start_idx
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[inline]
    pub fn len_lines(&self, line_type: LineType) -> usize {
        let line_start_idx = self.rope.byte_to_line(self.byte_range[0], line_type);
        let line_end_idx = self.rope.byte_to_line(self.byte_range[1], line_type);
        let ends_with_crlf_split = self
            .rope
            .is_relevant_crlf_split(self.byte_range[1], line_type);

        line_end_idx - line_start_idx + 1 + ends_with_crlf_split as usize
    }

    #[inline]
    pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
        assert!(byte_idx <= self.len_bytes());

        self.rope.is_char_boundary(self.byte_range[0] + byte_idx)
    }

    //---------------------------------------------------------
    // Metric conversions.

    #[cfg(feature = "metric_chars")]
    #[inline]
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        assert!(byte_idx <= self.len_bytes());

        self.rope.byte_to_char(self.byte_range[0] + byte_idx)
            - self.rope.byte_to_char(self.byte_range[0])
    }

    #[cfg(feature = "metric_chars")]
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        assert!(char_idx <= self.len_chars());

        let char_start_idx = self.rope.byte_to_char(self.byte_range[0]);
        self.rope.char_to_byte(char_start_idx + char_idx) - self.byte_range[0]
    }

    #[cfg(feature = "metric_utf16")]
    #[inline]
    pub fn byte_to_utf16(&self, byte_idx: usize) -> usize {
        assert!(byte_idx <= self.len_bytes());

        self.rope.byte_to_utf16(self.byte_range[0] + byte_idx)
            - self.rope.byte_to_utf16(self.byte_range[0])
    }

    #[cfg(feature = "metric_utf16")]
    pub fn utf16_to_byte(&self, utf16_idx: usize) -> usize {
        assert!(utf16_idx <= self.len_utf16());

        let utf16_start_idx = self.rope.byte_to_utf16(self.byte_range[0]);
        self.rope.utf16_to_byte(utf16_start_idx + utf16_idx) - self.byte_range[0]
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[inline]
    pub fn byte_to_line(&self, byte_idx: usize, line_type: LineType) -> usize {
        assert!(byte_idx <= self.len_bytes());

        let crlf_split = if byte_idx == self.byte_range[1] {
            self.rope
                .is_relevant_crlf_split(self.byte_range[1], line_type)
        } else {
            false
        };

        self.rope
            .byte_to_line(self.byte_range[0] + byte_idx, line_type)
            - self.rope.byte_to_line(self.byte_range[0], line_type)
            + crlf_split as usize
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    pub fn line_to_byte(&self, line_idx: usize, line_type: LineType) -> usize {
        assert!(line_idx <= self.len_lines(line_type));

        let line_start_idx = self.rope.byte_to_line(self.byte_range[0], line_type);
        self.rope
            .line_to_byte(line_start_idx + line_idx, line_type)
            .saturating_sub(self.byte_range[0])
            .min(self.len_bytes())
    }

    //---------------------------------------------------------
    // Slicing.

    #[inline]
    pub fn slice<R>(&self, byte_range: R) -> Self
    where
        R: RangeBounds<usize>,
    {
        let start_idx = start_bound_to_num(byte_range.start_bound()).unwrap_or(0);
        let end_idx = end_bound_to_num(byte_range.end_bound()).unwrap_or_else(|| self.len_bytes());
        assert!(
            start_idx <= end_idx && end_idx <= self.len_bytes(),
            "Invalid byte range: either end < start or the range is outside the bounds of the rope slice.",
        );

        let start_idx_real = self.byte_range[0] + start_idx;
        let end_idx_real = self.byte_range[0] + end_idx;
        assert!(
            self.rope.is_char_boundary(start_idx_real) && self.rope.is_char_boundary(end_idx_real),
            "Byte range does not align with char boundaries."
        );

        RopeSlice::new(self.rope, [start_idx_real, end_idx_real])
    }
}

//==============================================================
// Comparison impls.

// impl std::cmp::Eq for RopeSlice<'_> {}

impl std::cmp::PartialEq<&str> for RopeSlice<'_> {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        if self.len_bytes() != other.len() {
            return false;
        }
        let other = other.as_bytes();

        // TODO: this is temporary, just to get things working.  It iterates
        // over all the chunks in the underlying rope, which is inefficient.
        // This can be both simplified and optimized by changing to the same
        // code as the Rope<->str comparison code after `Chunks` is implemented
        // for RopeSlices.
        let mut text_idx = 0;
        let mut chunk_start = 0;
        for chunk in self.rope.chunks() {
            if chunk_start + chunk.len() <= self.byte_range[0] {
                chunk_start += chunk.len();
                continue;
            }
            if text_idx >= other.len() {
                break;
            }

            let chunk_bytes = {
                let mut chunk_bytes = chunk.as_bytes();
                if chunk_start < self.byte_range[0] {
                    chunk_bytes = &chunk_bytes[(self.byte_range[0] - chunk_start)..];
                }
                &chunk_bytes[..chunk_bytes.len().min(other.len() - text_idx)]
            };

            if chunk_bytes != &other[text_idx..(text_idx + chunk_bytes.len())] {
                return false;
            }
            text_idx += chunk_bytes.len();
            chunk_start += chunk.len();
        }

        return true;
    }
}

impl std::cmp::PartialEq<RopeSlice<'_>> for &str {
    #[inline(always)]
    fn eq(&self, other: &RopeSlice) -> bool {
        other == self
    }
}

impl std::cmp::PartialEq<str> for RopeSlice<'_> {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        std::cmp::PartialEq::<&str>::eq(self, &other)
    }
}

impl std::cmp::PartialEq<RopeSlice<'_>> for str {
    #[inline(always)]
    fn eq(&self, other: &RopeSlice) -> bool {
        std::cmp::PartialEq::<&str>::eq(other, &self)
    }
}

impl std::cmp::PartialEq<String> for RopeSlice<'_> {
    #[inline(always)]
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

impl std::cmp::PartialEq<RopeSlice<'_>> for String {
    #[inline(always)]
    fn eq(&self, other: &RopeSlice) -> bool {
        other == self.as_str()
    }
}

impl std::cmp::PartialEq<std::borrow::Cow<'_, str>> for RopeSlice<'_> {
    #[inline]
    fn eq(&self, other: &std::borrow::Cow<str>) -> bool {
        *self == **other
    }
}

impl std::cmp::PartialEq<RopeSlice<'_>> for std::borrow::Cow<'_, str> {
    #[inline]
    fn eq(&self, other: &RopeSlice) -> bool {
        *other == **self
    }
}

//==============================================================
// Other impls.

impl std::fmt::Debug for RopeSlice<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // TODO: this is temporary, just to get things working.  It iterates
        // over all the chunks in the underlying rope, which is inefficient.
        // This can be both simplified and optimized by changing to the same
        // code as the Rope::Debug impl after `Chunks` is implemented for
        // RopeSlices.
        let mut debug_list = f.debug_list();
        let mut chunk_start = 0;
        for chunk in self.rope.chunks() {
            if chunk_start + chunk.len() <= self.byte_range[0] {
                chunk_start += chunk.len();
                continue;
            }
            if chunk_start >= self.byte_range[1] {
                break;
            }

            {
                let mut chunk: &str = chunk;
                if chunk_start + chunk.len() > self.byte_range[1] {
                    chunk = &chunk[..(self.byte_range[1] - chunk_start)];
                }
                if chunk_start < self.byte_range[0] {
                    chunk = &chunk[(self.byte_range[0] - chunk_start)..];
                }
                debug_list.entry(&chunk);
            }

            chunk_start += chunk.len();
        }
        debug_list.finish()
    }
}

impl std::fmt::Display for RopeSlice<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // TODO: this is temporary, just to get things working.  It iterates
        // over all the chunks in the underlying rope, which is inefficient.
        // This can be both simplified and optimized by changing to the same
        // code as the Rope::Display impl after `Chunks` is implemented for
        // RopeSlices.
        let mut chunk_start = 0;
        for chunk in self.rope.chunks() {
            if chunk_start + chunk.len() <= self.byte_range[0] {
                chunk_start += chunk.len();
                continue;
            }
            if chunk_start >= self.byte_range[1] {
                break;
            }

            {
                let mut chunk: &str = chunk;
                if chunk_start + chunk.len() > self.byte_range[1] {
                    chunk = &chunk[..(self.byte_range[1] - chunk_start)];
                }
                if chunk_start < self.byte_range[0] {
                    chunk = &chunk[(self.byte_range[0] - chunk_start)..];
                }

                write!(f, "{}", chunk)?;
            }

            chunk_start += chunk.len();
        }
        Ok(())
    }
}

//=============================================================

#[cfg(test)]
mod tests {
    use crate::rope_builder::RopeBuilder;
    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_lf"))]
    use crate::LineType;

    // use crate::str_utils::{
    //     byte_to_char_idx, byte_to_line_idx, char_to_byte_idx, char_to_line_idx,
    // };
    use crate::Rope;
    // use std::hash::{Hash, Hasher};

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

    #[test]
    fn len_bytes_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(7..97);
        assert_eq!(s.len_bytes(), 90);
    }

    #[test]
    fn len_bytes_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(43..43);
        assert_eq!(s.len_bytes(), 0);
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

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    fn len_lines_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..97);
        assert_eq!(s.len_lines(LineType::CRLF), 3);
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    fn len_lines_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);
        assert_eq!(s.len_lines(LineType::CRLF), 1);
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_lf"))]
    #[test]
    fn len_lines_03() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk("\r\n\r\n\r\n");
            rb._append_chunk("\r\n\r\n\r");
            rb._append_chunk("\n\r\n\r\n\r");
            rb._append_chunk("\n\r\n\r\n\r\n");
            rb._append_chunk("\r\n\r\n\r\n");
            rb.finish()
        };
        for i in 0..=r.len_bytes() {
            #[cfg(feature = "metric_lines_cr_lf")]
            assert_eq!(r.slice(..i).len_lines(LineType::CRLF), 1 + ((i + 1) / 2));

            #[cfg(feature = "metric_lines_lf")]
            assert_eq!(r.slice(..i).len_lines(LineType::LF), 1 + (i / 2));
        }
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_lf"))]
    #[test]
    fn len_lines_04() {
        // Make sure splitting CRLF pairs at the start works properly.
        let r = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk("\r\n\r\n\r\n");
            rb._append_chunk("\r\n\r\n\r");
            rb._append_chunk("\n\r\n\r\n\r");
            rb._append_chunk("\n\r\n\r\n\r\n");
            rb._append_chunk("\r\n\r\n\r\n");
            rb.finish()
        };
        for i in 0..=r.len_bytes() {
            #[cfg(feature = "metric_lines_cr_lf")]
            assert_eq!(r.slice(i..).len_lines(LineType::CRLF), 16 - (i / 2));

            #[cfg(feature = "metric_lines_lf")]
            assert_eq!(r.slice(i..).len_lines(LineType::LF), 16 - (i / 2));
        }
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

    #[cfg(feature = "metric_chars")]
    #[test]
    fn byte_to_char_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(88..124);

        assert_eq!("?  こんにちは、みんなさん", s);

        assert_eq!(0, s.byte_to_char(0));
        assert_eq!(1, s.byte_to_char(1));
        assert_eq!(2, s.byte_to_char(2));

        assert_eq!(3, s.byte_to_char(3));
        assert_eq!(3, s.byte_to_char(4));
        assert_eq!(3, s.byte_to_char(5));

        assert_eq!(4, s.byte_to_char(6));
        assert_eq!(4, s.byte_to_char(7));
        assert_eq!(4, s.byte_to_char(8));

        assert_eq!(13, s.byte_to_char(33));
        assert_eq!(13, s.byte_to_char(34));
        assert_eq!(13, s.byte_to_char(35));
        assert_eq!(14, s.byte_to_char(36));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_01() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        assert_eq!(0, s.byte_to_utf16(0));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn byte_to_utf16_02() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        s.byte_to_utf16(1);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_03() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        assert_eq!(0, s.byte_to_utf16(0));
        assert_eq!(0, s.byte_to_utf16(1));
        assert_eq!(0, s.byte_to_utf16(2));
        assert_eq!(0, s.byte_to_utf16(3));
        assert_eq!(2, s.byte_to_utf16(4));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn byte_to_utf16_04() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        s.byte_to_utf16(5);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_05() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);

        assert_eq!(0, s.byte_to_utf16(0));

        assert_eq!(12, s.byte_to_utf16(12));
        assert_eq!(14, s.byte_to_utf16(16));

        assert_eq!(33, s.byte_to_utf16(35));
        assert_eq!(35, s.byte_to_utf16(39));

        assert_eq!(63, s.byte_to_utf16(67));
        assert_eq!(65, s.byte_to_utf16(71));

        assert_eq!(95, s.byte_to_utf16(101));
        assert_eq!(97, s.byte_to_utf16(105));

        assert_eq!(99, s.byte_to_utf16(107));
        assert_eq!(100, s.byte_to_utf16(110));

        assert_eq!(110, s.byte_to_utf16(140));
        assert_eq!(111, s.byte_to_utf16(143));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn char_to_utf16_06() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);
        s.byte_to_utf16(144);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_07() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..137);

        assert_eq!(0, s.byte_to_utf16(0));

        assert_eq!(11, s.byte_to_utf16(11));
        assert_eq!(13, s.byte_to_utf16(15));

        assert_eq!(32, s.byte_to_utf16(34));
        assert_eq!(34, s.byte_to_utf16(38));

        assert_eq!(62, s.byte_to_utf16(66));
        assert_eq!(64, s.byte_to_utf16(70));

        assert_eq!(94, s.byte_to_utf16(100));
        assert_eq!(96, s.byte_to_utf16(104));

        assert_eq!(98, s.byte_to_utf16(106));
        assert_eq!(99, s.byte_to_utf16(109));

        assert_eq!(107, s.byte_to_utf16(133));
        assert_eq!(108, s.byte_to_utf16(136));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn byte_to_utf16_08() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..137);
        s.byte_to_utf16(137);
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    fn byte_to_line_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..112);

        assert_eq!(
            "'s a fine day, isn't it?\nAren't you glad \
             we're alive?\nこんにちは、みん",
            s,
        );

        assert_eq!(0, s.byte_to_line(0, LineType::CRLF));
        assert_eq!(0, s.byte_to_line(1, LineType::CRLF));

        assert_eq!(0, s.byte_to_line(24, LineType::CRLF));
        assert_eq!(1, s.byte_to_line(25, LineType::CRLF));
        assert_eq!(1, s.byte_to_line(26, LineType::CRLF));

        assert_eq!(1, s.byte_to_line(53, LineType::CRLF));
        assert_eq!(2, s.byte_to_line(54, LineType::CRLF));
        assert_eq!(2, s.byte_to_line(57, LineType::CRLF));

        assert_eq!(2, s.byte_to_line(78, LineType::CRLF));
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    fn byte_to_line_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(50..50);
        assert_eq!(0, s.byte_to_line(0, LineType::CRLF));
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    fn byte_to_line_03() {
        let r = Rope::from_str("Hi there\nstranger!");
        let s = r.slice(0..9);
        assert_eq!(0, s.byte_to_line(0, LineType::CRLF));
        assert_eq!(0, s.byte_to_line(8, LineType::CRLF));
        assert_eq!(1, s.byte_to_line(9, LineType::CRLF));
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[should_panic]
    fn byte_to_line_04() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..112);
        s.byte_to_line(79, LineType::CRLF);
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_lf"))]
    #[test]
    fn byte_to_line_05() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk("\r\n\r\n\r\n");
            rb._append_chunk("\r\n\r\n\r");
            rb._append_chunk("\n\r\n\r\n\r");
            rb._append_chunk("\n\r\n\r\n\r\n");
            rb._append_chunk("\r\n\r\n\r\n");
            rb.finish()
        };
        for si in 0..=r.len_bytes() {
            let s = r.slice(..si);
            for i in 0..s.len_bytes() {
                #[cfg(feature = "metric_lines_cr_lf")]
                assert_eq!(s.byte_to_line(i, LineType::CRLF), i / 2);

                #[cfg(feature = "metric_lines_lf")]
                assert_eq!(s.byte_to_line(i, LineType::LF), i / 2);
            }

            #[cfg(feature = "metric_lines_cr_lf")]
            assert_eq!(s.byte_to_line(si, LineType::CRLF), (si + 1) / 2);

            #[cfg(feature = "metric_lines_lf")]
            assert_eq!(s.byte_to_line(si, LineType::LF), si / 2);
        }
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn char_to_byte_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(88..124);

        assert_eq!("?  こんにちは、みんなさん", s);

        assert_eq!(0, s.char_to_byte(0));
        assert_eq!(1, s.char_to_byte(1));
        assert_eq!(2, s.char_to_byte(2));

        assert_eq!(3, s.char_to_byte(3));
        assert_eq!(6, s.char_to_byte(4));
        assert_eq!(33, s.char_to_byte(13));
        assert_eq!(36, s.char_to_byte(14));
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    fn line_to_byte_01() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..112);

        assert_eq!(
            "'s a fine day, isn't it?\nAren't you glad \
             we're alive?\nこんにちは、みん",
            s,
        );

        assert_eq!(0, s.line_to_byte(0, LineType::CRLF));
        assert_eq!(25, s.line_to_byte(1, LineType::CRLF));
        assert_eq!(54, s.line_to_byte(2, LineType::CRLF));
        assert_eq!(78, s.line_to_byte(3, LineType::CRLF));
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    fn line_to_byte_02() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(43..43);

        assert_eq!(0, s.line_to_byte(0, LineType::CRLF));
        assert_eq!(0, s.line_to_byte(1, LineType::CRLF));
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[should_panic]
    fn line_to_byte_03() {
        let r = Rope::from_str(TEXT_LINES);
        let s = r.slice(34..96);

        s.line_to_byte(4, LineType::CRLF);
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[should_panic]
    fn line_to_byte_04() {
        let r = Rope::from_str("\n\n\n\n");
        let s = r.slice(1..3);

        s.line_to_byte(4, LineType::CRLF);
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_lf"))]
    #[test]
    fn line_to_byte_05() {
        // Make sure splitting CRLF pairs at the end works properly.
        let r = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk("\r\n\r\n\r\n");
            rb._append_chunk("\r\n\r\n\r");
            rb._append_chunk("\n\r\n\r\n\r");
            rb._append_chunk("\n\r\n\r\n\r\n");
            rb._append_chunk("\r\n\r\n\r\n");
            rb.finish()
        };

        #[cfg(feature = "metric_lines_cr_lf")]
        for si in 0..=r.len_bytes() {
            let s = r.slice(..si);
            for li in 0..(s.len_lines(LineType::CRLF) - 1) {
                assert_eq!(s.line_to_byte(li, LineType::CRLF), li * 2);
            }
            assert_eq!(
                s.line_to_byte(s.len_lines(LineType::CRLF) - 1, LineType::CRLF),
                si,
            );
            assert_eq!(
                s.line_to_byte(s.len_lines(LineType::CRLF), LineType::CRLF),
                si,
            );
        }

        #[cfg(feature = "metric_lines_lf")]
        for si in 0..=r.len_bytes() {
            let s = r.slice(..si);
            for li in 0..(s.len_lines(LineType::LF) - 1) {
                assert_eq!(s.line_to_byte(li, LineType::LF), li * 2);
            }
            assert_eq!(
                s.line_to_byte(s.len_lines(LineType::LF) - 1, LineType::LF),
                si - (si % 2),
            );
            assert_eq!(s.line_to_byte(s.len_lines(LineType::LF), LineType::LF), si);
        }
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_01() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        assert_eq!(0, s.utf16_to_byte(0));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn utf16_to_byte_02() {
        let r = Rope::from_str("");
        let s = r.slice(..);
        s.utf16_to_byte(1);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_03() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        assert_eq!(0, s.utf16_to_byte(0));
        assert_eq!(0, s.utf16_to_byte(1));
        assert_eq!(4, s.utf16_to_byte(2));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn utf16_to_byte_04() {
        let r = Rope::from_str("🐸");
        let s = r.slice(..);
        s.utf16_to_byte(3);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_05() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);

        assert_eq!(0, s.utf16_to_byte(0));

        assert_eq!(12, s.utf16_to_byte(12));
        assert_eq!(16, s.utf16_to_byte(14));

        assert_eq!(35, s.utf16_to_byte(33));
        assert_eq!(39, s.utf16_to_byte(35));

        assert_eq!(67, s.utf16_to_byte(63));
        assert_eq!(71, s.utf16_to_byte(65));

        assert_eq!(101, s.utf16_to_byte(95));
        assert_eq!(105, s.utf16_to_byte(97));

        assert_eq!(107, s.utf16_to_byte(99));
        assert_eq!(110, s.utf16_to_byte(100));

        assert_eq!(140, s.utf16_to_byte(110));
        assert_eq!(143, s.utf16_to_byte(111));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn utf16_to_byte_06() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(..);
        s.utf16_to_byte(112);
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_07() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..137);

        assert_eq!(0, s.utf16_to_byte(0));

        assert_eq!(11, s.utf16_to_byte(11));
        assert_eq!(15, s.utf16_to_byte(13));

        assert_eq!(34, s.utf16_to_byte(32));
        assert_eq!(38, s.utf16_to_byte(34));

        assert_eq!(66, s.utf16_to_byte(62));
        assert_eq!(70, s.utf16_to_byte(64));

        assert_eq!(100, s.utf16_to_byte(94));
        assert_eq!(104, s.utf16_to_byte(96));

        assert_eq!(106, s.utf16_to_byte(98));
        assert_eq!(109, s.utf16_to_byte(99));

        assert_eq!(133, s.utf16_to_byte(107));
        assert_eq!(136, s.utf16_to_byte(108));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    #[should_panic]
    fn utf16_to_byte_08() {
        let r = Rope::from_str(TEXT_EMOJI);
        let s = r.slice(1..137);
        s.utf16_to_byte(109);
    }

    // #[test]
    // fn byte_01() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(34..100);

    //     assert_eq!(s.byte(0), b't');
    //     assert_eq!(s.byte(10), b' ');

    //     // UTF-8 encoding of 'な'.
    //     assert_eq!(s.byte(s.len_bytes() - 3), 0xE3);
    //     assert_eq!(s.byte(s.len_bytes() - 2), 0x81);
    //     assert_eq!(s.byte(s.len_bytes() - 1), 0xAA);
    // }

    // #[test]
    // #[should_panic]
    // fn byte_02() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(34..100);
    //     s.byte(s.len_bytes());
    // }

    // #[test]
    // #[should_panic]
    // fn byte_03() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(42..42);
    //     s.byte(0);
    // }

    // #[test]
    // fn char_01() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(34..100);

    //     // t's \
    //     // a fine day, isn't it?  Aren't you glad \
    //     // we're alive?  こんにちは、みんな

    //     assert_eq!(s.char(0), 't');
    //     assert_eq!(s.char(10), ' ');
    //     assert_eq!(s.char(18), 'n');
    //     assert_eq!(s.char(65), 'な');
    // }

    // #[test]
    // #[should_panic]
    // fn char_02() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(34..100);
    //     s.char(66);
    // }

    // #[test]
    // #[should_panic]
    // fn char_03() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(43..43);
    //     s.char(0);
    // }

    // #[test]
    // fn line_01() {
    //     let r = Rope::from_str(TEXT_LINES);
    //     let s = r.slice(34..96);
    //     // "'s a fine day, isn't it?\nAren't you glad \
    //     //  we're alive?\nこんにちは、みん"

    //     let l0 = s.line(0);
    //     assert_eq!(l0, "'s a fine day, isn't it?\n");
    //     assert_eq!(l0.len_bytes(), 25);
    //     assert_eq!(l0.len_chars(), 25);
    //     assert_eq!(l0.len_lines(), 2);

    //     let l1 = s.line(1);
    //     assert_eq!(l1, "Aren't you glad we're alive?\n");
    //     assert_eq!(l1.len_bytes(), 29);
    //     assert_eq!(l1.len_chars(), 29);
    //     assert_eq!(l1.len_lines(), 2);

    //     let l2 = s.line(2);
    //     assert_eq!(l2, "こんにちは、みん");
    //     assert_eq!(l2.len_bytes(), 24);
    //     assert_eq!(l2.len_chars(), 8);
    //     assert_eq!(l2.len_lines(), 1);
    // }

    // #[test]
    // fn line_02() {
    //     let r = Rope::from_str(TEXT_LINES);
    //     let s = r.slice(34..59);
    //     // "'s a fine day, isn't it?\n"

    //     assert_eq!(s.line(0), "'s a fine day, isn't it?\n");
    //     assert_eq!(s.line(1), "");
    // }

    // #[test]
    // fn line_03() {
    //     let r = Rope::from_str("Hi\nHi\nHi\nHi\nHi\nHi\n");
    //     let s = r.slice(1..17);

    //     assert_eq!(s.line(0), "i\n");
    //     assert_eq!(s.line(1), "Hi\n");
    //     assert_eq!(s.line(2), "Hi\n");
    //     assert_eq!(s.line(3), "Hi\n");
    //     assert_eq!(s.line(4), "Hi\n");
    //     assert_eq!(s.line(5), "Hi");
    // }

    // #[test]
    // fn line_04() {
    //     let r = Rope::from_str(TEXT_LINES);
    //     let s = r.slice(43..43);

    //     assert_eq!(s.line(0), "");
    // }

    // #[test]
    // #[should_panic]
    // fn line_05() {
    //     let r = Rope::from_str(TEXT_LINES);
    //     let s = r.slice(34..96);
    //     s.line(3);
    // }

    // #[test]
    // fn line_06() {
    //     let r = Rope::from_str("1\n2\n3\n4\n5\n6\n7\n8");
    //     let s = r.slice(1..11);
    //     // "\n2\n3\n4\n5\n6"

    //     assert_eq!(s.line(0).len_lines(), 2);
    //     assert_eq!(s.line(1).len_lines(), 2);
    //     assert_eq!(s.line(2).len_lines(), 2);
    //     assert_eq!(s.line(3).len_lines(), 2);
    //     assert_eq!(s.line(4).len_lines(), 2);
    //     assert_eq!(s.line(5).len_lines(), 1);
    // }

    // #[test]
    // fn chunk_at_byte() {
    //     let r = Rope::from_str(TEXT_LINES);
    //     let s = r.slice(34..96);
    //     let text = &TEXT_LINES[34..112];
    //     // "'s a fine day, isn't it?\nAren't you glad \
    //     //  we're alive?\nこんにちは、みん"

    //     let mut t = text;
    //     let mut prev_chunk = "";
    //     for i in 0..s.len_bytes() {
    //         let (chunk, b, c, l) = s.chunk_at_byte(i);
    //         assert_eq!(c, byte_to_char_idx(text, b));
    //         assert_eq!(l, byte_to_line_idx(text, b));
    //         if chunk != prev_chunk {
    //             assert_eq!(chunk, &t[..chunk.len()]);
    //             t = &t[chunk.len()..];
    //             prev_chunk = chunk;
    //         }

    //         let c1 = {
    //             let i2 = byte_to_char_idx(text, i);
    //             text.chars().nth(i2).unwrap()
    //         };
    //         let c2 = {
    //             let i2 = i - b;
    //             let i3 = byte_to_char_idx(chunk, i2);
    //             chunk.chars().nth(i3).unwrap()
    //         };
    //         assert_eq!(c1, c2);
    //     }

    //     assert_eq!(t.len(), 0);
    // }

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
    //         assert_eq!(b, char_to_byte_idx(text, c));
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

    // #[test]
    // fn eq_rope_slice_01() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(43..43);

    //     assert_eq!(s, s);
    // }

    // #[test]
    // fn eq_rope_slice_02() {
    //     let r = Rope::from_str(TEXT);
    //     let s1 = r.slice(43..97);
    //     let s2 = r.slice(43..97);

    //     assert_eq!(s1, s2);
    // }

    // #[test]
    // fn eq_rope_slice_03() {
    //     let r = Rope::from_str(TEXT);
    //     let s1 = r.slice(43..43);
    //     let s2 = r.slice(43..45);

    //     assert_ne!(s1, s2);
    // }

    // #[test]
    // fn eq_rope_slice_04() {
    //     let r = Rope::from_str(TEXT);
    //     let s1 = r.slice(43..45);
    //     let s2 = r.slice(43..43);

    //     assert_ne!(s1, s2);
    // }

    // #[test]
    // fn eq_rope_slice_05() {
    //     let r = Rope::from_str("");
    //     let s = r.slice(0..0);

    //     assert_eq!(s, s);
    // }

    // #[test]
    // fn cmp_rope_slice_01() {
    //     let r1 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
    //     let r2 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
    //     let s1 = r1.slice(..);
    //     let s2 = r2.slice(..);

    //     assert_eq!(s1.cmp(&s2), std::cmp::Ordering::Equal);
    //     assert_eq!(s1.slice(..24).cmp(&s2), std::cmp::Ordering::Less);
    //     assert_eq!(s1.cmp(&s2.slice(..24)), std::cmp::Ordering::Greater);
    // }

    // #[test]
    // fn cmp_rope_slice_02() {
    //     let r1 = Rope::from_str("abcdefghijklmnzpqrstuvwxyz");
    //     let r2 = Rope::from_str("abcdefghijklmnopqrstuvwxyz");
    //     let s1 = r1.slice(..);
    //     let s2 = r2.slice(..);

    //     assert_eq!(s1.cmp(&s2), std::cmp::Ordering::Greater);
    //     assert_eq!(s2.cmp(&s1), std::cmp::Ordering::Less);
    // }

    // #[test]
    // fn to_string_01() {
    //     let r = Rope::from_str(TEXT);
    //     let slc = r.slice(..);
    //     let s: String = slc.into();

    //     assert_eq!(r, s);
    //     assert_eq!(slc, s);
    // }

    // #[test]
    // fn to_string_02() {
    //     let r = Rope::from_str(TEXT);
    //     let slc = r.slice(0..24);
    //     let s: String = slc.into();

    //     assert_eq!(slc, s);
    // }

    // #[test]
    // fn to_string_03() {
    //     let r = Rope::from_str(TEXT);
    //     let slc = r.slice(13..89);
    //     let s: String = slc.into();

    //     assert_eq!(slc, s);
    // }

    // #[test]
    // fn to_string_04() {
    //     let r = Rope::from_str(TEXT);
    //     let slc = r.slice(13..41);
    //     let s: String = slc.into();

    //     assert_eq!(slc, s);
    // }

    // #[test]
    // fn to_cow_01() {
    //     use std::borrow::Cow;
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(13..83);
    //     let cow: Cow<str> = s.into();

    //     assert_eq!(s, cow);
    // }

    // #[test]
    // fn to_cow_02() {
    //     use std::borrow::Cow;
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(13..14);
    //     let cow: Cow<str> = r.slice(13..14).into();

    //     // Make sure it's borrowed.
    //     if let Cow::Owned(_) = cow {
    //         panic!("Small Cow conversions should result in a borrow.");
    //     }

    //     assert_eq!(s, cow);
    // }

    // #[test]
    // fn hash_01() {
    //     let mut h1 = std::collections::hash_map::DefaultHasher::new();
    //     let mut h2 = std::collections::hash_map::DefaultHasher::new();
    //     let r = Rope::from_str("Hello there!");
    //     let s = r.slice(..);

    //     r.hash(&mut h1);
    //     s.hash(&mut h2);

    //     assert_eq!(h1.finish(), h2.finish());
    // }

    // Iterator tests are in the iter module
}
