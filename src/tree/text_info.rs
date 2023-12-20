use std::ops::{Add, AddAssign};
use str_indices::{chars, lines, lines_crlf, lines_lf, utf16};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct TextInfo {
    pub bytes: u64,
    pub chars: u64,
    pub utf16_surrogates: u64,
    pub line_breaks_lf: u64,
    pub line_breaks_crlf: u64,
    pub line_breaks_unicode: u64,

    // To handle split CRLF line breaks correctly.
    pub starts_with_lf: bool,
    pub ends_with_cr: bool,
}

impl TextInfo {
    #[inline]
    pub fn new() -> TextInfo {
        TextInfo {
            bytes: 0,
            chars: 0,
            utf16_surrogates: 0,
            line_breaks_lf: 0,
            line_breaks_crlf: 0,
            line_breaks_unicode: 0,
            starts_with_lf: false,
            ends_with_cr: false,
        }
    }

    #[inline]
    pub fn from_str(text: &str) -> TextInfo {
        TextInfo {
            bytes: text.len() as u64,
            chars: chars::count(text) as u64,
            utf16_surrogates: utf16::count_surrogates(text) as u64,
            line_breaks_lf: lines_lf::count_breaks(text) as u64,
            line_breaks_crlf: lines_crlf::count_breaks(text) as u64,
            line_breaks_unicode: lines::count_breaks(text) as u64,
            starts_with_lf: text.as_bytes().get(0).map(|&b| b == 0x0A).unwrap_or(false),
            ends_with_cr: text.as_bytes().last().map(|&b| b == 0x0D).unwrap_or(false),
        }
    }

    /// Returns the adjusted version of self based on whatever the next block is.
    #[must_use]
    pub fn adjusted_by_next(self, next: TextInfo) -> TextInfo {
        let crlf_split_compensation = if self.ends_with_cr && next.starts_with_lf {
            1
        } else {
            0
        };
        TextInfo {
            line_breaks_crlf: self.line_breaks_crlf - crlf_split_compensation,
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
            chars: self.chars + rhs.chars,
            utf16_surrogates: self.utf16_surrogates + rhs.utf16_surrogates,
            line_breaks_lf: self.line_breaks_lf + rhs.line_breaks_lf,
            line_breaks_crlf: self.line_breaks_crlf + rhs.line_breaks_crlf,
            line_breaks_unicode: self.line_breaks_unicode + rhs.line_breaks_unicode,
            starts_with_lf: if self.bytes > 0 {
                self.starts_with_lf
            } else {
                rhs.starts_with_lf
            },
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
