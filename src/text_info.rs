use str_utils::LineBreakIter;

pub(crate) type Count = u32;

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) struct TextInfo {
    pub(crate) bytes: Count,
    pub(crate) chars: Count,
    pub(crate) line_breaks: Count,
}

impl TextInfo {
    pub(crate) fn new() -> TextInfo {
        TextInfo {
            bytes: 0,
            chars: 0,
            line_breaks: 0,
        }
    }

    pub(crate) fn from_str(text: &str) -> TextInfo {
        TextInfo {
            bytes: text.len() as Count,
            chars: text.chars().count() as Count,
            line_breaks: LineBreakIter::new(text).count() as Count,
        }
    }

    pub(crate) fn combine(&self, other: &TextInfo) -> TextInfo {
        TextInfo {
            bytes: self.bytes + other.bytes,
            chars: self.chars + other.chars,
            line_breaks: self.line_breaks + other.line_breaks,
        }
    }
}
