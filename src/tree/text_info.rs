use std::ops::{Add, AddAssign, Sub, SubAssign};

#[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
use str_indices::chars;

#[cfg(feature = "metric_utf16")]
use str_indices::utf16;

#[cfg(feature = "metric_lines_lf")]
use str_indices::lines_lf;

#[cfg(feature = "metric_lines_lf_cr")]
use str_indices::lines_crlf;

#[cfg(feature = "metric_lines_unicode")]
use str_indices::lines;

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
use crate::LineType;

use crate::str_utils::{ends_with_cr, starts_with_lf};

#[cfg(any(feature = "metric_lines_lf_cr", feature = "metric_lines_unicode"))]
use crate::str_utils::{byte_is_cr, byte_is_lf};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct TextInfo {
    pub bytes: usize,

    #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
    pub chars: usize,

    #[cfg(feature = "metric_utf16")]
    pub utf16: usize,

    #[cfg(feature = "metric_lines_lf")]
    pub line_breaks_lf: usize,

    #[cfg(feature = "metric_lines_lf_cr")]
    pub line_breaks_cr_lf: usize,

    #[cfg(feature = "metric_lines_unicode")]
    pub line_breaks_unicode: usize,
}

impl TextInfo {
    /// Creates a new empty `TextInfo`.
    ///
    /// The returned `TextInfo` is identical to what `TextInfo::from_str("")`
    /// would return, but is constructed more efficiently since this can skip
    /// all of the text scan function calls.
    pub(crate) fn new() -> TextInfo {
        TextInfo {
            bytes: 0,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: 0,

            #[cfg(feature = "metric_utf16")]
            utf16: 0,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: 0,

            #[cfg(feature = "metric_lines_lf_cr")]
            line_breaks_cr_lf: 0,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: 0,
        }
    }

