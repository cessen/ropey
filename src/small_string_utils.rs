#![allow(dead_code)]

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

pub fn char_count(text: &str) -> usize {
    let mut count = 0;
    for byte in text.bytes() {
        if (byte & 0xC0) != 0x80 {
            count += 1;
        }
    }
    count
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


/// Splits a string at the byte index given, or the at the nearest codepoint
/// boundary after it if it is not a codepoint boundary itself.  The left part
/// of the split remains in the given string, and the right part is returned as
/// a new string.
///
/// Note that because this only splits at code points, it is not guaranteed to
/// split at the exact byte given.  Indeed, depending on the string and byte
/// index given, it may not split at all.  Be aware of this!
///
/// TODO: make this only split on grapheme boundaries, too.
pub fn split_string_near_byte<B: Array<Item = u8>>(
    s1: &mut SmallString<B>,
    pos: usize,
) -> SmallString<B> {
    let mut split_pos = pos;
    while !s1.is_char_boundary(split_pos) {
        split_pos += 1;
    }

    s1.split_off(split_pos)
}
