#![allow(dead_code)]

use std;

use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

/// Uses bit-fiddling magic to count utf8 chars really quickly.
/// We actually count the number of non-starting utf8 bytes, since
/// they have a consistent starting two-bit pattern.  We then
/// subtract from the byte length of the text to get the final
/// count.
pub fn count_chars(text: &str) -> usize {
    #[allow(overflowing_literals)]
    const ONEMASK: usize = 0x01010101010101010101010101010101;

    let tsize: usize = std::mem::size_of::<usize>();

    let len = text.len();
    let mut ptr = text.as_ptr();
    let end_ptr = unsafe { ptr.offset(len as isize) };
    let mut inv_count = 0;

    // Take care of any unaligned bytes at the beginning
    let end_pre_ptr = {
        let aligned = ptr as usize + (tsize - (ptr as usize & (tsize - 1)));
        (end_ptr as usize).min(aligned) as *const u8
    };
    while ptr < end_pre_ptr {
        let byte = unsafe { *ptr };
        let a = (byte >> 7) & (!byte >> 6);
        inv_count += a as usize;
        ptr = unsafe { ptr.offset(1) };
    }

    // Use usize to count multiple bytes at once, using bit-fiddling magic.
    let mut ptr = ptr as *const usize;
    let end_mid_ptr = (end_ptr as usize - (end_ptr as usize & (tsize - 1))) as *const usize;
    while ptr < end_mid_ptr {
        // Do the clever counting
        let n = unsafe { *ptr };
        let masked = ((n & (ONEMASK.wrapping_mul(0x80))) >> 7) & (!n >> 6);
        inv_count += (masked.wrapping_mul(ONEMASK)) >> ((tsize - 1) * 8);
        ptr = unsafe { ptr.offset(1) };
    }

    // Take care of any unaligned bytes at the end
    let mut ptr = ptr as *const u8;
    while ptr < end_ptr {
        let byte = unsafe { *ptr };
        let a = (byte >> 7) & (!byte >> 6);
        inv_count += a as usize;
        ptr = unsafe { ptr.offset(1) };
    }

    len - inv_count
}

pub fn byte_idx_to_char_idx(text: &str, byte_idx: usize) -> usize {
    let mut char_i = 0;
    for (offset, _) in text.char_indices() {
        if byte_idx < offset {
            break;
        } else {
            char_i += 1;
        }
    }
    if byte_idx == text.len() {
        char_i
    } else {
        char_i - 1
    }
}

pub fn byte_idx_to_line_idx(text: &str, byte_idx: usize) -> usize {
    let mut line_i = 1;
    for offset in LineBreakIter::new(text) {
        if byte_idx < offset {
            break;
        } else {
            line_i += 1;
        }
    }
    line_i - 1
}

pub fn char_idx_to_byte_idx(text: &str, char_idx: usize) -> usize {
    if let Some((offset, _)) = text.char_indices().nth(char_idx) {
        offset
    } else {
        text.len()
    }
}

pub fn char_idx_to_line_idx(text: &str, char_idx: usize) -> usize {
    byte_idx_to_line_idx(text, char_idx_to_byte_idx(text, char_idx))
}

pub fn line_idx_to_byte_idx(text: &str, line_idx: usize) -> usize {
    if line_idx == 0 {
        0
    } else {
        LineBreakIter::new(text).nth(line_idx - 1).unwrap()
    }
}

pub fn line_idx_to_char_idx(text: &str, line_idx: usize) -> usize {
    byte_idx_to_char_idx(text, line_idx_to_byte_idx(text, line_idx))
}

/// Returns whether the given byte boundary in the text is a grapheme cluster
/// boundary or not.
pub fn is_grapheme_boundary(text: &str, byte_idx: usize) -> bool {
    // Bounds check
    assert!(byte_idx <= text.len());

    if byte_idx == 0 || byte_idx == text.len() {
        // True if we're on the edge of the text
        true
    } else if !text.is_char_boundary(byte_idx) {
        // False if we're not even on a codepoint boundary
        false
    } else {
        // Full check
        GraphemeCursor::new(byte_idx, text.len(), true)
            .is_boundary(text, 0)
            .unwrap()
    }
}

/// Returns the grapheme cluster boundary before (but not including) the given
/// byte boundary.
///
/// This will return back the passed byte boundary if it is at the start of
/// the string.
pub fn prev_grapheme_boundary(text: &str, byte_idx: usize) -> usize {
    // Bounds check
    assert!(byte_idx <= text.len());

    // Early out
    if byte_idx == 0 {
        return byte_idx;
    }

    // Find codepoint boundary
    let mut boundary_idx = byte_idx;
    while !text.is_char_boundary(boundary_idx) {
        boundary_idx -= 1;
    }

    // Find the next grapheme cluster boundary
    let mut gc = GraphemeCursor::new(boundary_idx, text.len(), true);
    if boundary_idx < byte_idx && gc.is_boundary(text, 0).unwrap() {
        return boundary_idx;
    } else {
        return gc.prev_boundary(text, 0).unwrap().unwrap();
    }
}

