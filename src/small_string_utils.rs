#![allow(dead_code)]

use unicode_segmentation::GraphemeCursor;

use smallvec::Array;
use small_string::SmallString;


pub fn is_line_ending(text: &str) -> bool {
    match text {
        "\u{000D}\u{000A}" |
        "\u{000A}" |
        "\u{000B}" |
        "\u{000C}" |
        "\u{000D}" |
        "\u{0085}" |
        "\u{2028}" |
        "\u{2029}" => true,
        _ => false,
    }
}


pub fn newline_count(text: &str) -> usize {
    let mut count = 0;
    let mut last_was_d = false;
    let mut itr = text.bytes();
    while let Some(byte) = itr.next() {
        match byte {
            0x0B | 0x0C | 0x85 => {
                count += 1;
            }
            0x0D => {
                count += 1;
                last_was_d = true;
                continue;
            }
            0x0A => {
                if !last_was_d {
                    count += 1;
                }
            }
            0x20 => {
                match itr.next() {
                    Some(0x28) => count += 1,
                    Some(0x29) => count += 1,
                    _ => {}
                }
            }
            _ => {}
        }
        last_was_d = false;
    }
    count
}


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
        split_pos += 1;
    }

    // Check if it's a grapheme boundary, and split there if it is
    if let Ok(true) = GraphemeCursor::new(split_pos, s.len(), true).is_boundary(s, 0) {
        return s.split_off(split_pos);
    }

    // Otherwise, find prev and next grapheme boundaries
    let next = GraphemeCursor::new(split_pos, s.len(), true)
        .next_boundary(s, 0)
        .unwrap_or(None)
        .unwrap_or(s.len());
    let prev = GraphemeCursor::new(split_pos, s.len(), true)
        .prev_boundary(s, 0)
        .unwrap_or(None)
        .unwrap_or(0);

    // Split on the closest of prev and next that isn't the start or
    // end of the string
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
}
