#![allow(dead_code)]

use std;

use smallvec::Array;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use small_string::SmallString;


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


/// Inserts the given text into the given string at the given char index.
pub fn insert_at_char<B: Array<Item = u8>>(s: &mut SmallString<B>, text: &str, pos: usize) {
    let byte_pos = char_idx_to_byte_idx(s, pos);
    s.insert_str(byte_pos, text);
}


/// Removes the text between the given char indices in the given string.
pub fn remove_text_between_char_indices<B: Array<Item = u8>>(
    s: &mut SmallString<B>,
    pos_a: usize,
    pos_b: usize,
) {
    // Bounds checks
    assert!(
        pos_a <= pos_b,
        "remove_text_between_char_indices(): pos_a must be less than or equal to pos_b."
    );

    if pos_a == pos_b {
        return;
    }

    // Find removal positions in bytes
    // TODO: get both of these in a single pass
    let byte_pos_a = char_idx_to_byte_idx(&s[..], pos_a);
    let byte_pos_b = char_idx_to_byte_idx(&s[..], pos_b);

    // Get byte vec of string
    let byte_vec = unsafe { s.as_mut_smallvec() };

    // Move bytes to fill in the gap left by the removed bytes
    let mut from = byte_pos_b;
    let mut to = byte_pos_a;
    while from < byte_vec.len() {
        byte_vec[to] = byte_vec[from];

        from += 1;
        to += 1;
    }

    // Remove data from the end
    let final_text_size = byte_vec.len() + byte_pos_a - byte_pos_b;
    byte_vec.truncate(final_text_size);
}


/// Splits a string into two strings at the char index given.
/// The first section of the split is stored in the original string,
/// while the second section of the split is returned as a new string.
pub fn split_string_at_char<B: Array<Item = u8>>(
    s1: &mut SmallString<B>,
    pos: usize,
) -> SmallString<B> {
    let split_pos = char_idx_to_byte_idx(&s1[..], pos);
    s1.split_off(split_pos)
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

/// Takes two SmallStrings and mends the grapheme boundary between them, if any.
///
/// Note: this will leave one of the strings empty if the entire composite string
/// is one big grapheme.
pub fn fix_grapheme_seam<B: Array<Item = u8>>(l: &mut SmallString<B>, r: &mut SmallString<B>) {
    let tot_len = l.len() + r.len();
    let mut gc = GraphemeCursor::new(l.len(), tot_len, true);
    let next = gc.next_boundary(r, l.len()).unwrap();
    let prev = {
        match gc.prev_boundary(r, l.len()) {
            Ok(pos) => pos,
            Err(GraphemeIncomplete::PrevChunk) => gc.prev_boundary(l, 0).unwrap(),
            _ => unreachable!(),
        }
    };

    // Find the new split position, if any.
    let new_split_pos = if let (Some(a), Some(b)) = (prev, next) {
        if a == l.len() {
            // We're on a graphem boundary, don't need to do anything
            return;
        }
        if a == 0 {
            b
        } else if b == tot_len {
            a
        } else if l.len() > r.len() {
            a
        } else {
            b
        }
    } else if let Some(a) = prev {
        if a == l.len() {
            return;
        }
        a
    } else if let Some(b) = next {
        b
    } else {
        unreachable!()
    };

    // Move the bytes to create the new split
    if new_split_pos < l.len() {
        r.insert_str(0, &l[new_split_pos..]);
        l.truncate(new_split_pos);
    } else {
        let pos = new_split_pos - l.len();
        l.push_str(&r[..pos]);
        r.truncate_front(pos);
    }
}

//----------------------------------------------------------------------

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
            match byte {
                0x0A | 0x0B | 0x0C => {
                    return Some(self.byte_idx);
                }
                0x0D => {
                    // We're basically "peeking" here.
                    if let Some(0x0A) = self.byte_itr.clone().next() {
                        self.byte_itr.next();
                        self.byte_idx += 1;
                    }
                    return Some(self.byte_idx);
                }
                0xC2 => {
                    if let Some(0x85) = self.byte_itr.next() {
                        self.byte_idx += 1;
                        return Some(self.byte_idx);
                    }
                }
                0xE2 => {
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
                _ => {}
            }
        }

        return None;
    }
}


//----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