/// Returns the grapheme cluster boundary after (but not including) the given
/// byte boundary.
///
/// This will return back the passed byte boundary if it is at the end of
/// the string.
pub fn next_grapheme_boundary(text: &str, byte_idx: usize) -> usize {
    // Bounds check
    assert!(byte_idx <= text.len());

    // Early out
    if byte_idx == text.len() {
        return byte_idx;
    }

    // Find codepoint boundary
    let mut boundary_idx = byte_idx;
    while !text.is_char_boundary(boundary_idx) {
        boundary_idx += 1;
    }

    // Find the next grapheme cluster boundary
    let mut gc = GraphemeCursor::new(boundary_idx, text.len(), true);
    if byte_idx < boundary_idx && gc.is_boundary(text, 0).unwrap() {
        return boundary_idx;
    } else {
        return gc.next_boundary(text, 0).unwrap().unwrap();
    }
}

/// Finds the nearest grapheme boundary near the given byte that is
/// not the left or right edge of the text.
///
/// There is only one circumstance where the left or right edge will
/// be returned: if the entire text is a single grapheme cluster,
/// then the right edge of the text is returned.
pub fn nearest_internal_grapheme_boundary(text: &str, byte_idx: usize) -> usize {
    // Bounds check
    assert!(byte_idx <= text.len());

    // Find codepoint boundary
    let mut boundary_idx = byte_idx;
    while !text.is_char_boundary(boundary_idx) {
        boundary_idx -= 1;
    }

    // Find the two nearest grapheme boundaries
    let mut gc = GraphemeCursor::new(boundary_idx, text.len(), true);
    let next = gc.next_boundary(text, 0).unwrap().unwrap_or(text.len());
    let prev = gc.prev_boundary(text, 0).unwrap().unwrap_or(0);

    // If the given byte was already on an internal grapheme boundary
    if prev == byte_idx && byte_idx != 0 {
        return byte_idx;
    }

    // Otherwise, return the closest of prev and next that isn't the
    // start or end of the string
    if prev == 0 {
        return next;
    } else if next == text.len() {
        return prev;
    } else if (byte_idx - prev) >= (next - byte_idx) {
        return next;
    } else {
        return prev;
    }
}

pub fn seam_is_grapheme_boundary(l: &str, r: &str) -> bool {
    assert!(l.len() > 0 && r.len() > 0);

    let tot_len = l.len() + r.len();
    let mut gc = GraphemeCursor::new(l.len(), tot_len, true);

    gc.next_boundary(r, l.len()).unwrap();
    let prev = {
        match gc.prev_boundary(r, l.len()) {
            Ok(pos) => pos,
            Err(GraphemeIncomplete::PrevChunk) => gc.prev_boundary(l, 0).unwrap(),
            _ => unreachable!(),
        }
    };

    if let Some(a) = prev {
        if a == l.len() {
            return true;
        }
    }

    return false;
}

//======================================================================

/// An iterator that yields the byte indices of line breaks in a string.
/// A line break in this case is the point immediately *after* a newline
/// character.
///
/// The following unicode sequences are considered newlines by this function:
/// - u{000A} (a.k.a. LF)
/// - u{000D} (a.k.a. CR)
/// - u{000D}u{000A} (a.k.a. CRLF)
/// - u{000B}
/// - u{000C}
/// - u{0085}
/// - u{2028}
/// - u{2029}
pub(crate) struct LineBreakIter<'a> {
    byte_itr: std::str::Bytes<'a>,
    byte_idx: usize,
}

impl<'a> LineBreakIter<'a> {
    pub fn new<'b>(text: &'b str) -> LineBreakIter<'b> {
        LineBreakIter {
            byte_itr: text.bytes(),
            byte_idx: 0,
        }
    }
}