    pub(crate) fn from_str(text: &str) -> TextInfo {
        #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
        let char_count = chars::count(text);

        TextInfo {
            bytes: text.len(),

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: char_count,

            #[cfg(feature = "metric_utf16")]
            utf16: utf16::count_surrogates(text) + char_count,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: lines_lf::count_breaks(text),

            #[cfg(feature = "metric_lines_lf_cr")]
            line_breaks_cr_lf: lines_crlf::count_breaks(text),

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: lines::count_breaks(text),
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[inline(always)]
    pub(crate) fn line_breaks(&self, line_type: LineType) -> usize {
        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => self.line_breaks_lf,

            #[cfg(feature = "metric_lines_lf_cr")]
            LineType::LF_CR => self.line_breaks_cr_lf,

            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => self.line_breaks_unicode,
        }
    }

    /// Updates info for a leaf node that has text inserted into it.  This
    /// should be called with the pre-insertion `text`.
    #[must_use]
    pub(crate) fn str_insert(
        self,
        text: &str,
        byte_idx: usize,
        insertion_info: TextInfo,
        ins_text: &str,
    ) -> TextInfo {
        // To silence unused parameter warnings when the relevant features are
        // disabled.
        let _ = (text, byte_idx, insertion_info, ins_text);

        // This function only works correctly when the inserted text is non-zero
        // length.
        debug_assert!(insertion_info.bytes > 0);

        // Silence unused mut warnings when the relevant features are disabled.
        #[allow(unused_mut)]
        let mut new_info = self + insertion_info;

        #[cfg(any(feature = "metric_lines_lf_cr", feature = "metric_lines_unicode"))]
        {
            let seam_cr = byte_idx > 0 && byte_is_cr(text, byte_idx - 1);
            let seam_lf = byte_is_lf(text, byte_idx);

            let crlf_split_compensation = (seam_cr && seam_lf) as usize;
            let crlf_merge_compensation_1 = (seam_cr && starts_with_lf(ins_text)) as usize;
            let crlf_merge_compensation_2 = (ends_with_cr(ins_text) && seam_lf) as usize;

            #[cfg(feature = "metric_lines_lf_cr")]
            {
                new_info.line_breaks_cr_lf += crlf_split_compensation;
                new_info.line_breaks_cr_lf -= crlf_merge_compensation_1;
                new_info.line_breaks_cr_lf -= crlf_merge_compensation_2;
            }
            #[cfg(feature = "metric_lines_unicode")]
            {
                new_info.line_breaks_unicode += crlf_split_compensation;
                new_info.line_breaks_unicode -= crlf_merge_compensation_1;
                new_info.line_breaks_unicode -= crlf_merge_compensation_2;
            }
        }

        new_info
    }

    /// Updates info for a leaf node that has text removed from it.  This
    /// should be called with the pre-removal `text`.
    #[must_use]
    pub(crate) fn str_remove(self, text: &str, byte_idx_range: [usize; 2]) -> TextInfo {
        // For terseness.
        let [start, end] = byte_idx_range;

        if start == end {
            return self;
        }
        if start == 0 && end == text.len() {
            return TextInfo::new();
        }

        // Easier case: the removal range is larger than the text that will
        // remain, so we just scan the text that will be left over.
        if (end - start) >= (text.len() / 2) {
            let left = &text[..start];
            let right = &text[end..];

            let left_info = TextInfo::from_str(&text[..start]);
            let right_info = TextInfo::from_str(&text[end..]);

            #[allow(unused_mut)] // `mut` only needed with some features.
            let mut new_info = left_info + right_info;

            if ends_with_cr(left) && starts_with_lf(right) {
                #[cfg(feature = "metric_lines_lf_cr")]
                {
                    new_info.line_breaks_cr_lf -= 1
                }
                #[cfg(feature = "metric_lines_unicode")]
                {
                    new_info.line_breaks_unicode -= 1;
                }
            }

            return new_info;
        }

        #[allow(unused_mut)] // `mut` only needed with some features.
        let mut new_info = self - TextInfo::from_str(&text[byte_idx_range[0]..byte_idx_range[1]]);

        #[cfg(any(feature = "metric_lines_lf_cr", feature = "metric_lines_unicode"))]
        {
            let start_lf = byte_is_lf(text, start);
            let start_cr = start > 0 && byte_is_cr(text, start - 1);
            let end_lf = end < text.len() && byte_is_lf(text, end);
            let end_cr = byte_is_cr(text, end - 1);

            let crlf_split_compensation_1 = (start_cr && start_lf) as usize;
            let crlf_split_compensation_2 = (end_cr && end_lf) as usize;
            let crlf_merge_compensation = (start_cr && end_lf) as usize;

            #[cfg(feature = "metric_lines_lf_cr")]
            {
                new_info.line_breaks_cr_lf += crlf_split_compensation_1;
                new_info.line_breaks_cr_lf += crlf_split_compensation_2;
                new_info.line_breaks_cr_lf -= crlf_merge_compensation;
            }
            #[cfg(feature = "metric_lines_unicode")]
            {
                new_info.line_breaks_unicode += crlf_split_compensation_1;
                new_info.line_breaks_unicode += crlf_split_compensation_2;
                new_info.line_breaks_unicode -= crlf_merge_compensation;
            }
        }

        new_info
    }
}

impl Add for TextInfo {
    type Output = Self;
    // Note: this does *not* handle anything related to CRLF line breaks.  It
    // just does a simple sum of the metrics.
    //
    // Because of that, using this correctly typically requires special
    // handling.  Beware.
    //
    // If you want to combine two TextInfo's as if their text were
    // concatenated, see `concat()`.
    #[inline(always)]
    fn add(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes + rhs.bytes,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: self.chars + rhs.chars,

            #[cfg(feature = "metric_utf16")]
            utf16: self.utf16 + rhs.utf16,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: self.line_breaks_lf + rhs.line_breaks_lf,

            #[cfg(feature = "metric_lines_lf_cr")]
            line_breaks_cr_lf: self.line_breaks_cr_lf + rhs.line_breaks_cr_lf,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: self.line_breaks_unicode + rhs.line_breaks_unicode,

            ..self
        }
    }
}

