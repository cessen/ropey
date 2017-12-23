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

    pub fn combine(&self, other: &TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes + other.bytes,
            chars: self.chars + other.chars,
            line_breaks: self.line_breaks + other.line_breaks,
        }
    }
}
