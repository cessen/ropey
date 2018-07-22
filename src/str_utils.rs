//! Utility functions for utf8 string slices.
//!
//! This module provides various utility functions that operate on string
//! slices in ways compatible with Ropey.  They may be useful when building
//! additional functionality on top of Ropey.

use std;

const TSIZE: usize = std::mem::size_of::<usize>(); // Shorthand for usize size.

/// Converts from byte-index to char-index in a string slice.
///
/// If the byte is in the middle of a multi-byte char, returns the index of
/// the char that the byte belongs to.
///
/// Any past-the-end index will return the one-past-the-end char index.
#[inline]
pub fn byte_to_char_idx(text: &str, byte_idx: usize) -> usize {
    if byte_idx == 0 {
        return 0;
    } else if byte_idx >= text.len() {
        return count_chars(text);
    } else {
        return count_chars(unsafe {
            std::str::from_utf8_unchecked(&text.as_bytes()[0..(byte_idx + 1)])
        }) - 1;
    }
}

/// Converts from byte-index to line-index in a string slice.
///
/// This is equivalent to counting the line endings before the given byte.
///
/// Any past-the-end index will return the last line index.
#[inline]
pub fn byte_to_line_idx(text: &str, byte_idx: usize) -> usize {
    use crlf;
    let mut byte_idx = byte_idx.min(text.len());
    while !text.is_char_boundary(byte_idx) {
        byte_idx -= 1;
    }
    let nl_count = count_line_breaks(&text[..byte_idx]);
    if crlf::is_break(byte_idx, text.as_bytes()) {
        nl_count
    } else {
        nl_count - 1
    }
}

/// Converts from char-index to byte-index in a string slice.
///
/// Any past-the-end index will return the one-past-the-end byte index.
#[inline]
pub fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    const ONEMASK: usize = std::usize::MAX / 0xFF;

    let mut char_count = 0;
    let mut ptr = text.as_ptr();
    let start_ptr = text.as_ptr();
    let end_ptr = unsafe { ptr.offset(text.len() as isize) };

    // Take care of any unaligned bytes at the beginning
    let end_pre_ptr = {
        let aligned = ptr as usize + (TSIZE - (ptr as usize & (TSIZE - 1)));
        (end_ptr as usize).min(aligned) as *const u8
    };
    while ptr < end_pre_ptr && char_count <= char_idx {
        let byte = unsafe { *ptr };
        char_count += ((byte & 0xC0) != 0x80) as usize;
        ptr = unsafe { ptr.offset(1) };
    }

    // Use usize to count multiple bytes at once, using bit-fiddling magic.
    let mut ptr = ptr as *const usize;
    let end_mid_ptr = (end_ptr as usize - (end_ptr as usize & (TSIZE - 1))) as *const usize;
    while ptr < end_mid_ptr && (char_count + TSIZE) <= char_idx {
        // Do the clever counting
        let n = unsafe { *ptr };
        let byte_bools = (!((n >> 7) & (!n >> 6))) & ONEMASK;
        char_count += (byte_bools.wrapping_mul(ONEMASK)) >> ((TSIZE - 1) * 8);
        ptr = unsafe { ptr.offset(1) };
    }

    // Take care of any unaligned bytes at the end
    let mut ptr = ptr as *const u8;
    while ptr < end_ptr && char_count <= char_idx {
        let byte = unsafe { *ptr };
        char_count += ((byte & 0xC0) != 0x80) as usize;
        ptr = unsafe { ptr.offset(1) };
    }

    // Finish up
    let byte_count = ptr as usize - start_ptr as usize;
    if ptr == end_ptr && char_count <= char_idx {
        byte_count
    } else {
        byte_count - 1
    }
}

/// Converts from char-index to line-index in a string slice.
///
/// This is equivalent to counting the line endings before the given char.
///
/// Any past-the-end index will return the last line index.
#[inline]
pub fn char_to_line_idx(text: &str, char_idx: usize) -> usize {
    byte_to_line_idx(text, char_to_byte_idx(text, char_idx))
}