impl AddAssign for TextInfo {
    #[inline(always)]
    fn add_assign(&mut self, other: TextInfo) {
        *self = *self + other;
    }
}

impl Sub for TextInfo {
    type Output = Self;
    // IMPORTANT: this does *not* handle anything related to CRLF line breaks.
    // It just does a simple subtraction of the metrics.
    //
    // Because of that, using this correctly typically requires special
    // handling.  Beware.
    #[inline(always)]
    fn sub(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes - rhs.bytes,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: self.chars - rhs.chars,

            #[cfg(feature = "metric_utf16")]
            utf16: self.utf16 - rhs.utf16,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: self.line_breaks_lf - rhs.line_breaks_lf,

            #[cfg(feature = "metric_lines_lf_cr")]
            line_breaks_cr_lf: self.line_breaks_cr_lf - rhs.line_breaks_cr_lf,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: self.line_breaks_unicode - rhs.line_breaks_unicode,

            ..self
        }
    }
}

impl SubAssign for TextInfo {
    #[inline(always)]
    fn sub_assign(&mut self, other: TextInfo) {
        *self = *self - other;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_01() {
        assert_eq!(TextInfo::new(), TextInfo::from_str(""));
    }

    #[test]
    fn from_str_bytes_01() {
        assert_eq!(0, TextInfo::from_str("").bytes);
        assert_eq!(6, TextInfo::from_str("Hello!").bytes);
        assert_eq!(18, TextInfo::from_str("こんにちは！").bytes);
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn from_str_chars_01() {
        assert_eq!(0, TextInfo::from_str("").chars);
        assert_eq!(6, TextInfo::from_str("Hello!").chars);
        assert_eq!(6, TextInfo::from_str("こんにちは！").chars);
    }

    #[cfg(feature = "metric_lines_lf")]
    #[test]
    fn from_str_line_breaks_lf_01() {
        assert_eq!(0, TextInfo::from_str("").line_breaks_lf);
        assert_eq!(0, TextInfo::from_str("\u{0085}").line_breaks_lf);
        assert_eq!(0, TextInfo::from_str("\r").line_breaks_lf);
        assert_eq!(1, TextInfo::from_str("\n").line_breaks_lf);
        assert_eq!(1, TextInfo::from_str("\r\n").line_breaks_lf);
        assert_eq!(1, TextInfo::from_str("Hello\n world!").line_breaks_lf);
        assert_eq!(3, TextInfo::from_str("\nこんにち\nは！\n").line_breaks_lf);
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn from_str_line_breaks_cr_lf_01() {
        assert_eq!(0, TextInfo::from_str("").line_breaks_cr_lf);
        assert_eq!(0, TextInfo::from_str("\u{0085}").line_breaks_cr_lf);
        assert_eq!(1, TextInfo::from_str("\r").line_breaks_cr_lf);
        assert_eq!(1, TextInfo::from_str("\n").line_breaks_cr_lf);
        assert_eq!(1, TextInfo::from_str("\r\n").line_breaks_cr_lf);
        assert_eq!(1, TextInfo::from_str("Hello\n world!").line_breaks_cr_lf);
        assert_eq!(
            4,
            TextInfo::from_str("\nこん\rにち\nは！\r\n").line_breaks_cr_lf
        );
    }

    #[cfg(feature = "metric_lines_unicode")]
    #[test]
    fn from_str_line_breaks_unicode_01() {
        assert_eq!(0, TextInfo::from_str("").line_breaks_unicode);
        assert_eq!(1, TextInfo::from_str("\u{0085}").line_breaks_unicode);
        assert_eq!(1, TextInfo::from_str("\r").line_breaks_unicode);
        assert_eq!(1, TextInfo::from_str("\n").line_breaks_unicode);
        assert_eq!(1, TextInfo::from_str("\r\n").line_breaks_unicode);
        assert_eq!(1, TextInfo::from_str("Hello\n world!").line_breaks_unicode);
        assert_eq!(
            4,
            TextInfo::from_str("\nこん\rにち\nは！\r\n").line_breaks_unicode
        );
    }
}
