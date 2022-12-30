//! Utility functions for utf8 string slices.
//!
//! This module provides various utility functions that operate on string
//! slices in ways compatible with Ropey.  They may be useful when building
//! additional functionality on top of Ropey.

pub(crate) use str_indices::chars::count as count_chars;
pub use str_indices::chars::from_byte_idx as byte_to_char_idx;
pub use str_indices::chars::to_byte_idx as char_to_byte_idx;
pub(crate) use str_indices::utf16::count_surrogates as count_utf16_surrogates;

// Determine which line implementation to use.
#[cfg(feature = "unicode_lines")]
use str_indices::lines;
#[cfg(all(feature = "cr_lines", not(feature = "unicode_lines")))]
use str_indices::lines_crlf as lines;
#[cfg(not(any(feature = "cr_lines", feature = "unicode_lines")))]
use str_indices::lines_lf as lines;

pub(crate) use self::lines::count_breaks as count_line_breaks;
pub use self::lines::from_byte_idx as byte_to_line_idx;
pub use self::lines::to_byte_idx as line_to_byte_idx;

/// Converts from char-index to line-index in a string slice.
///
/// This is equivalent to counting the line endings before the given char.
///
/// Any past-the-end index will return the last line index.
///
/// Runs in O(N) time.
#[inline]
pub fn char_to_line_idx(text: &str, char_idx: usize) -> usize {
    lines::from_byte_idx(text, str_indices::chars::to_byte_idx(text, char_idx))
}

/// Converts from line-index to char-index in a string slice.
///
/// More specifically, this returns the index of the first char of the given line.
///
/// Any past-the-end index will return the one-past-the-end char index.
///
/// Runs in O(N) time.
#[inline]
pub fn line_to_char_idx(text: &str, line_idx: usize) -> usize {
    str_indices::chars::from_byte_idx(text, lines::to_byte_idx(text, line_idx))
}

//-------------------------------------------------------------

pub(crate) fn byte_to_utf16_surrogate_idx(text: &str, byte_idx: usize) -> usize {
    let mut i = byte_idx;
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    str_indices::utf16::count_surrogates(&text[..i])
}

pub(crate) fn utf16_code_unit_to_char_idx(text: &str, utf16_idx: usize) -> usize {
    str_indices::chars::from_byte_idx(text, str_indices::utf16::to_byte_idx(text, utf16_idx))
}

