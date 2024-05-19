use std::ops::{Add, AddAssign, Sub, SubAssign};

#[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
use str_indices::chars;

#[cfg(feature = "metric_utf16")]
use str_indices::utf16;

#[cfg(feature = "metric_lines_cr_lf")]
use str_indices::lines_crlf;

#[cfg(feature = "metric_lines_lf")]
use str_indices::lines_lf;

#[cfg(feature = "metric_lines_unicode")]
use str_indices::lines;

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
use crate::LineType;

use crate::str_utils::{byte_is_cr, byte_is_lf, ends_with_cr, starts_with_lf};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct TextInfo {
    pub bytes: usize,

    #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
    pub chars: usize,

    #[cfg(feature = "metric_utf16")]
    pub utf16: usize,

    #[cfg(feature = "metric_lines_lf")]
    pub line_breaks_lf: usize,

    #[cfg(feature = "metric_lines_cr_lf")]
    pub line_breaks_cr_lf: usize,

    #[cfg(feature = "metric_lines_unicode")]
    pub line_breaks_unicode: usize,

    //---------------------------------------------------------
    // Split CRLF handling.

    // Marks whether the text starts with an LF line break.
    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    starts_with_lf: bool,

    // Marks whether the text ends with a CR line break.
    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    ends_with_cr: bool,

    // Whether split crlf line breaks have already been accounted for in the
    // line break counts.
    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    split_crlf_compensation_done: bool,
}

impl TextInfo {
    /// Creates a new empty `TextInfo`.
    ///
    /// The returned `TextInfo` is identical to what `TextInfo::from_str("")`
    /// would return, but is constructed more efficiently since this can skip
    /// all of the text scan function calls.
    #[inline(always)]
    pub(crate) fn new() -> TextInfo {
        TextInfo {
            bytes: 0,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: 0,

            #[cfg(feature = "metric_utf16")]
            utf16: 0,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: 0,

            #[cfg(feature = "metric_lines_cr_lf")]
            line_breaks_cr_lf: 0,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: 0,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            starts_with_lf: false,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            ends_with_cr: false,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            split_crlf_compensation_done: false,
        }
    }

    /// Same as `new()` but sets up the CRLF flags to be "already adjusted".
    ///
    /// The correct uses for this are very narrow.  Strongly prefer `new()`
    /// unless you really know what you're doing.
    #[inline(always)]
    pub(crate) fn new_adjusted() -> TextInfo {
        TextInfo {
            bytes: 0,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: 0,

            #[cfg(feature = "metric_utf16")]
            utf16: 0,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: 0,

            #[cfg(feature = "metric_lines_cr_lf")]
            line_breaks_cr_lf: 0,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: 0,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            starts_with_lf: false,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            ends_with_cr: false,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            split_crlf_compensation_done: true,
        }
    }

    #[inline]
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

