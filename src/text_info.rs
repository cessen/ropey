use small_string_utils::LineBreakIter;

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

pub(crate) trait TextInfoArray {
    fn combine(&self) -> TextInfo;
    fn search_combine<F: Fn(&TextInfo) -> bool>(&self, pred: F) -> (usize, TextInfo);
}

impl TextInfoArray for [TextInfo] {
    fn combine(&self) -> TextInfo {
        self.iter().fold(TextInfo::new(), |a, b| a.combine(b))
    }

    fn search_combine<F: Fn(&TextInfo) -> bool>(&self, pred: F) -> (usize, TextInfo) {
        let mut accum = TextInfo::new();
        for (idx, inf) in self.iter().enumerate() {
            if pred(&accum.combine(inf)) {
                return (idx, accum);
            } else {
                accum = accum.combine(inf);
            }
        }
        panic!("Predicate is mal-formed and never evaluated true.")
    }
}