impl<'a> Iterator for LineBreakIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        while let Some(byte) = self.byte_itr.next() {
            self.byte_idx += 1;
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
            if byte == 0xC2 {
                if let Some(0x85) = self.byte_itr.next() {
                    self.byte_idx += 1;
                    return Some(self.byte_idx);
                }
            }
            if byte == 0xE2 {
                self.byte_idx += 1;
                if let Some(0x80) = self.byte_itr.next() {
                    self.byte_idx += 1;
                    match self.byte_itr.next() {
                        Some(0xA8) | Some(0xA9) => {
                            return Some(self.byte_idx);
                        }
                        _ => {}
                    }
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

    #[test]
    fn count_chars_01() {
        let text = "Hello せかい! Hello せかい! Hello せかい! Hello せかい! Hello せかい!";

        assert_eq!(54, count_chars(text));
    }

    #[test]
    fn nearest_internal_grapheme_boundary_01() {
        let text = "Hello world!";
        assert_eq!(1, nearest_internal_grapheme_boundary(text, 0));
        assert_eq!(6, nearest_internal_grapheme_boundary(text, 6));
        assert_eq!(11, nearest_internal_grapheme_boundary(text, 12));
    }

    #[test]
    fn nearest_internal_grapheme_boundary_02() {
        let text = "Hello\r\n world!";
        assert_eq!(5, nearest_internal_grapheme_boundary(text, 5));
        assert_eq!(7, nearest_internal_grapheme_boundary(text, 6));
        assert_eq!(7, nearest_internal_grapheme_boundary(text, 7));
    }

    #[test]
    fn nearest_internal_grapheme_boundary_03() {
        let text = "\r\nHello world!\r\n";
        assert_eq!(2, nearest_internal_grapheme_boundary(text, 0));
        assert_eq!(2, nearest_internal_grapheme_boundary(text, 1));
        assert_eq!(2, nearest_internal_grapheme_boundary(text, 2));
        assert_eq!(14, nearest_internal_grapheme_boundary(text, 14));
        assert_eq!(14, nearest_internal_grapheme_boundary(text, 15));
        assert_eq!(14, nearest_internal_grapheme_boundary(text, 16));
    }

    #[test]
    fn nearest_internal_grapheme_boundary_04() {
        let text = "\r\n";
        assert_eq!(2, nearest_internal_grapheme_boundary(text, 0));
        assert_eq!(2, nearest_internal_grapheme_boundary(text, 1));
        assert_eq!(2, nearest_internal_grapheme_boundary(text, 2));
    }

    #[test]
    fn is_grapheme_boundary_01() {
        let text = "\n\r\n\r\n\r\n\r\n\r\n\r";

        assert!(is_grapheme_boundary(text, 0));
        assert!(is_grapheme_boundary(text, 12));
        assert!(is_grapheme_boundary(text, 3));
        assert!(!is_grapheme_boundary(text, 6));
    }

    #[test]
    fn seam_is_grapheme_boundary_01() {
        let text1 = "\r\n\r\n\r\n";
        let text2 = "\r\n\r\n";

        assert!(seam_is_grapheme_boundary(text1, text2));
    }

    #[test]
    fn seam_is_grapheme_boundary_02() {
        let text1 = "\r\n\r\n\r";
        let text2 = "\n\r\n\r\n";

        assert!(!seam_is_grapheme_boundary(text1, text2));
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
    fn byte_idx_to_char_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(8, byte_idx_to_char_idx(text, 12));
        assert_eq!(0, byte_idx_to_char_idx(text, 0));
        assert_eq!(10, byte_idx_to_char_idx(text, 16));
    }

    #[test]
    fn byte_idx_to_char_idx_02() {
        let text = "せかい";
        assert_eq!(0, byte_idx_to_char_idx(text, 0));
        assert_eq!(0, byte_idx_to_char_idx(text, 1));
        assert_eq!(0, byte_idx_to_char_idx(text, 2));
        assert_eq!(1, byte_idx_to_char_idx(text, 3));
        assert_eq!(1, byte_idx_to_char_idx(text, 4));
        assert_eq!(1, byte_idx_to_char_idx(text, 5));
        assert_eq!(2, byte_idx_to_char_idx(text, 6));
        assert_eq!(2, byte_idx_to_char_idx(text, 7));
        assert_eq!(2, byte_idx_to_char_idx(text, 8));
        assert_eq!(3, byte_idx_to_char_idx(text, 9));
    }

    #[test]
    fn byte_idx_to_line_idx_01() {
        let text = "Here\nare\nsome\nwords";
        assert_eq!(0, byte_idx_to_line_idx(text, 0));
        assert_eq!(0, byte_idx_to_line_idx(text, 4));
        assert_eq!(1, byte_idx_to_line_idx(text, 5));
        assert_eq!(1, byte_idx_to_line_idx(text, 8));
        assert_eq!(2, byte_idx_to_line_idx(text, 9));
        assert_eq!(2, byte_idx_to_line_idx(text, 13));
        assert_eq!(3, byte_idx_to_line_idx(text, 14));
        assert_eq!(3, byte_idx_to_line_idx(text, 19));
    }

    #[test]
    fn byte_idx_to_line_idx_02() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(0, byte_idx_to_line_idx(text, 0));
        assert_eq!(1, byte_idx_to_line_idx(text, 1));
        assert_eq!(1, byte_idx_to_line_idx(text, 5));
        assert_eq!(2, byte_idx_to_line_idx(text, 6));
        assert_eq!(2, byte_idx_to_line_idx(text, 9));
        assert_eq!(3, byte_idx_to_line_idx(text, 10));
        assert_eq!(3, byte_idx_to_line_idx(text, 14));
        assert_eq!(4, byte_idx_to_line_idx(text, 15));
        assert_eq!(4, byte_idx_to_line_idx(text, 20));
        assert_eq!(5, byte_idx_to_line_idx(text, 21));
    }

    #[test]
    fn byte_idx_to_line_idx_03() {
        let text = "Here\r\nare\r\nsome\r\nwords";
        assert_eq!(0, byte_idx_to_line_idx(text, 0));
        assert_eq!(0, byte_idx_to_line_idx(text, 4));
        assert_eq!(0, byte_idx_to_line_idx(text, 5));
        assert_eq!(1, byte_idx_to_line_idx(text, 6));
        assert_eq!(1, byte_idx_to_line_idx(text, 9));
        assert_eq!(1, byte_idx_to_line_idx(text, 10));
        assert_eq!(2, byte_idx_to_line_idx(text, 11));
        assert_eq!(2, byte_idx_to_line_idx(text, 15));
        assert_eq!(2, byte_idx_to_line_idx(text, 16));
        assert_eq!(3, byte_idx_to_line_idx(text, 17));
    }

    #[test]
    fn char_idx_to_byte_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(12, char_idx_to_byte_idx(text, 8));
        assert_eq!(0, char_idx_to_byte_idx(text, 0));
        assert_eq!(16, char_idx_to_byte_idx(text, 10));
    }

    #[test]
    fn char_idx_to_line_idx_01() {
        let text = "Hello せ\nか\nい!";
        assert_eq!(0, char_idx_to_line_idx(text, 0));
        assert_eq!(0, char_idx_to_line_idx(text, 7));
        assert_eq!(1, char_idx_to_line_idx(text, 8));
        assert_eq!(1, char_idx_to_line_idx(text, 9));
        assert_eq!(2, char_idx_to_line_idx(text, 10));
    }

    #[test]
    fn line_idx_to_byte_idx_01() {
        let text = "Here\r\nare\r\nsome\r\nwords";
        assert_eq!(0, line_idx_to_byte_idx(text, 0));
        assert_eq!(6, line_idx_to_byte_idx(text, 1));
        assert_eq!(11, line_idx_to_byte_idx(text, 2));
        assert_eq!(17, line_idx_to_byte_idx(text, 3));
    }

    #[test]
    fn line_idx_to_byte_idx_02() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(0, line_idx_to_byte_idx(text, 0));
        assert_eq!(1, line_idx_to_byte_idx(text, 1));
        assert_eq!(6, line_idx_to_byte_idx(text, 2));
        assert_eq!(10, line_idx_to_byte_idx(text, 3));
        assert_eq!(15, line_idx_to_byte_idx(text, 4));
        assert_eq!(21, line_idx_to_byte_idx(text, 5));
    }

    #[test]
    fn line_idx_to_char_idx_01() {
        let text = "Hello せ\nか\nい!";
        assert_eq!(0, line_idx_to_char_idx(text, 0));
        assert_eq!(8, line_idx_to_char_idx(text, 1));
        assert_eq!(10, line_idx_to_char_idx(text, 2));
    }

    #[test]
    fn line_byte_round_trip() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(6, line_idx_to_byte_idx(text, byte_idx_to_line_idx(text, 6)));
        assert_eq!(2, byte_idx_to_line_idx(text, line_idx_to_byte_idx(text, 2)));

        assert_eq!(0, line_idx_to_byte_idx(text, byte_idx_to_line_idx(text, 0)));
        assert_eq!(0, byte_idx_to_line_idx(text, line_idx_to_byte_idx(text, 0)));

        assert_eq!(
            21,
            line_idx_to_byte_idx(text, byte_idx_to_line_idx(text, 21))
        );
        assert_eq!(5, byte_idx_to_line_idx(text, line_idx_to_byte_idx(text, 5)));
    }

    #[test]
    fn line_char_round_trip() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(6, line_idx_to_char_idx(text, char_idx_to_line_idx(text, 6)));
        assert_eq!(2, char_idx_to_line_idx(text, line_idx_to_char_idx(text, 2)));

        assert_eq!(0, line_idx_to_char_idx(text, char_idx_to_line_idx(text, 0)));
        assert_eq!(0, char_idx_to_line_idx(text, line_idx_to_char_idx(text, 0)));

        assert_eq!(
            21,
            line_idx_to_char_idx(text, char_idx_to_line_idx(text, 21))
        );
        assert_eq!(5, char_idx_to_line_idx(text, line_idx_to_char_idx(text, 5)));
    }
}
