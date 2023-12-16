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
}

impl Add for TextInfo {
    type Output = Self;
    #[inline]
    fn add(self, rhs: TextInfo) -> TextInfo {
        let crlf_split_compensation = if self.ends_with_cr && rhs.starts_with_lf {
            1
        } else {
            0
        };
        TextInfo {
            bytes: self.bytes + rhs.bytes,
            chars: self.chars + rhs.chars,
            utf16_surrogates: self.utf16_surrogates + rhs.utf16_surrogates,
            line_breaks_lf: self.line_breaks_lf + rhs.line_breaks_lf,
            line_breaks_crlf: self.line_breaks_crlf + rhs.line_breaks_crlf
                - crlf_split_compensation,
            line_breaks_unicode: self.line_breaks_unicode + rhs.line_breaks_unicode
                - crlf_split_compensation,
            starts_with_lf: self.starts_with_lf,
            ends_with_cr: rhs.ends_with_cr,
        }
    }
}

impl AddAssign for TextInfo {
    #[inline]
    fn add_assign(&mut self, other: TextInfo) {
        *self = *self + other;
    }
}
