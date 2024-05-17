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

use crate::str_utils::{ends_with_cr, starts_with_lf};

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

    // To handle split CRLF line breaks correctly.
    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    pub(crate) starts_with_lf: bool,
    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    pub(crate) ends_with_cr: bool,
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
        }
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
            starts_with_lf: (self.bytes == 0 && rhs.starts_with_lf) || self.starts_with_lf,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            ends_with_cr: (rhs.bytes == 0 && self.ends_with_cr) || rhs.ends_with_cr,

            ..(self.adjusted_by_next(rhs) + rhs)
        }
    }

    /// Computes the new info for a text after removing some of it from
    /// the right side.
    ///
    /// The info this is called on is the pre-truncation text info.
    ///
    /// - `remaining_text`: the remaining text after truncation.
    /// - `removed_info`: the text info for the portion of the text was
    ///   removed from the right side.
    #[must_use]
    #[inline(always)]
    pub(crate) fn truncate(self, remaining_text: &str, removed_info: TextInfo) -> TextInfo {
        if remaining_text.is_empty() {
            return TextInfo::new();
        }

        let mut info = self - removed_info;

        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        {
            info.starts_with_lf = starts_with_lf(remaining_text);
            info.ends_with_cr = ends_with_cr(remaining_text);
            if info.ends_with_cr && removed_info.starts_with_lf {
                #[cfg(feature = "metric_lines_cr_lf")]
                {
                    info.line_breaks_cr_lf += 1;
                }

                #[cfg(feature = "metric_lines_unicode")]
                {
                    info.line_breaks_unicode += 1;
                }
            }
        }

        info
    }
}

impl Add for TextInfo {
    type Output = Self;
    // Note: this does *not* handle anything related to CRLF line breaks, such
    // as split CRLF compensation or updating starts/ends_with flags.  It just
    // does a straight sum of the text info stats.
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
    // Note: this does *not* handle anything related to CRLF line breaks, such
    // as split CRLF compensation or updating starts/ends_with flags.  It just
    // does a straight subtraction of the text info stats.
    //
    // Because of that, using this correctly typically requires special
    // handling.  Beware.
    //
    // If you want to remove one TextInfo from another as if truncating, see
    // `truncate()`.
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

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            starts_with_lf: false,
            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            ends_with_cr: false,
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

    #[test]
    fn truncate_01() {
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
                    TextInfo::from_str(left),
                    TextInfo::from_str(text).truncate(left, TextInfo::from_str(right),),
                );
            }
        }
    }
}