/// Returns the byte index of the start of the last line of the passed text.
///
/// Note: if the text ends in a line break, that means the last line is
/// an empty line that starts at the end of the text.
pub(crate) fn last_line_start_byte_idx(text: &str) -> usize {
    let mut itr = text.bytes().enumerate().rev();

    while let Some((idx, byte)) = itr.next() {
        match byte {
            0x0A => {
                return idx + 1;
            }
            0x0D => {
                #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
                return idx + 1;
            }
            0x0B | 0x0C => {
                #[cfg(feature = "unicode_lines")]
                return idx + 1;
            }
            0x85 =>
            {
                #[cfg(feature = "unicode_lines")]
                if let Some((_, 0xC2)) = itr.next() {
                    return idx + 1;
                }
            }
            0xA8 | 0xA9 =>
            {
                #[cfg(feature = "unicode_lines")]
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

/// Trims a single trailing line break (if any) off the end of the passed string.
///
/// If the string doesn't end in a line break, returns the string unchanged.
#[inline]
pub(crate) fn trim_line_break(text: &str) -> &str {
    if text.is_empty() {
        return "";
    }

    // Find the starting boundary of the last codepoint.
    let mut i = text.len() - 1;
    while !text.is_char_boundary(i) {
        i -= 1;
    }

    let tail = &text[i..];

    // Check if it's one of the fancy unicode line breaks.
    #[cfg(feature = "unicode_lines")]
    if matches!(
        tail,
        "\u{000B}" | "\u{000C}" | "\u{0085}" | "\u{2028}" | "\u{2029}"
    ) {
        return &text[..i];
    }

    #[cfg(feature = "cr_lines")]
    if tail == "\u{000D}" {
        return &text[..i];
    }

    if tail == "\u{000A}" {
        #[cfg(feature = "cr_lines")]
        if i > 0 && text.as_bytes()[i - 1] == 0xd {
            return &text[..(i - 1)];
        }

        return &text[..i];
    }

    return text;
}

/// Returns whether the given string ends in a line break or not.
#[inline]
pub(crate) fn ends_with_line_break(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    // Find the starting boundary of the last codepoint.
    let mut i = text.len() - 1;
    while !text.is_char_boundary(i) {
        i -= 1;
    }

    // Check if the last codepoint is a line break.
    #[cfg(feature = "unicode_lines")]
    return matches!(
        &text[i..],
        "\u{000A}" | "\u{000B}" | "\u{000C}" | "\u{000D}" | "\u{0085}" | "\u{2028}" | "\u{2029}"
    );

    #[cfg(all(feature = "cr_lines", not(feature = "unicode_lines")))]
    return matches!(&text[i..], "\u{000A}" | "\u{000D}");

    #[cfg(not(any(feature = "cr_lines", feature = "unicode_lines")))]
    return &text[i..] == "\u{000A}";
}

//======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(any(feature = "cr_lines", feature = "unicode_lines")))]
    #[test]
    fn last_line_start_byte_idx_lf_01() {
        assert_eq!(0, last_line_start_byte_idx(""));
        assert_eq!(0, last_line_start_byte_idx("Hi"));

        assert_eq!(3, last_line_start_byte_idx("Hi\u{000A}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{000B}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{000C}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{000D}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{0085}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{2028}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{2029}there."));
    }

    #[cfg(not(any(feature = "cr_lines", feature = "unicode_lines")))]
    #[test]
    fn last_line_start_byte_idx_lf_02() {
        let mut text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                        There\u{2028}is something.\u{2029}";

        assert_eq!(48, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(8, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(1, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(0, text.len());
    }

    #[cfg(all(feature = "cr_lines", not(feature = "unicode_lines")))]
    #[test]
    fn last_line_start_byte_idx_crlf_01() {
        assert_eq!(0, last_line_start_byte_idx(""));
        assert_eq!(0, last_line_start_byte_idx("Hi"));

        assert_eq!(3, last_line_start_byte_idx("Hi\u{000A}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{000B}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{000C}there."));
        assert_eq!(3, last_line_start_byte_idx("Hi\u{000D}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{0085}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{2028}there."));
        assert_eq!(0, last_line_start_byte_idx("Hi\u{2029}there."));
    }

    #[cfg(all(feature = "cr_lines", not(feature = "unicode_lines")))]
    #[test]
    fn last_line_start_byte_idx_crlf_02() {
        let mut text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                        There\u{2028}is something.\u{2029}";

        assert_eq!(48, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(9, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(8, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(1, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(0, text.len());
    }

    #[cfg(feature = "unicode_lines")]
    #[test]
    fn last_line_start_byte_idx_unicode_01() {
        assert_eq!(0, last_line_start_byte_idx(""));
        assert_eq!(0, last_line_start_byte_idx("Hi"));

        assert_eq!(3, last_line_start_byte_idx("Hi\u{000A}there."));
        assert_eq!(3, last_line_start_byte_idx("Hi\u{000B}there."));
        assert_eq!(3, last_line_start_byte_idx("Hi\u{000C}there."));
        assert_eq!(3, last_line_start_byte_idx("Hi\u{000D}there."));
        assert_eq!(4, last_line_start_byte_idx("Hi\u{0085}there."));
        assert_eq!(5, last_line_start_byte_idx("Hi\u{2028}there."));
        assert_eq!(5, last_line_start_byte_idx("Hi\u{2029}there."));
    }

    #[cfg(feature = "unicode_lines")]
    #[test]
    fn last_line_start_byte_idx_unicode_02() {
        let mut text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                        There\u{2028}is something.\u{2029}";

        assert_eq!(48, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(32, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(22, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(17, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(13, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(9, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(8, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(1, text.len());
        text = &text[..last_line_start_byte_idx(trim_line_break(text))];
        assert_eq!(0, text.len());
    }

    #[cfg(not(any(feature = "cr_lines", feature = "unicode_lines")))]
    #[test]
    fn trim_line_break_lf_01() {
        assert_eq!("", trim_line_break(""));
        assert_eq!("Hi", trim_line_break("Hi"));

        assert_eq!("Hi", trim_line_break("Hi\u{000A}"));
        assert_eq!("Hi\u{000B}", trim_line_break("Hi\u{000B}"));
        assert_eq!("Hi\u{000C}", trim_line_break("Hi\u{000C}"));
        assert_eq!("Hi\u{000D}", trim_line_break("Hi\u{000D}"));
        assert_eq!("Hi\u{0085}", trim_line_break("Hi\u{0085}"));
        assert_eq!("Hi\u{2028}", trim_line_break("Hi\u{2028}"));
        assert_eq!("Hi\u{2029}", trim_line_break("Hi\u{2029}"));

        assert_eq!("\r", trim_line_break("\r\n"));
        assert_eq!("Hi\r", trim_line_break("Hi\r\n"));
    }

    #[cfg(all(feature = "cr_lines", not(feature = "unicode_lines")))]
    #[test]
    fn trim_line_break_crlf_01() {
        assert_eq!("", trim_line_break(""));
        assert_eq!("Hi", trim_line_break("Hi"));

        assert_eq!("Hi", trim_line_break("Hi\u{000A}"));
        assert_eq!("Hi\u{000B}", trim_line_break("Hi\u{000B}"));
        assert_eq!("Hi\u{000C}", trim_line_break("Hi\u{000C}"));
        assert_eq!("Hi", trim_line_break("Hi\u{000D}"));
        assert_eq!("Hi\u{0085}", trim_line_break("Hi\u{0085}"));
        assert_eq!("Hi\u{2028}", trim_line_break("Hi\u{2028}"));
        assert_eq!("Hi\u{2029}", trim_line_break("Hi\u{2029}"));

        assert_eq!("", trim_line_break("\r\n"));
        assert_eq!("Hi", trim_line_break("Hi\r\n"));
    }

    #[cfg(feature = "unicode_lines")]
    #[test]
    fn trim_line_break_unicode_01() {
        assert_eq!("", trim_line_break(""));
        assert_eq!("Hi", trim_line_break("Hi"));

        assert_eq!("Hi", trim_line_break("Hi\u{000A}"));
        assert_eq!("Hi", trim_line_break("Hi\u{000B}"));
        assert_eq!("Hi", trim_line_break("Hi\u{000C}"));
        assert_eq!("Hi", trim_line_break("Hi\u{000D}"));
        assert_eq!("Hi", trim_line_break("Hi\u{0085}"));
        assert_eq!("Hi", trim_line_break("Hi\u{2028}"));
        assert_eq!("Hi", trim_line_break("Hi\u{2029}"));

        assert_eq!("", trim_line_break("\r\n"));
        assert_eq!("Hi", trim_line_break("Hi\r\n"));
    }

    #[test]
    fn ends_with_line_break_01() {
        assert!(ends_with_line_break("\n"));

        #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
        assert!(ends_with_line_break("\r"));

        #[cfg(feature = "unicode_lines")]
        {
            assert!(ends_with_line_break("\u{000A}"));
            assert!(ends_with_line_break("\u{000B}"));
            assert!(ends_with_line_break("\u{000C}"));
            assert!(ends_with_line_break("\u{000D}"));
            assert!(ends_with_line_break("\u{0085}"));
            assert!(ends_with_line_break("\u{2028}"));
            assert!(ends_with_line_break("\u{2029}"));
        }
    }

    #[test]
    fn ends_with_line_break_02() {
        assert!(ends_with_line_break("Hi there!\n"));

        #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
        assert!(ends_with_line_break("Hi there!\r"));

        #[cfg(feature = "unicode_lines")]
        {
            assert!(ends_with_line_break("Hi there!\u{000A}"));
            assert!(ends_with_line_break("Hi there!\u{000B}"));
            assert!(ends_with_line_break("Hi there!\u{000C}"));
            assert!(ends_with_line_break("Hi there!\u{000D}"));
            assert!(ends_with_line_break("Hi there!\u{0085}"));
            assert!(ends_with_line_break("Hi there!\u{2028}"));
            assert!(ends_with_line_break("Hi there!\u{2029}"));
        }
    }

    #[test]
    fn ends_with_line_break_03() {
        assert!(!ends_with_line_break(""));
        assert!(!ends_with_line_break("a"));
        assert!(!ends_with_line_break("Hi there!"));
    }

    #[test]
    fn ends_with_line_break_04() {
        assert!(!ends_with_line_break("\na"));
        assert!(!ends_with_line_break("\ra"));
        assert!(!ends_with_line_break("\u{000A}a"));
        assert!(!ends_with_line_break("\u{000B}a"));
        assert!(!ends_with_line_break("\u{000C}a"));
        assert!(!ends_with_line_break("\u{000D}a"));
        assert!(!ends_with_line_break("\u{0085}a"));
        assert!(!ends_with_line_break("\u{2028}a"));
        assert!(!ends_with_line_break("\u{2029}a"));
    }

    #[test]
    fn char_to_line_idx_01() {
        let text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                    There\u{2028}is something.\u{2029}";

        #[cfg(not(any(feature = "cr_lines", feature = "unicode_lines")))]
        {
            assert_eq!(0, char_to_line_idx(text, 0));
            assert_eq!(1, char_to_line_idx(text, 1));
            assert_eq!(2, char_to_line_idx(text, 8));
            assert_eq!(2, char_to_line_idx(text, 38));
        }

        #[cfg(all(feature = "cr_lines", not(feature = "unicode_lines")))]
        {
            assert_eq!(0, char_to_line_idx(text, 0));
            assert_eq!(1, char_to_line_idx(text, 1));
            assert_eq!(2, char_to_line_idx(text, 8));
            assert_eq!(3, char_to_line_idx(text, 9));
            assert_eq!(3, char_to_line_idx(text, 38));
        }

        #[cfg(feature = "unicode_lines")]
        {
            assert_eq!(0, char_to_line_idx(text, 0));
            assert_eq!(1, char_to_line_idx(text, 1));
            assert_eq!(2, char_to_line_idx(text, 8));
            assert_eq!(3, char_to_line_idx(text, 9));
            assert_eq!(4, char_to_line_idx(text, 11));
            assert_eq!(5, char_to_line_idx(text, 13));
            assert_eq!(6, char_to_line_idx(text, 15));
            assert_eq!(7, char_to_line_idx(text, 23));
            assert_eq!(8, char_to_line_idx(text, 37));
            assert_eq!(8, char_to_line_idx(text, 38));
        }
    }

    #[test]
    fn line_to_char_idx_01() {
        let text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                    There\u{2028}is something.\u{2029}";

        #[cfg(not(any(feature = "cr_lines", feature = "unicode_lines")))]
        {
            assert_eq!(0, line_to_char_idx(text, 0));
            assert_eq!(1, line_to_char_idx(text, 1));
            assert_eq!(8, line_to_char_idx(text, 2));
            assert_eq!(37, line_to_char_idx(text, 3));
        }

        #[cfg(all(feature = "cr_lines", not(feature = "unicode_lines")))]
        {
            assert_eq!(0, line_to_char_idx(text, 0));
            assert_eq!(1, line_to_char_idx(text, 1));
            assert_eq!(8, line_to_char_idx(text, 2));
            assert_eq!(9, line_to_char_idx(text, 3));
            assert_eq!(37, line_to_char_idx(text, 4));
        }

        #[cfg(feature = "unicode_lines")]
        {
            assert_eq!(0, line_to_char_idx(text, 0));
            assert_eq!(1, line_to_char_idx(text, 1));
            assert_eq!(8, line_to_char_idx(text, 2));
            assert_eq!(9, line_to_char_idx(text, 3));
            assert_eq!(11, line_to_char_idx(text, 4));
            assert_eq!(13, line_to_char_idx(text, 5));
            assert_eq!(15, line_to_char_idx(text, 6));
            assert_eq!(23, line_to_char_idx(text, 7));
            assert_eq!(37, line_to_char_idx(text, 8));
            assert_eq!(37, line_to_char_idx(text, 9));
        }
    }
}
