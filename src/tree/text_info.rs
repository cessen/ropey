use std::ops::{Add, AddAssign, Sub, SubAssign};

use crate::str_utils::{count_chars, count_line_breaks, count_utf16_surrogates};
use crate::tree::Count;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TextInfo {
    pub(crate) bytes: Count,
    pub(crate) chars: Count,
    pub(crate) utf16_surrogates: Count,
    pub(crate) line_breaks: Count,
}

impl TextInfo {
    #[inline]
    pub fn new() -> TextInfo {
        TextInfo {
            bytes: 0,
            chars: 0,
            utf16_surrogates: 0,
            line_breaks: 0,
        }
    }

    #[inline]
    pub fn from_str(text: &str) -> TextInfo {
        TextInfo {
            bytes: text.len() as Count,
            chars: count_chars(text) as Count,
            utf16_surrogates: count_utf16_surrogates(text) as Count,
            line_breaks: count_line_breaks(text) as Count,
        }
    }
}

impl Add for TextInfo {
    type Output = Self;
    #[inline]
    fn add(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes + rhs.bytes,
            chars: self.chars + rhs.chars,
            utf16_surrogates: self.utf16_surrogates + rhs.utf16_surrogates,
            line_breaks: self.line_breaks + rhs.line_breaks,
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
    #[inline]
    fn sub(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes - rhs.bytes,
            chars: self.chars - rhs.chars,
            utf16_surrogates: self.utf16_surrogates - rhs.utf16_surrogates,
            line_breaks: self.line_breaks - rhs.line_breaks,
        }
    }
}

impl SubAssign for TextInfo {
    #[inline]
    fn sub_assign(&mut self, other: TextInfo) {
        *self = *self - other;
    }
}