/// Converts from line-index to byte-index in a string slice.
///
/// More specifically, this returns the index of the first byte of the given
/// line.
///
/// Any past-the-end index will return the one-past-the-end byte index.
#[inline(never)]
pub fn line_to_byte_idx(text: &str, line_idx: usize) -> usize {
    let len = text.len();
    let mut ptr = text.as_ptr();
    let start_ptr = text.as_ptr();
    let end_ptr = unsafe { ptr.offset(len as isize) };
    let mut line_break_count = 0;

    while ptr < end_ptr {
        // Calculate the next aligned ptr after this one
        let end_aligned_ptr = next_aligned_ptr(ptr, TSIZE).min(end_ptr);

        // Count line breaks a byte at a time.
        while ptr < end_aligned_ptr && line_break_count < line_idx {
            let byte = unsafe { *ptr };

            // Handle u{000A}, u{000B}, u{000C}, and u{000D}
            if (byte <= 0x0D) && (byte >= 0x0A) {
                // Check for CRLF and go forward one more if it is
                let next = unsafe { ptr.offset(1) };
                if byte == 0x0D && next < end_ptr && unsafe { *next } == 0x0A {
                    ptr = next;
                }

                line_break_count += 1;
            }
            // Handle u{0085}
            else if byte == 0xC2 {
                ptr = unsafe { ptr.offset(1) };
                if ptr < end_ptr && unsafe { *ptr } == 0x85 {
                    line_break_count += 1;
                }
            }
            // Handle u{2028} and u{2029}
            else if byte == 0xE2 {
                let next1 = unsafe { ptr.offset(1) };
                let next2 = unsafe { ptr.offset(2) };
                if next1 < end_ptr
                    && next2 < end_ptr
                    && unsafe { *next1 } == 0x80
                    && (unsafe { *next2 } >> 1) == 0x54
                {
                    line_break_count += 1;
                }
                ptr = unsafe { ptr.offset(2) };
            }

            ptr = unsafe { ptr.offset(1) };
        }

        // Have we counted all the lines for the conversion?
        if line_break_count >= line_idx {
            break;
        }

        // Use usize to count line breaks in big chunks.
        if ptr == end_aligned_ptr {
            while unsafe { ptr.offset(TSIZE as isize) } < end_ptr {
                let lb =
                    line_break_count + unsafe { count_line_breaks_in_usize_from_ptr(ptr, end_ptr) };

                if lb >= line_idx {
                    break;
                } else {
                    line_break_count = lb;
                }

                ptr = unsafe { ptr.offset(TSIZE as isize) };
            }
        }
    }

    // Finish up
    ptr as usize - start_ptr as usize
}

/// Converts from line-index to char-index in a string slice.
///
/// More specifically, this returns the index of the first char of the given
/// line.
///
/// Any past-the-end index will return the one-past-the-end char index.
#[inline]
pub fn line_to_char_idx(text: &str, line_idx: usize) -> usize {
    byte_to_char_idx(text, line_to_byte_idx(text, line_idx))
}

//===========================================================================
// Internal
//===========================================================================

