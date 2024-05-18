#[inline(always)]
pub(crate) fn starts_with_lf(text: &str) -> bool {
    text.as_bytes().get(0).map(|&b| b == 0x0A).unwrap_or(false)
}

#[inline(always)]
pub(crate) fn ends_with_cr(text: &str) -> bool {
    text.as_bytes().last().map(|&b| b == 0x0D).unwrap_or(false)
}

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
pub(crate) mod lines {
    use crate::LineType;

    #[inline(always)]
    pub(crate) fn from_byte_idx(text: &str, byte_idx: usize, line_type: LineType) -> usize {
        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => str_indices::lines_lf::from_byte_idx(text, byte_idx),
            #[cfg(feature = "metric_lines_cr_lf")]
            LineType::CRLF => str_indices::lines_crlf::from_byte_idx(text, byte_idx),
            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => str_indices::lines::from_byte_idx(text, byte_idx),
        }
    }

    #[inline(always)]
    pub(crate) fn to_byte_idx(text: &str, byte_idx: usize, line_type: LineType) -> usize {
        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => str_indices::lines_lf::to_byte_idx(text, byte_idx),
            #[cfg(feature = "metric_lines_cr_lf")]
            LineType::CRLF => str_indices::lines_crlf::to_byte_idx(text, byte_idx),
            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => str_indices::lines::to_byte_idx(text, byte_idx),
        }
    }

    #[allow(unused)]
    #[inline(always)]
    pub(crate) fn count_breaks(text: &str, line_type: LineType) -> usize {
        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => str_indices::lines_lf::count_breaks(text),
            #[cfg(feature = "metric_lines_cr_lf")]
            LineType::CRLF => str_indices::lines_crlf::count_breaks(text),
            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => str_indices::lines::count_breaks(text),
        }
    }
}

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_lf_01() {
        assert_eq!(false, starts_with_lf(""));
        assert_eq!(false, starts_with_lf("Hello!"));
        assert_eq!(true, starts_with_lf("\n"));
        assert_eq!(true, starts_with_lf("\nHello!"));
    }

    #[test]
    fn ends_with_cr_01() {
        assert_eq!(false, ends_with_cr(""));
        assert_eq!(false, ends_with_cr("Hello!"));
        assert_eq!(true, ends_with_cr("\r"));
        assert_eq!(true, ends_with_cr("Hello!\r"));
    }
}