            #[cfg(feature = "metric_lines_cr_lf")]
            line_breaks_cr_lf: lines_crlf::count_breaks(text),

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: lines::count_breaks(text),

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            starts_with_lf: starts_with_lf(text),

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            ends_with_cr: ends_with_cr(text),

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            split_crlf_compensation_done: false,
        }
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    pub(crate) fn is_split_crlf_compensation_applied(&self) -> bool {
        self.split_crlf_compensation_done
    }

    /// NOTE: this intentionally doesn't account for whether the CR is active in
    /// the line counts or not.
    pub(crate) fn ends_with_split_crlf(&self, next_is_lf: bool) -> bool {
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        return self.ends_with_cr && next_is_lf;

        #[cfg(not(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode")))]
        false
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[inline(always)]
    pub(crate) fn line_breaks(&self, line_type: LineType) -> usize {
        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => self.line_breaks_lf,

            #[cfg(feature = "metric_lines_cr_lf")]
            LineType::CRLF => self.line_breaks_cr_lf,

            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => self.line_breaks_unicode,
        }
    }

    /// Returns the line-break-count-adjusted version of self based on whatever
    /// the next block is.
    ///
    /// Note: this does *not* update the starts/ends_with CRLF tags.  They are
    /// left alone.
    #[must_use]
    #[inline(always)]
    pub(crate) fn adjusted_by_next(
        self,
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        next: TextInfo,
        #[cfg(not(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode")))]
        _next: TextInfo,
    ) -> TextInfo {
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        {
            self.adjusted_by_next_is_lf(next.starts_with_lf)
        }

        #[cfg(not(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode")))]
        self
    }

    /// Returns the line-break-count-adjusted version of self based on whether
    /// the next character after this block is LF or not.
    ///
    /// Note: this does *not* update the starts/ends_with CRLF tags.  They are
    /// left alone.
    #[must_use]
    #[inline(always)]
    pub(crate) fn adjusted_by_next_is_lf(
        self,
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        next_is_lf: bool,
        #[cfg(not(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode")))]
        _next_is_lf: bool,
    ) -> TextInfo {
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        {
            if self.split_crlf_compensation_done {
                return self;
            }

            let crlf_split_compensation = if self.ends_with_cr && next_is_lf {
                1
            } else {
                0
            };

            TextInfo {
                #[cfg(feature = "metric_lines_cr_lf")]
                line_breaks_cr_lf: self.line_breaks_cr_lf - crlf_split_compensation,

                #[cfg(feature = "metric_lines_unicode")]
                line_breaks_unicode: self.line_breaks_unicode - crlf_split_compensation,

                split_crlf_compensation_done: true,

                ..self
            }
        }

        #[cfg(not(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode")))]
        self
    }

    /// Combines two TextInfos as if their texts were concatenated.
    ///
    /// This properly accounts for split CRLF line breaks, and computes
    /// the starts/ends_with CRLF tags appropriately.
    #[must_use]
    #[inline(always)]
    pub(crate) fn concat(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            starts_with_lf: if self.bytes == 0 {
                rhs.starts_with_lf
            } else {
                self.starts_with_lf
            },

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            ends_with_cr: if rhs.bytes == 0 {
                self.ends_with_cr
            } else {
                rhs.ends_with_cr
            },

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            split_crlf_compensation_done: if rhs.bytes == 0 {
                self.split_crlf_compensation_done
            } else {
                rhs.split_crlf_compensation_done
            },

            ..(self.adjusted_by_next(rhs) + rhs)
        }
    }

    #[must_use]
    #[inline(always)]
    pub(crate) fn str_insert(
        self,
        text: &str,
        byte_idx: usize,
        insertion_info: TextInfo,
    ) -> TextInfo {
        // This function only works correctly when these preconditions are met.
        // It will give errorneous results otherwise.
        debug_assert!(insertion_info.bytes > 0);
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        debug_assert!(!self.split_crlf_compensation_done);
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        debug_assert!(!insertion_info.split_crlf_compensation_done);

        let mut new_info = self + insertion_info;

        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        {
            let seam_cr = byte_idx > 0 && byte_is_cr(text, byte_idx - 1);
            let seam_lf = byte_is_lf(text, byte_idx);

            let crlf_split_compensation = (seam_cr && seam_lf) as usize;
            let crlf_merge_compensation_1 = (seam_cr && insertion_info.starts_with_lf) as usize;
            let crlf_merge_compensation_2 = (insertion_info.ends_with_cr && seam_lf) as usize;

            #[cfg(feature = "metric_lines_cr_lf")]
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

            if byte_idx == 0 {
                new_info.starts_with_lf = insertion_info.starts_with_lf;
            }
            if byte_idx == text.len() {
                new_info.ends_with_cr = insertion_info.ends_with_cr;
            }
        }

        new_info
    }
}

impl Add for TextInfo {
    type Output = Self;
    // Note: this does *not* handle anything related to CRLF line breaks, such
    // as split CRLF compensation or updating starts/ends_with flags.  It just
    // does a simple sum of the metrics.
    //
    // Because of that, using this correctly typically requires special
    // handling.  Beware.
    //
    // If you want to combine two TextInfo's as if their text were
    // concatenated, see `concat()`.
    #[inline]
    fn add(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes + rhs.bytes,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: self.chars + rhs.chars,

            #[cfg(feature = "metric_utf16")]
            utf16: self.utf16 + rhs.utf16,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: self.line_breaks_lf + rhs.line_breaks_lf,

            #[cfg(feature = "metric_lines_cr_lf")]
            line_breaks_cr_lf: self.line_breaks_cr_lf + rhs.line_breaks_cr_lf,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: self.line_breaks_unicode + rhs.line_breaks_unicode,

            ..self
        }
    }
}

impl AddAssign for TextInfo {
    #[inline]
    fn add_assign(&mut self, other: TextInfo) {
        *self = *self + other;
    }
}

impl Sub for TextInfo {
    type Output = Self;
    // IMPORTANT: this does *not* handle anything related to CRLF line breaks, such
    // as split CRLF compensation or updating starts/ends_with flags.  It just
    // does a simple subtraction of the metrics.
    //
    // Because of that, using this correctly typically requires special
    // handling.  Beware.
    #[inline]
    fn sub(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes - rhs.bytes,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: self.chars - rhs.chars,

            #[cfg(feature = "metric_utf16")]
            utf16: self.utf16 - rhs.utf16,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: self.line_breaks_lf - rhs.line_breaks_lf,

            #[cfg(feature = "metric_lines_cr_lf")]
            line_breaks_cr_lf: self.line_breaks_cr_lf - rhs.line_breaks_cr_lf,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: self.line_breaks_unicode - rhs.line_breaks_unicode,

            ..self
        }
    }
}

impl SubAssign for TextInfo {
    #[inline]
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

    #[cfg(feature = "metric_lines_cr_lf")]
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

    #[test]
    fn concat_01() {
        let test_texts = [
            "Hello world!",
            "\nHello\nworld!\n",
            "\r\nHello\r\nworld!\r\n",
            "\r\n\r\n\r\n\r\n\r\n\r\n",
        ];

        for text in test_texts {
            for split in 0..(text.len() + 1) {
                let left = &text[..split];
                let right = &text[split..];
                assert_eq!(
                    TextInfo::from_str(text),
                    TextInfo::from_str(left).concat(TextInfo::from_str(right)),
                );
            }
        }
    }
}
