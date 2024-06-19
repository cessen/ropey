// Note: the "allow unused" is because these are only used when certain features
// are enabled, so this silences the resulting compiler warnings.

#[inline(always)]
#[allow(unused)]
pub(crate) fn starts_with_lf(text: &str) -> bool {
    text.as_bytes().first().map(|&b| b == 0x0A).unwrap_or(false)
}

#[inline(always)]
#[allow(unused)]
pub(crate) fn ends_with_cr(text: &str) -> bool {
    text.as_bytes().last().map(|&b| b == 0x0D).unwrap_or(false)
}

#[inline(always)]
#[allow(unused)]
pub(crate) fn byte_is_lf(text: &str, byte_idx: usize) -> bool {
    text.as_bytes()
        .get(byte_idx)
        .map(|&b| b == 0x0A)
        .unwrap_or(false)
}

#[inline(always)]
#[allow(unused)]
pub(crate) fn byte_is_cr(text: &str, byte_idx: usize) -> bool {
    text.as_bytes()
        .get(byte_idx)
        .map(|&b| b == 0x0D)
        .unwrap_or(false)
}

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
pub(crate) mod lines {
    use crate::LineType;

    #[inline(always)]
    pub(crate) fn from_byte_idx(text: &str, byte_idx: usize, line_type: LineType) -> usize {
        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => str_indices::lines_lf::from_byte_idx(text, byte_idx),
            #[cfg(feature = "metric_lines_lf_cr")]
            LineType::LF_CR => str_indices::lines_crlf::from_byte_idx(text, byte_idx),
            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => str_indices::lines::from_byte_idx(text, byte_idx),
        }
    }

    #[inline(always)]
    pub(crate) fn to_byte_idx(text: &str, byte_idx: usize, line_type: LineType) -> usize {
        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => str_indices::lines_lf::to_byte_idx(text, byte_idx),
            #[cfg(feature = "metric_lines_lf_cr")]
            LineType::LF_CR => str_indices::lines_crlf::to_byte_idx(text, byte_idx),
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
            #[cfg(feature = "metric_lines_lf_cr")]
            LineType::LF_CR => str_indices::lines_crlf::count_breaks(text),
            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => str_indices::lines::count_breaks(text),
        }
    }

    /// Returns the byte index of the start of the last line of the passed text.
    ///
    /// Note: if the text ends in a line break, that means the last line is
    /// an empty line that starts at the end of the text.
    pub(crate) fn last_line_start_byte_idx(text: &str, line_type: LineType) -> usize {
        // Silence unused parameter warning with certain feature
        // configurations.
        let _ = line_type;

        let mut itr = text.bytes().enumerate().rev();

        while let Some((idx, byte)) = itr.next() {
            if byte == 0x0A {
                return idx + 1;
            }

            // That was the only case for `LineType::LF`, so early out if that's
            // the line type.
            #[cfg(feature = "metric_lines_lf")]
            if line_type == LineType::LF {
                continue;
            }

            if byte == 0x0D {
                return idx + 1;
            }

            // That was the last case for `LineType::LF_CR`, so early out if
            // that's the line type.
            #[cfg(feature = "metric_lines_lf_cr")]
            if line_type == LineType::LF_CR {
                continue;
            }

            // Handle the remaining unicode cases.
            match byte {
                0x0B | 0x0C => {
                    return idx + 1;
                }
                0x85 => {
                    if let Some((_, 0xC2)) = itr.next() {
                        return idx + 1;
                    }
                }
                0xA8 | 0xA9 => {
                    if let Some((_, 0x80)) = itr.next() {
                        if let Some((_, 0xE2)) = itr.next() {
                            return idx + 1;
                        }
                    }
                }
                _ => {}
            }
        }

        return 0;
    }
    /// If there is a trailing line break, returns its byte index.
    /// Otherwise returns `None`.
    ///
    /// Note: a CRLF pair is always treated as a single unit, and thus
    /// this function will return the index of the CR in such cases, even
    /// with `LineType::LF` where CR is not on its own recognized as a line
    /// break.
    pub(crate) fn trailing_line_break_idx(text: &str, line_type: LineType) -> Option<usize> {
        // Silence unused parameter warning with certain feature
        // configurations.
        let _ = line_type;

        if text.len() == 0 {
            return None;
        }

        let text = text.as_bytes();
        let last_byte = text[text.len() - 1];

        // First handle LF and CRLF since that's the most typical case, and
        // also because it's the same for all line types.
        if last_byte == 0x0A {
            if text.len() > 1 {
                let second_to_last_byte = text[text.len() - 2];
                if second_to_last_byte == 0x0D {
                    return Some(text.len() - 2);
                }
            }

            return Some(text.len() - 1);
        }

        // That was the only case for `LineType::LF`, so early out if that's
        // the line type.
        #[cfg(feature = "metric_lines_lf")]
        if line_type == LineType::LF {
            return None;
        }

        // Next we handle CR on its own.
        if last_byte == 0x0D {
            return Some(text.len() - 1);
        }

        // That was the last case for `LineType::LF_CR`, so early out if
        // that's the line type.
        #[cfg(feature = "metric_lines_lf_cr")]
        if line_type == LineType::LF_CR {
            return None;
        }

        // Last char and its byte index.
        let last_char_byte_idx = crate::floor_char_boundary(text.len() - 1, text);
        let last_char = &text[last_char_byte_idx..];

        // Handle the remaining unicode cases.
        match last_char {
            // - VT (Vertical Tab)
            // - FF (Form Feed)
            // - NEL (Next Line)
            // - Line Separator
            // - Paragraph Separator
            &[0xb] | &[0xc] | &[0xc2, 0x85] | &[0xe2, 0x80, 0xa8] | &[0xe2, 0x80, 0xa9] => {
                Some(last_char_byte_idx)
            }
            _ => None,
        }
    }

    pub(crate) fn ends_with_line_break(text: &str, line_type: LineType) -> bool {
        trailing_line_break_idx(text, line_type).is_some()
    }

    pub(crate) fn trim_trailing_line_break(text: &str, line_type: LineType) -> &str {
        if let Some(idx) = trailing_line_break_idx(text, line_type) {
            &text[..idx]
        } else {
            text
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
