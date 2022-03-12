//! Utility functions for utf8 string slices.
//!
//! This module provides various utility functions that operate on string
//! slices in ways compatible with Ropey.  They may be useful when building
//! additional functionality on top of Ropey.

pub use str_indices::chars::from_byte_idx as byte_to_char_idx;
pub use str_indices::chars::to_byte_idx as char_to_byte_idx;
pub use str_indices::lines::from_byte_idx as byte_to_line_idx;
pub use str_indices::lines::to_byte_idx as line_to_byte_idx;

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

/// Counts the utf16 surrogate pairs that would be in `text` if it were encoded
/// as utf16.
pub(crate) fn count_utf16_surrogates(text: &str) -> usize {
    str_indices::utf16::count_surrogates(text)
}

/// Returns the byte position just after the second-to-last line break
/// in `text`, or zero of there is no second-to-last line break.
///
/// This function is narrow in scope, only being used for iterating
/// backwards over the lines of a `str`.
pub(crate) fn prev_line_end_char_idx(text: &str) -> usize {
    let mut itr = text.bytes().enumerate().rev();

    let first_byte = if let Some((_, byte)) = itr.next() {
        byte
    } else {
        return 0;
    };

    while let Some((idx, byte)) = itr.next() {
        match byte {
            0x0A | 0x0B | 0x0C => {
                return idx + 1;
            }
            0x0D => {
                if first_byte != 0x0A {
                    return idx + 1;
                }
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
    matches!(
        &text[i..],
        "\u{000A}" | "\u{000B}" | "\u{000C}" | "\u{000D}" | "\u{0085}" | "\u{2028}" | "\u{2029}"
    )
}

/// Uses bit-fiddling magic to count utf8 chars really quickly.
/// We actually count the number of non-starting utf8 bytes, since
/// they have a consistent starting two-bit pattern.  We then
/// subtract from the byte length of the text to get the final
/// count.
#[inline]
pub(crate) fn count_chars(text: &str) -> usize {
    byte_to_char_idx(text, text.len())
}

/// Uses bit-fiddling magic to count line breaks really quickly.
///
/// The following unicode sequences are considered newlines by this function:
/// - u{000A}        (Line Feed)
/// - u{000B}        (Vertical Tab)
/// - u{000C}        (Form Feed)
/// - u{000D}        (Carriage Return)
/// - u{000D}u{000A} (Carriage Return + Line Feed)
/// - u{0085}        (Next Line)
/// - u{2028}        (Line Separator)
/// - u{2029}        (Paragraph Separator)
#[inline]
pub(crate) fn count_line_breaks(text: &str) -> usize {
    byte_to_line_idx(text, text.len())
}

//======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    #[test]
    fn count_chars_01() {
        let text = "Hello せかい! Hello せかい! Hello せかい! Hello せかい! Hello せかい!";

        assert_eq!(54, count_chars(text));
    }

    #[test]
    fn count_chars_02() {
        assert_eq!(100, count_chars(TEXT_LINES));
    }

    #[test]
    fn prev_line_end_char_idx_01() {
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
    fn count_line_breaks_01() {
        let text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                    There\u{2028}is something.\u{2029}";
        assert_eq!(48, text.len());
        assert_eq!(8, count_line_breaks(text));
    }

    #[test]
    fn ends_with_line_break_01() {
        assert_eq!(true, ends_with_line_break("\n"));
        assert_eq!(true, ends_with_line_break("\r"));
        assert_eq!(true, ends_with_line_break("\u{000A}"));
        assert_eq!(true, ends_with_line_break("\u{000B}"));
        assert_eq!(true, ends_with_line_break("\u{000C}"));
        assert_eq!(true, ends_with_line_break("\u{000D}"));
        assert_eq!(true, ends_with_line_break("\u{0085}"));
        assert_eq!(true, ends_with_line_break("\u{2028}"));
        assert_eq!(true, ends_with_line_break("\u{2029}"));
    }

    #[test]
    fn ends_with_line_break_02() {
        assert_eq!(true, ends_with_line_break("Hi there!\n"));
        assert_eq!(true, ends_with_line_break("Hi there!\r"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{000A}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{000B}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{000C}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{000D}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{0085}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{2028}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{2029}"));
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
