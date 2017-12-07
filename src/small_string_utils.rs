#![allow(dead_code)]

use std;

use smallvec::Array;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use small_string::SmallString;


pub fn char_pos_to_byte_pos(text: &str, pos: usize) -> usize {
    if let Some((offset, _)) = text.char_indices().nth(pos) {
        offset
    } else {
        text.len()
    }
}


/// Inserts the given text into the given string at the given char index.
pub fn insert_at_char<B: Array<Item = u8>>(s: &mut SmallString<B>, text: &str, pos: usize) {
    let byte_pos = char_pos_to_byte_pos(s, pos);
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
    let byte_pos_a = char_pos_to_byte_pos(&s[..], pos_a);
    let byte_pos_b = char_pos_to_byte_pos(&s[..], pos_b);

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
    let split_pos = char_pos_to_byte_pos(&s1[..], pos);
    s1.split_off(split_pos)
}


/// Splits a string at the byte index given, or if the byte given is not at a
/// grapheme boundary, then at the closest grapheme boundary that isn't the
/// start or end of the string.  The left part of the split remains in the
/// given string, and the right part is returned as a new string.
///
/// Note that because this only splits at grapheme boundaries, it is not
/// guaranteed to split at the exact byte given.  Indeed, depending on the
/// string and byte index given, it may not split at all.  Be aware of this!
pub fn split_string_near_byte<B: Array<Item = u8>>(
    s: &mut SmallString<B>,
    pos: usize,
) -> SmallString<B> {
    // Handle some corner-cases ahead of time, to simplify the logic below
    if pos == s.len() || s.len() == 0 {
        return SmallString::new();
    }
    if pos == 0 {
        return s.split_off(0);
    }

    // Find codepoint boundary
    let mut split_pos = pos;
    while !s.is_char_boundary(split_pos) {
        split_pos -= 1;
    }

    // Find the two nearest grapheme boundaries
    let mut gc = GraphemeCursor::new(split_pos, s.len(), true);
    let next = gc.next_boundary(s, 0).unwrap().unwrap_or(s.len());
    let prev = gc.prev_boundary(s, 0).unwrap().unwrap_or(0);

    // Check if the specified split position is on a boundary, and split
    // there if it is
    if prev == pos {
        return s.split_off(split_pos);
    }

    // Otherwise, split on the closest of prev and next that isn't the
    // start or end of the string
    if prev == 0 {
        return s.split_off(next);
    } else if next == s.len() {
        return s.split_off(prev);
    } else if (pos - prev) >= (next - pos) {
        return s.split_off(next);
    } else {
        return s.split_off(prev);
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
/// - u{000A} (LF)
/// - u{000B}
/// - u{000C}
/// - u{000D} (CR)
/// - u{000D}u{000A} (CRLF)
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
                0x0A | 0x0B | 0x0C | 0x85 => {
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
    use small_string::SmallString;
    use rope::BackingArray;
    use super::*;

    #[test]
    fn split_near_byte_01() {
        let mut s1 = SmallString::<BackingArray>::from_str("Hello world!");
        let s2 = split_string_near_byte(&mut s1, 0);
        assert_eq!("", s1);
        assert_eq!("Hello world!", s2);
    }

    #[test]
    fn split_near_byte_02() {
        let mut s1 = SmallString::<BackingArray>::from_str("Hello world!");
        let s2 = split_string_near_byte(&mut s1, 12);
        assert_eq!("Hello world!", s1);
        assert_eq!("", s2);
    }

    #[test]
    fn split_near_byte_03() {
        let mut s1 = SmallString::<BackingArray>::from_str("Hello world!");
        let s2 = split_string_near_byte(&mut s1, 6);
        assert_eq!("Hello ", s1);
        assert_eq!("world!", s2);
    }

    #[test]
    fn split_near_byte_04() {
        let mut s1 = SmallString::<BackingArray>::from_str("Hello\r\n world!");
        let s2 = split_string_near_byte(&mut s1, 5);
        assert_eq!("Hello", s1);
        assert_eq!("\r\n world!", s2);
    }

    #[test]
    fn split_near_byte_05() {
        let mut s1 = SmallString::<BackingArray>::from_str("Hello\r\n world!");
        let s2 = split_string_near_byte(&mut s1, 6);
        assert_eq!("Hello\r\n", s1);
        assert_eq!(" world!", s2);
    }

    #[test]
    fn split_near_byte_06() {
        let mut s1 = SmallString::<BackingArray>::from_str("Hello\r\n world!");
        let s2 = split_string_near_byte(&mut s1, 7);
        assert_eq!("Hello\r\n", s1);
        assert_eq!(" world!", s2);
    }

    #[test]
    fn split_near_byte_07() {
        let mut s1 = SmallString::<BackingArray>::from_str("Hello world!\r\n");
        let s2 = split_string_near_byte(&mut s1, 13);
        assert_eq!("Hello world!", s1);
        assert_eq!("\r\n", s2);
    }

    #[test]
    fn split_near_byte_08() {
        let mut s1 = SmallString::<BackingArray>::from_str("\r\n");
        let s2 = split_string_near_byte(&mut s1, 1);
        assert_eq!("\r\n", s1);
        assert_eq!("", s2);
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
}