/// Uses bit-fiddling magic to count utf8 chars really quickly.
/// We actually count the number of non-starting utf8 bytes, since
/// they have a consistent starting two-bit pattern.  We then
/// subtract from the byte length of the text to get the final
/// count.
#[inline]
pub(crate) fn count_chars(text: &str) -> usize {
    const ONEMASK: usize = std::usize::MAX / 0xFF;

    let len = text.len();
    let mut ptr = text.as_ptr();
    let end_ptr = unsafe { ptr.offset(len as isize) };
    let mut inv_count = 0;

    // Take care of any unaligned bytes at the beginning
    let end_pre_ptr = align_ptr(ptr, TSIZE).min(end_ptr);
    while ptr < end_pre_ptr {
        let byte = unsafe { *ptr };
        inv_count += ((byte & 0xC0) == 0x80) as usize;
        ptr = unsafe { ptr.offset(1) };
    }

    // Use usize to count multiple bytes at once, using bit-fiddling magic.
    let mut ptr = ptr as *const usize;
    let end_mid_ptr = (end_ptr as usize - (end_ptr as usize & (TSIZE - 1))) as *const usize;
    while ptr < end_mid_ptr {
        // Do the clever counting
        let n = unsafe { *ptr };
        let byte_bools = ((n >> 7) & (!n >> 6)) & ONEMASK;
        inv_count += (byte_bools.wrapping_mul(ONEMASK)) >> ((TSIZE - 1) * 8);
        ptr = unsafe { ptr.offset(1) };
    }

    // Take care of any unaligned bytes at the end
    let mut ptr = ptr as *const u8;
    while ptr < end_ptr {
        let byte = unsafe { *ptr };
        inv_count += ((byte & 0xC0) == 0x80) as usize;
        ptr = unsafe { ptr.offset(1) };
    }

    len - inv_count
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
#[inline(never)] // Actually slightly faster when inlining is not allowed
pub(crate) fn count_line_breaks(text: &str) -> usize {
    let len = text.len();
    let mut ptr = text.as_ptr();
    let end_ptr = unsafe { ptr.offset(len as isize) };
    let mut count = 0;

    while ptr < end_ptr {
        // Calculate the next aligned ptr after this one
        let end_aligned_ptr = next_aligned_ptr(ptr, TSIZE).min(end_ptr);

        // Count line breaks a byte at a time.
        while ptr < end_aligned_ptr {
            let byte = unsafe { *ptr };

            // Handle u{000A}, u{000B}, u{000C}, and u{000D}
            if (byte <= 0x0D) && (byte >= 0x0A) {
                // Check for CRLF and go forward one more if it is
                let next = unsafe { ptr.offset(1) };
                if byte == 0x0D && next < end_ptr && unsafe { *next } == 0x0A {
                    ptr = next;
                }

                count += 1;
            }
            // Handle u{0085}
            else if byte == 0xC2 {
                ptr = unsafe { ptr.offset(1) };
                if ptr < end_ptr && unsafe { *ptr } == 0x85 {
                    count += 1;
                }
            }
            // Handle u{2028} and u{2029}
            else if byte == 0xE2 {
                let next1 = unsafe { ptr.offset(1) };
                let next2 = unsafe { ptr.offset(2) };
                if next1 < end_ptr
                    && next2 < end_ptr
                    && unsafe { *next1 } == 0x80
                    && (unsafe { *next2 } >> 1) == 0x54
                {
                    count += 1;
                }
                ptr = unsafe { ptr.offset(2) };
            }

            ptr = unsafe { ptr.offset(1) };
        }

        // Use usize to count line breaks in big chunks.
        if ptr == end_aligned_ptr {
            while unsafe { ptr.offset(TSIZE as isize) } < end_ptr {
                count += unsafe { count_line_breaks_in_usize_from_ptr(ptr, end_ptr) };
                ptr = unsafe { ptr.offset(TSIZE as isize) };
            }
        }
    }

    count
}

/// Used internally in the line-break counting functions.
///
/// ptr MUST be aligned to usize alignment.
#[inline(always)]
unsafe fn count_line_breaks_in_usize_from_ptr(ptr: *const u8, end_ptr: *const u8) -> usize {
    let mut count = 0;
    let n = *(ptr as *const usize);
    let next_ptr = ptr.offset(TSIZE as isize);

    let nl_1_flags = flag_bytes(n, 0xC2);
    let sp_1_flags = flag_bytes(n, 0xE2);

    if !(nl_1_flags == 0 && sp_1_flags == 0) {
        // Next Line: u{0085}
        if nl_1_flags != 0 {
            let nl_2_flags = shift_bytes_back(flag_bytes(n, 0x85), 1);
            count += count_flag_bytes(nl_1_flags & nl_2_flags);

            // Handle ending boundary
            if next_ptr < end_ptr && *next_ptr.offset(-1) == 0xC2 && *next_ptr == 0x85 {
                count += 1;
            }
        }

        // Line Separator:      u{2028}
        // Paragraph Separator: u{2029}
        if sp_1_flags != 0 {
            let sp_2_flags = sp_1_flags & shift_bytes_back(flag_bytes(n, 0x80), 1);
            if sp_2_flags != 0 {
                let sp_3_flags = flag_bytes(n, 0xA8);
                let sp_4_flags = flag_bytes(n, 0xA9);
                let sp_flags = sp_2_flags & shift_bytes_back(sp_3_flags | sp_4_flags, 2);
                count += count_flag_bytes(sp_flags);
            }

            // Handle ending boundary
            if next_ptr < end_ptr
                && *next_ptr.offset(-2) == 0xE2
                && *next_ptr.offset(-1) == 0x80
                && (*next_ptr >> 1) == 0x54
            {
                count += 1;
            } else if next_ptr.offset(1) < end_ptr
                && *next_ptr.offset(-1) == 0xE2
                && *next_ptr == 0x80
                && (*next_ptr.offset(1) >> 1) == 0x54
            {
                count += 1;
            }
        }
    }

    // Line Feed:                   u{000A}
    // Vertical Tab:                u{000B}
    // Form Feed:                   u{000C}
    // Carriage Return:             u{000D}
    // Carriage Return + Line Feed: u{000D}u{000A}
    if has_bytes_less_than(n, 0x0E) {
        let lf_flags = flag_bytes(n, 0x0A);
        count += count_flag_bytes(lf_flags);
        let vt_flags = flag_bytes(n, 0x0B);
        count += count_flag_bytes(vt_flags);
        let ff_flags = flag_bytes(n, 0x0C);
        count += count_flag_bytes(ff_flags);
        let cr_flags = flag_bytes(n, 0x0D);
        count += count_flag_bytes(cr_flags);

        // Handle CRLF
        if cr_flags != 0 {
            let crlf_flags = cr_flags & shift_bytes_back(lf_flags, 1);
            count -= count_flag_bytes(crlf_flags);
            if next_ptr < end_ptr && *next_ptr.offset(-1) == 0x0D && *next_ptr == 0x0A {
                count -= 1;
            }
        }
    }

    count
}

#[inline(always)]
fn flag_zero_bytes(word: usize) -> usize {
    const ONEMASK_LOW: usize = std::usize::MAX / 0xFF;
    const ONEMASK_HIGH: usize = ONEMASK_LOW << 7;
    let a = !word;
    let b = a & (a << 4);
    let c = b & (b << 2);
    c & (c << 1) & ONEMASK_HIGH
}

#[inline(always)]
fn flag_bytes(word: usize, n: u8) -> usize {
    const ONEMASK_LOW: usize = std::usize::MAX / 0xFF;
    flag_zero_bytes(word ^ (n as usize * ONEMASK_LOW))
}

#[inline(always)]
fn count_flag_bytes(word: usize) -> usize {
    if word == 0 {
        0
    } else {
        word / 128 % 255
    }
}

#[inline(always)]
fn shift_bytes_back(word: usize, n: usize) -> usize {
    if cfg!(target_endian = "little") {
        word >> (n * 8)
    } else {
        word << (n * 8)
    }
}

#[inline(always)]
#[allow(unused)] // Used in tests
fn has_byte(word: usize, n: u8) -> bool {
    flag_bytes(word, n) != 0
}

#[inline(always)]
fn has_bytes_less_than(word: usize, n: u8) -> bool {
    const ONEMASK: usize = std::usize::MAX / 0xFF;
    ((word.wrapping_sub(ONEMASK * n as usize)) & !word & (ONEMASK * 128)) != 0
}

/// Returns the next pointer after `ptr` that is aligned with `alignment`.
///
/// NOTE: only works for power-of-two alignments.
#[inline(always)]
fn next_aligned_ptr<T>(ptr: *const T, alignment: usize) -> *const T {
    (ptr as usize + alignment - (ptr as usize & (alignment - 1))) as *const T
}

/// Returns `ptr` if aligned to `alignment`, or the next aligned pointer
/// after if not.
///
/// NOTE: only works for power-of-two alignments.
#[inline(always)]
fn align_ptr<T>(ptr: *const T, alignment: usize) -> *const T {
    next_aligned_ptr(unsafe { ptr.offset(-1) }, alignment)
}

//======================================================================

/// An iterator that yields the byte indices of line breaks in a string.
/// A line break in this case is the point immediately *after* a newline
/// character.
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
#[allow(unused)] // Used in tests, as reference solution.
struct LineBreakIter<'a> {
    byte_itr: std::str::Bytes<'a>,
    byte_idx: usize,
}

