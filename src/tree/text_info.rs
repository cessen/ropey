#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct TextInfo {
    pub bytes: u64,
    pub chars: u64,
    pub utf16_surrogates: u64,
    pub line_breaks: u64,

    // To handle split CRLF line breaks correctly.
    pub starts_with_lf: bool,
    pub ends_with_cr: bool,
}
