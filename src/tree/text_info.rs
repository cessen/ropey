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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct TextInfo {
    pub bytes: u64,

    #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
    pub chars: u64,

    #[cfg(feature = "metric_utf16")]
    pub utf16_surrogates: u64,

    #[cfg(feature = "metric_lines_lf")]
    pub line_breaks_lf: u64,

    #[cfg(feature = "metric_lines_cr_lf")]
    pub line_breaks_cr_lf: u64,

    #[cfg(feature = "metric_lines_unicode")]
    pub line_breaks_unicode: u64,

    // To handle split CRLF line breaks correctly.
    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    pub starts_with_lf: bool,
    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    pub ends_with_cr: bool,
}

impl TextInfo {
    #[inline]
    pub fn new() -> TextInfo {
        TextInfo {
            bytes: 0,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: 0,

            #[cfg(feature = "metric_utf16")]
            utf16_surrogates: 0,

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
    pub fn from_str(text: &str) -> TextInfo {
        TextInfo {
            bytes: text.len() as u64,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: chars::count(text) as u64,

            #[cfg(feature = "metric_utf16")]
            utf16_surrogates: utf16::count_surrogates(text) as u64,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: lines_lf::count_breaks(text) as u64,

            #[cfg(feature = "metric_lines_cr_lf")]
            line_breaks_cr_lf: lines_crlf::count_breaks(text) as u64,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: lines::count_breaks(text) as u64,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            starts_with_lf: text.as_bytes().get(0).map(|&b| b == 0x0A).unwrap_or(false),

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            ends_with_cr: text.as_bytes().last().map(|&b| b == 0x0D).unwrap_or(false),
        }
    }

    /// Returns the adjusted version of self based on whatever the next block is.
    #[must_use]
    pub fn adjusted_by_next(
        self,
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        next: TextInfo,
        #[cfg(not(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode")))]
        _next: TextInfo,
    ) -> TextInfo {
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        let crlf_split_compensation = if self.ends_with_cr && next.starts_with_lf {
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

    /// Combines two TextInfos as if they represented abutting pieces of text data.
    ///
    /// This properly accounts for things like split CRLF line breaks, etc.
    #[must_use]
    #[inline(always)]
    pub fn combine(self, rhs: TextInfo) -> TextInfo {
        let tmp = self.adjusted_by_next(rhs);
        tmp + rhs
    }
}

impl Add for TextInfo {
    type Output = Self;
    /// Note: this does *not* account for neccesary compensation for e.g.
    /// CRLF line breaks that are split across boundaries.  It just does
    /// a fairly naive sum of the text info stats.
    ///
    /// See `combine()` for an equivalent function that does account for
    /// such things.
    #[inline]
    fn add(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes + rhs.bytes,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: self.chars + rhs.chars,

            #[cfg(feature = "metric_utf16")]
            utf16_surrogates: self.utf16_surrogates + rhs.utf16_surrogates,

            #[cfg(feature = "metric_lines_lf")]
            line_breaks_lf: self.line_breaks_lf + rhs.line_breaks_lf,

            #[cfg(feature = "metric_lines_cr_lf")]
            line_breaks_cr_lf: self.line_breaks_cr_lf + rhs.line_breaks_cr_lf,

            #[cfg(feature = "metric_lines_unicode")]
            line_breaks_unicode: self.line_breaks_unicode + rhs.line_breaks_unicode,

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            starts_with_lf: if self.bytes > 0 {
                self.starts_with_lf
            } else {
                rhs.starts_with_lf
            },
            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            ends_with_cr: if rhs.bytes > 0 {
                rhs.ends_with_cr
            } else {
                self.ends_with_cr
            },
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
    /// Note: this does *not* account for neccesary compensation for e.g.
    /// CRLF line breaks that are split across boundaries.  It just does
    /// a fairly naive subtraction of the text info stats.
    ///
    /// Because of that, using this correctly requires knowledge of the
    /// specific chunks of text that the text info represents, and doing
    /// some special handling based on that.
    #[inline]
    fn sub(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes - rhs.bytes,

            #[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
            chars: self.chars - rhs.chars,

            #[cfg(feature = "metric_utf16")]
            utf16_surrogates: self.utf16_surrogates - rhs.utf16_surrogates,

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
