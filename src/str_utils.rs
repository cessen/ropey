//! Utility functions for utf8 string slices.
//!
//! This module provides various utility functions that operate on string
//! slices in ways compatible with Ropey.  They may be useful when building
//! additional functionality on top of Ropey.

pub(crate) use str_indices::chars::count as count_chars;
pub use str_indices::chars::from_byte_idx as byte_to_char_idx;
pub use str_indices::chars::to_byte_idx as char_to_byte_idx;
pub(crate) use str_indices::utf16::count_surrogates as count_utf16_surrogates;
pub(crate) use str_indices::utf16::from_byte_idx as utf16_code_unit_to_char_idx;

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
    str_indices::lines::from_byte_idx(text, str_indices::chars::to_byte_idx(text, char_idx))
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
    str_indices::chars::from_byte_idx(text, str_indices::lines::to_byte_idx(text, line_idx))
}

//-------------------------------------------------------------

pub(crate) fn byte_to_utf16_surrogate_idx(text: &str, byte_idx: usize) -> usize {
    let mut i = byte_idx;
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    str_indices::utf16::count_surrogates(&text[..i])
}

/// Returns the byte position just after the second-to-last line break
/// in `text`, or zero of there is no second-to-last line break.
///
/// This function is narrow in scope, only being used for iterating
/// backwards over the lines of a `str`.
pub(crate) fn prev_line_end_char_idx(text: &str) -> usize {
    let mut itr = text.bytes().enumerate().rev();

    // This code always needs to execute, but the variable is only needed
    // for certain feature sets, so silence the warning.
    #[allow(unused_variables)]
    let first_byte = if let Some((_, byte)) = itr.next() {
        byte
    } else {
        return 0;
    };

    while let Some((idx, byte)) = itr.next() {
        match byte {
            0x0A => {
                return idx + 1;
            }
            0x0D =>
            {
                #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
                if first_byte != 0x0A {
                    return idx + 1;
                }
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
    fn prev_line_end_char_idx_lf_01() {
        let mut text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                        There\u{2028}is something.\u{2029}";

        assert_eq!(48, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(8, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(1, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(0, text.len());
    }

    #[cfg(all(feature = "cr_lines", not(feature = "unicode_lines")))]
    #[test]
    fn prev_line_end_char_idx_crlf_01() {
        let mut text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                        There\u{2028}is something.\u{2029}";

        assert_eq!(48, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(9, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(8, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(1, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(0, text.len());
    }

    #[cfg(feature = "unicode_lines")]
    #[test]
    fn prev_line_end_char_idx_unicode_01() {
        let mut text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                        There\u{2028}is something.\u{2029}";

        assert_eq!(48, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(32, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(22, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(17, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(13, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(9, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(8, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(1, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(0, text.len());
    }

    #[test]
    fn ends_with_line_break_01() {
        assert_eq!(true, ends_with_line_break("\n"));

        #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
        assert_eq!(true, ends_with_line_break("\r"));

        #[cfg(feature = "unicode_lines")]
        {
            assert_eq!(true, ends_with_line_break("\u{000A}"));
            assert_eq!(true, ends_with_line_break("\u{000B}"));
            assert_eq!(true, ends_with_line_break("\u{000C}"));
            assert_eq!(true, ends_with_line_break("\u{000D}"));
            assert_eq!(true, ends_with_line_break("\u{0085}"));
            assert_eq!(true, ends_with_line_break("\u{2028}"));
            assert_eq!(true, ends_with_line_break("\u{2029}"));
        }
    }

    #[test]
    fn ends_with_line_break_02() {
        assert_eq!(true, ends_with_line_break("Hi there!\n"));

        #[cfg(any(feature = "cr_lines", feature = "unicode_lines"))]
        assert_eq!(true, ends_with_line_break("Hi there!\r"));

        #[cfg(feature = "unicode_lines")]
        {
            assert_eq!(true, ends_with_line_break("Hi there!\u{000A}"));
            assert_eq!(true, ends_with_line_break("Hi there!\u{000B}"));
            assert_eq!(true, ends_with_line_break("Hi there!\u{000C}"));
            assert_eq!(true, ends_with_line_break("Hi there!\u{000D}"));
            assert_eq!(true, ends_with_line_break("Hi there!\u{0085}"));
            assert_eq!(true, ends_with_line_break("Hi there!\u{2028}"));
            assert_eq!(true, ends_with_line_break("Hi there!\u{2029}"));
        }
    }

    #[test]
    fn ends_with_line_break_03() {
        assert_eq!(false, ends_with_line_break(""));
        assert_eq!(false, ends_with_line_break("a"));
        assert_eq!(false, ends_with_line_break("Hi there!"));
    }

    #[test]
    fn ends_with_line_break_04() {
        assert_eq!(false, ends_with_line_break("\na"));
        assert_eq!(false, ends_with_line_break("\ra"));
        assert_eq!(false, ends_with_line_break("\u{000A}a"));
        assert_eq!(false, ends_with_line_break("\u{000B}a"));
        assert_eq!(false, ends_with_line_break("\u{000C}a"));
        assert_eq!(false, ends_with_line_break("\u{000D}a"));
        assert_eq!(false, ends_with_line_break("\u{0085}a"));
        assert_eq!(false, ends_with_line_break("\u{2028}a"));
        assert_eq!(false, ends_with_line_break("\u{2029}a"));
    }
}
