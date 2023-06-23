/// Zero-based line and column
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineColumn {
    /// Zero-based line number
    pub line: usize,
    /// Zero-based column number
    pub column: usize,
}

impl From<(usize, usize)> for LineColumn {
    fn from((line, column): (usize, usize)) -> Self {
        Self { line, column }
    }
}