#[allow(unused)]
impl<'a> LineBreakIter<'a> {
    #[inline]
    fn new(text: &str) -> LineBreakIter {
        LineBreakIter {
            byte_itr: text.bytes(),
            byte_idx: 0,
        }
    }
}

impl<'a> Iterator for LineBreakIter<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        while let Some(byte) = self.byte_itr.next() {
            self.byte_idx += 1;
            // Handle u{000A}, u{000B}, u{000C}, and u{000D}
            if (byte <= 0x0D) && (byte >= 0x0A) {
                if byte == 0x0D {
                    // We're basically "peeking" here.
                    if let Some(0x0A) = self.byte_itr.clone().next() {
                        self.byte_itr.next();
                        self.byte_idx += 1;
                    }
                }
                return Some(self.byte_idx);
            }
            // Handle u{0085}
            else if byte == 0xC2 {
                self.byte_idx += 1;
                if let Some(0x85) = self.byte_itr.next() {
                    return Some(self.byte_idx);
                }
            }
            // Handle u{2028} and u{2029}
            else if byte == 0xE2 {
                self.byte_idx += 2;
                let byte2 = self.byte_itr.next().unwrap();
                let byte3 = self.byte_itr.next().unwrap() >> 1;
                if byte2 == 0x80 && byte3 == 0x54 {
                    return Some(self.byte_idx);
                }
            }
        }

        return None;
    }
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
        let text =
            "Hello せかい! Hello せかい! Hello せかい! Hello せかい! Hello せかい!";

        assert_eq!(54, count_chars(text));
    }

    #[test]
    fn count_chars_02() {
        assert_eq!(100, count_chars(TEXT_LINES));
    }

    #[test]
    fn line_breaks_iter_01() {
        let text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                    There\u{2028}is something.\u{2029}";
        let mut itr = LineBreakIter::new(text);
        assert_eq!(48, text.len());
        assert_eq!(Some(1), itr.next());
        assert_eq!(Some(8), itr.next());
        assert_eq!(Some(9), itr.next());
        assert_eq!(Some(13), itr.next());
        assert_eq!(Some(17), itr.next());
        assert_eq!(Some(22), itr.next());
        assert_eq!(Some(32), itr.next());
        assert_eq!(Some(48), itr.next());
        assert_eq!(None, itr.next());
    }

    #[test]
    fn count_line_breaks_01() {
        let text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                    There\u{2028}is something.\u{2029}";
        assert_eq!(48, text.len());
        assert_eq!(8, count_line_breaks(text));
    }

    #[test]
    fn count_line_breaks_02() {
        let text = "\u{000A}Hello world!  This is a longer text.\u{000D}\u{000A}\u{000D}To better test that skipping by usize doesn't mess things up.\u{000B}Hello せかい!\u{000C}\u{0085}Yet more text.  How boring.\u{2028}Hi.\u{2029}\u{000A}Hello world!  This is a longer text.\u{000D}\u{000A}\u{000D}To better test that skipping by usize doesn't mess things up.\u{000B}Hello せかい!\u{000C}\u{0085}Yet more text.  How boring.\u{2028}Hi.\u{2029}\u{000A}Hello world!  This is a longer text.\u{000D}\u{000A}\u{000D}To better test that skipping by usize doesn't mess things up.\u{000B}Hello せかい!\u{000C}\u{0085}Yet more text.  How boring.\u{2028}Hi.\u{2029}\u{000A}Hello world!  This is a longer text.\u{000D}\u{000A}\u{000D}To better test that skipping by usize doesn't mess things up.\u{000B}Hello せかい!\u{000C}\u{0085}Yet more text.  How boring.\u{2028}Hi.\u{2029}";
        assert_eq!(count_line_breaks(text), LineBreakIter::new(text).count());
    }

    #[test]
    fn byte_to_char_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(1, byte_to_char_idx(text, 1));
        assert_eq!(6, byte_to_char_idx(text, 6));
        assert_eq!(6, byte_to_char_idx(text, 7));
        assert_eq!(6, byte_to_char_idx(text, 8));
        assert_eq!(7, byte_to_char_idx(text, 9));
        assert_eq!(7, byte_to_char_idx(text, 10));
        assert_eq!(7, byte_to_char_idx(text, 11));
        assert_eq!(8, byte_to_char_idx(text, 12));
        assert_eq!(8, byte_to_char_idx(text, 13));
        assert_eq!(8, byte_to_char_idx(text, 14));
        assert_eq!(9, byte_to_char_idx(text, 15));
        assert_eq!(10, byte_to_char_idx(text, 16));
        assert_eq!(10, byte_to_char_idx(text, 17));
        assert_eq!(10, byte_to_char_idx(text, 18));
        assert_eq!(10, byte_to_char_idx(text, 19));
    }

    #[test]
    fn byte_to_char_idx_02() {
        let text = "せかい";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(0, byte_to_char_idx(text, 1));
        assert_eq!(0, byte_to_char_idx(text, 2));
        assert_eq!(1, byte_to_char_idx(text, 3));
        assert_eq!(1, byte_to_char_idx(text, 4));
        assert_eq!(1, byte_to_char_idx(text, 5));
        assert_eq!(2, byte_to_char_idx(text, 6));
        assert_eq!(2, byte_to_char_idx(text, 7));
        assert_eq!(2, byte_to_char_idx(text, 8));
        assert_eq!(3, byte_to_char_idx(text, 9));
        assert_eq!(3, byte_to_char_idx(text, 10));
        assert_eq!(3, byte_to_char_idx(text, 11));
        assert_eq!(3, byte_to_char_idx(text, 12));
    }

    #[test]
    fn byte_to_char_idx_03() {
        // Ascii range
        for i in 0..88 {
            assert_eq!(i, byte_to_char_idx(TEXT_LINES, i));
        }

        // Hiragana characters
        for i in 88..125 {
            assert_eq!(88 + ((i - 88) / 3), byte_to_char_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 125..130 {
            assert_eq!(100, byte_to_char_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn byte_to_line_idx_01() {
        let text = "Here\nare\nsome\nwords";
        assert_eq!(0, byte_to_line_idx(text, 0));
        assert_eq!(0, byte_to_line_idx(text, 4));
        assert_eq!(1, byte_to_line_idx(text, 5));
        assert_eq!(1, byte_to_line_idx(text, 8));
        assert_eq!(2, byte_to_line_idx(text, 9));
        assert_eq!(2, byte_to_line_idx(text, 13));
        assert_eq!(3, byte_to_line_idx(text, 14));
        assert_eq!(3, byte_to_line_idx(text, 19));
    }

    #[test]
    fn byte_to_line_idx_02() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(0, byte_to_line_idx(text, 0));
        assert_eq!(1, byte_to_line_idx(text, 1));
        assert_eq!(1, byte_to_line_idx(text, 5));
        assert_eq!(2, byte_to_line_idx(text, 6));
        assert_eq!(2, byte_to_line_idx(text, 9));
        assert_eq!(3, byte_to_line_idx(text, 10));
        assert_eq!(3, byte_to_line_idx(text, 14));
        assert_eq!(4, byte_to_line_idx(text, 15));
        assert_eq!(4, byte_to_line_idx(text, 20));
        assert_eq!(5, byte_to_line_idx(text, 21));
    }

    #[test]
    fn byte_to_line_idx_03() {
        let text = "Here\r\nare\r\nsome\r\nwords";
        assert_eq!(0, byte_to_line_idx(text, 0));
        assert_eq!(0, byte_to_line_idx(text, 4));
        assert_eq!(0, byte_to_line_idx(text, 5));
        assert_eq!(1, byte_to_line_idx(text, 6));
        assert_eq!(1, byte_to_line_idx(text, 9));
        assert_eq!(1, byte_to_line_idx(text, 10));
        assert_eq!(2, byte_to_line_idx(text, 11));
        assert_eq!(2, byte_to_line_idx(text, 15));
        assert_eq!(2, byte_to_line_idx(text, 16));
        assert_eq!(3, byte_to_line_idx(text, 17));
    }

    #[test]
    fn byte_to_line_idx_04() {
        // Line 0
        for i in 0..32 {
            assert_eq!(0, byte_to_line_idx(TEXT_LINES, i));
        }

        // Line 1
        for i in 32..59 {
            assert_eq!(1, byte_to_line_idx(TEXT_LINES, i));
        }

        // Line 2
        for i in 59..88 {
            assert_eq!(2, byte_to_line_idx(TEXT_LINES, i));
        }

        // Line 3
        for i in 88..125 {
            assert_eq!(3, byte_to_line_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 125..130 {
            assert_eq!(3, byte_to_line_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn char_to_byte_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(1, char_to_byte_idx(text, 1));
        assert_eq!(2, char_to_byte_idx(text, 2));
        assert_eq!(5, char_to_byte_idx(text, 5));
        assert_eq!(6, char_to_byte_idx(text, 6));
        assert_eq!(12, char_to_byte_idx(text, 8));
        assert_eq!(15, char_to_byte_idx(text, 9));
        assert_eq!(16, char_to_byte_idx(text, 10));
    }

    #[test]
    fn char_to_byte_idx_02() {
        let text = "せかい";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(3, char_to_byte_idx(text, 1));
        assert_eq!(6, char_to_byte_idx(text, 2));
        assert_eq!(9, char_to_byte_idx(text, 3));
    }

    #[test]
    fn char_to_byte_idx_03() {
        let text = "Hello world!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(1, char_to_byte_idx(text, 1));
        assert_eq!(8, char_to_byte_idx(text, 8));
        assert_eq!(11, char_to_byte_idx(text, 11));
        assert_eq!(12, char_to_byte_idx(text, 12));
    }

    #[test]
    fn char_to_byte_idx_04() {
        let text = "Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(30, char_to_byte_idx(text, 24));
        assert_eq!(60, char_to_byte_idx(text, 48));
        assert_eq!(90, char_to_byte_idx(text, 72));
        assert_eq!(115, char_to_byte_idx(text, 93));
        assert_eq!(120, char_to_byte_idx(text, 96));
        assert_eq!(150, char_to_byte_idx(text, 120));
        assert_eq!(180, char_to_byte_idx(text, 144));
        assert_eq!(210, char_to_byte_idx(text, 168));
        assert_eq!(239, char_to_byte_idx(text, 191));
    }

    #[test]
    fn char_to_byte_idx_05() {
        // Ascii range
        for i in 0..88 {
            assert_eq!(i, char_to_byte_idx(TEXT_LINES, i));
        }

        // Hiragana characters
        for i in 88..100 {
            assert_eq!(88 + ((i - 88) * 3), char_to_byte_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 100..110 {
            assert_eq!(124, char_to_byte_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn char_to_line_idx_01() {
        let text = "Hello せ\nか\nい!";
        assert_eq!(0, char_to_line_idx(text, 0));
        assert_eq!(0, char_to_line_idx(text, 7));
        assert_eq!(1, char_to_line_idx(text, 8));
        assert_eq!(1, char_to_line_idx(text, 9));
        assert_eq!(2, char_to_line_idx(text, 10));
    }

    #[test]
    fn char_to_line_idx_02() {
        // Line 0
        for i in 0..32 {
            assert_eq!(0, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 1
        for i in 32..59 {
            assert_eq!(1, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 2
        for i in 59..88 {
            assert_eq!(2, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 3
        for i in 88..100 {
            assert_eq!(3, char_to_line_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 100..110 {
            assert_eq!(3, char_to_line_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn line_to_byte_idx_01() {
        let text = "Here\r\nare\r\nsome\r\nwords";
        assert_eq!(0, line_to_byte_idx(text, 0));
        assert_eq!(6, line_to_byte_idx(text, 1));
        assert_eq!(11, line_to_byte_idx(text, 2));
        assert_eq!(17, line_to_byte_idx(text, 3));
    }

    #[test]
    fn line_to_byte_idx_02() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(0, line_to_byte_idx(text, 0));
        assert_eq!(1, line_to_byte_idx(text, 1));
        assert_eq!(6, line_to_byte_idx(text, 2));
        assert_eq!(10, line_to_byte_idx(text, 3));
        assert_eq!(15, line_to_byte_idx(text, 4));
        assert_eq!(21, line_to_byte_idx(text, 5));
    }

    #[test]
    fn line_to_byte_idx_03() {
        assert_eq!(0, line_to_byte_idx(TEXT_LINES, 0));
        assert_eq!(32, line_to_byte_idx(TEXT_LINES, 1));
        assert_eq!(59, line_to_byte_idx(TEXT_LINES, 2));
        assert_eq!(88, line_to_byte_idx(TEXT_LINES, 3));

        // Past end
        assert_eq!(124, line_to_byte_idx(TEXT_LINES, 4));
        assert_eq!(124, line_to_byte_idx(TEXT_LINES, 5));
        assert_eq!(124, line_to_byte_idx(TEXT_LINES, 6));
    }

    #[test]
    fn line_to_char_idx_01() {
        let text = "Hello せ\nか\nい!";
        assert_eq!(0, line_to_char_idx(text, 0));
        assert_eq!(8, line_to_char_idx(text, 1));
        assert_eq!(10, line_to_char_idx(text, 2));
    }

    #[test]
    fn line_to_char_idx_02() {
        assert_eq!(0, line_to_char_idx(TEXT_LINES, 0));
        assert_eq!(32, line_to_char_idx(TEXT_LINES, 1));
        assert_eq!(59, line_to_char_idx(TEXT_LINES, 2));
        assert_eq!(88, line_to_char_idx(TEXT_LINES, 3));

        // Past end
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 4));
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 5));
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 6));
    }

    #[test]
    fn line_byte_round_trip() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(6, line_to_byte_idx(text, byte_to_line_idx(text, 6)));
        assert_eq!(2, byte_to_line_idx(text, line_to_byte_idx(text, 2)));

        assert_eq!(0, line_to_byte_idx(text, byte_to_line_idx(text, 0)));
        assert_eq!(0, byte_to_line_idx(text, line_to_byte_idx(text, 0)));

        assert_eq!(21, line_to_byte_idx(text, byte_to_line_idx(text, 21)));
        assert_eq!(5, byte_to_line_idx(text, line_to_byte_idx(text, 5)));
    }

    #[test]
    fn line_char_round_trip() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(6, line_to_char_idx(text, char_to_line_idx(text, 6)));
        assert_eq!(2, char_to_line_idx(text, line_to_char_idx(text, 2)));

        assert_eq!(0, line_to_char_idx(text, char_to_line_idx(text, 0)));
        assert_eq!(0, char_to_line_idx(text, line_to_char_idx(text, 0)));

        assert_eq!(21, line_to_char_idx(text, char_to_line_idx(text, 21)));
        assert_eq!(5, char_to_line_idx(text, line_to_char_idx(text, 5)));
    }

    #[test]
    fn has_bytes_less_than_01() {
        let v = 0x0709080905090609;
        assert!(has_bytes_less_than(v, 0x0A));
        assert!(has_bytes_less_than(v, 0x06));
        assert!(!has_bytes_less_than(v, 0x05));
    }

    #[test]
    fn has_byte_01() {
        let v = 0x070908A60509E209;
        assert!(has_byte(v, 0x07));
        assert!(has_byte(v, 0x09));
        assert!(has_byte(v, 0x08));
        assert!(has_byte(v, 0xA6));
        assert!(has_byte(v, 0x05));
        assert!(has_byte(v, 0xE2));

        assert!(!has_byte(v, 0xA0));
        assert!(!has_byte(v, 0xA7));
        assert!(!has_byte(v, 0x06));
        assert!(!has_byte(v, 0xE3));
    }
}
