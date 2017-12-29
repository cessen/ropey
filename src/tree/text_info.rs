use std::ops::{Add, AddAssign, Sub, SubAssign};

use str_utils::{count_chars, count_line_breaks};
use tree::Count;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TextInfo {
    pub(crate) bytes: Count,
    pub(crate) chars: Count,
    pub(crate) line_breaks: Count,
}

impl TextInfo {
    pub fn new() -> TextInfo {
        TextInfo {
            bytes: 0,
            chars: 0,
            line_breaks: 0,
        }
    }

    pub fn from_str(text: &str) -> TextInfo {
        TextInfo {
            bytes: text.len() as Count,
            chars: count_chars(text) as Count,
            line_breaks: count_line_breaks(text) as Count,
        }
    }
}

impl Add for TextInfo {
    type Output = Self;
    fn add(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes + rhs.bytes,
            chars: self.chars + rhs.chars,
            line_breaks: self.line_breaks + rhs.line_breaks,
        }
    }
}

impl AddAssign for TextInfo {
    fn add_assign(&mut self, other: TextInfo) {
        *self = *self + other;
    }
}

impl Sub for TextInfo {
    type Output = Self;
    fn sub(self, rhs: TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes - rhs.bytes,
            chars: self.chars - rhs.chars,
            line_breaks: self.line_breaks - rhs.line_breaks,
        }
    }
}

impl SubAssign for TextInfo {
    fn sub_assign(&mut self, other: TextInfo) {
        *self = *self - other;
    }
}
