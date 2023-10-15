#![cfg(not(miri))]

#[macro_use]
extern crate proptest;
extern crate ropey;

use proptest::collection::vec;
use proptest::test_runner::Config;
use ropey::{
    str_utils::{byte_to_char_idx, byte_to_line_idx, char_to_byte_idx, char_to_line_idx},
    Rope, MAX_BYTES,
};

fn string_insert(text: &mut String, char_idx: usize, text_ins: &str) {
    let byte_idx = char_to_byte_idx(text, char_idx);
    text.insert_str(byte_idx, text_ins);
}

fn string_remove(text: &mut String, char_start: usize, char_end: usize) {
    let byte_start = char_to_byte_idx(text, char_start);
    let byte_end = char_to_byte_idx(text, char_end);
    let text_r = text.split_off(byte_end);
    text.truncate(byte_start);
    text.push_str(&text_r);
}

fn string_slice(text: &str, char_start: usize, char_end: usize) -> &str {
    let byte_start = char_to_byte_idx(text, char_start);
    let text = &text[byte_start..];
    let byte_end = char_to_byte_idx(text, char_end - char_start);
    &text[..byte_end]
}

//===========================================================================

proptest! {
    #![proptest_config(Config::with_cases(512))]

    #[test]
    fn pt_from_str(ref text in "\\PC{0,200}") {
        let rope = Rope::from_str(text);

        rope.assert_integrity();
        rope.assert_invariants();

        assert_eq!(rope, text.as_str());
    }

    #[test]
    fn pt_from_str_crlf(ref text in "[\\u{000A}\\u{000D}]{0,200}") {
        let rope = Rope::from_str(text);

        rope.assert_integrity();
        rope.assert_invariants();

        assert_eq!(rope, text.as_str());
    }

    #[test]
    fn pt_insert(char_idx in 0usize..(CHAR_LEN+1), ref ins_text in "\\PC*") {
        let mut rope = Rope::from_str(TEXT);
        let mut text = String::from(TEXT);

        let len = rope.len_chars();
        rope.insert(char_idx % (len + 1), ins_text);
        string_insert(&mut text, char_idx % (len + 1), ins_text);

        rope.assert_integrity();
        rope.assert_invariants();

        assert_eq!(rope, text);
    }

    #[test]
    fn pt_remove(range in (0usize..(CHAR_LEN+1), 0usize..(CHAR_LEN+1))) {
        let mut rope = Rope::from_str(TEXT);
        let mut text = String::from(TEXT);

        let mut idx1 = range.0 % (rope.len_chars() + 1);
        let mut idx2 = range.1 % (rope.len_chars() + 1);
        if idx1 > idx2 {
            std::mem::swap(&mut idx1, &mut idx2)
        };

        rope.remove(idx1..idx2);
        string_remove(&mut text, idx1, idx2);

        rope.assert_integrity();
        rope.assert_invariants();

        assert_eq!(rope, text);
    }

    #[test]
    fn pt_split_off_and_append(mut idx in 0usize..(CHAR_LEN+1)) {
        let mut rope = Rope::from_str(TEXT);

        idx %= rope.len_chars() + 1;

        let rope2 = rope.split_off(idx);

        rope.assert_integrity();
        rope.assert_invariants();
        rope2.assert_integrity();
        rope2.assert_invariants();

        rope.append(rope2);

        rope.assert_integrity();
        rope.assert_invariants();

        assert_eq!(rope, TEXT);
    }

    #[test]
    fn pt_shrink_to_fit_01(ref char_idxs in vec(0usize..1000000, 0..1000)) {
        let mut rope = Rope::new();

        for idx in char_idxs.iter() {
            let len = rope.len_chars();
            rope.insert(idx % (len + 1), "Hello world!")
        }

        let capacity_before = rope.capacity();
        let rope_clone = rope.clone();

        rope.shrink_to_fit();

        rope.assert_integrity();
        rope.assert_invariants();
        assert_eq!(rope, rope_clone);

        assert!((rope.capacity() - rope.len_bytes()) <= MAX_BYTES);
        assert!(rope.capacity() <= capacity_before);
    }

    #[test]
    fn pt_shrink_to_fit_02(ref char_idxs in vec(0usize..1000000, 0..1000)) {
        let mut rope = Rope::new();
        let ins_text = "AT̴̷͚͖̜͈̪͎͔̝̫̦̹͔̻̮͂ͬͬ̌ͣ̿ͤ͌ͥ͑̀̂ͬ̚͘͜͞ô̵͚̤̯̹͖̅̌̈́̑̏̕͘͝A";

        for idx in char_idxs.iter() {
            let len = rope.len_chars();
            rope.insert(idx % (len + 1), ins_text);
        }

        let rope_clone = rope.clone();

        rope.shrink_to_fit();

        rope.assert_integrity();
        rope.assert_invariants();
        assert_eq!(rope, rope_clone);

        let max_diff = MAX_BYTES + ((rope.len_bytes() / MAX_BYTES) * ins_text.len());

        assert!((rope.capacity() - rope.len_bytes()) <= max_diff);
    }

    #[test]
    fn pt_chunk_at_byte(ref text in "\\PC*\\n?\\PC*\\n?\\PC*") {
        let r = Rope::from_str(text);
        let mut t = &text[..];

        let mut last_chunk = "";
        for i in 0..r.len_bytes() {
            let (chunk, b, c, l) = r.chunk_at_byte(i);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
            if chunk != last_chunk {
                assert_eq!(chunk, &t[..chunk.len()]);
                t = &t[chunk.len()..];
                last_chunk = chunk;
            }

            let c1 = {
                let i2 = byte_to_char_idx(text, i);
                text.chars().nth(i2).unwrap()
            };
            let c2 = {
                let i2 = i - b;
                let i3 = byte_to_char_idx(chunk, i2);
                chunk.chars().nth(i3).unwrap()
            };
            assert_eq!(c1, c2);
        }
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn pt_chunk_at_char(ref text in "\\PC*\\n?\\PC*\\n?\\PC*") {
        let r = Rope::from_str(text);
        let mut t = &text[..];

        let mut last_chunk = "";
        for i in 0..r.len_chars() {
            let (chunk, b, c, l) = r.chunk_at_char(i);
            assert_eq!(b, char_to_byte_idx(text, c));
            assert_eq!(l, char_to_line_idx(text, c));
            if chunk != last_chunk {
                assert_eq!(chunk, &t[..chunk.len()]);
                t = &t[chunk.len()..];
                last_chunk = chunk;
            }

            let c1 = text.chars().nth(i).unwrap();
            let c2 = {
                let i2 = i - c;
                chunk.chars().nth(i2).unwrap()
            };
            assert_eq!(c1, c2);
        }
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn pt_chunk_at_line_break(ref text in "\\PC*\\n?\\PC*\\n?\\PC*") {
        let r = Rope::from_str(text);

        // First chunk
        {
            let (chunk, b, c, l) = r.chunk_at_line_break(0);
            assert_eq!(chunk, &text[..chunk.len()]);
            assert_eq!(b, 0);
            assert_eq!(c, 0);
            assert_eq!(l, 0);
        }

        // Middle chunks
        for i in 1..r.len_lines() {
            let (chunk, b, c, l) = r.chunk_at_line_break(i);
            assert_eq!(chunk, &text[b..(b + chunk.len())]);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
            assert!(l < i);
            assert!(i <= byte_to_line_idx(text, b + chunk.len()));
        }

        // Last chunk
        {
            let (chunk, b, c, l) = r.chunk_at_line_break(r.len_lines());
            assert_eq!(chunk, &text[(text.len() - chunk.len())..]);
            assert_eq!(chunk, &text[b..]);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
        }
    }

    #[test]
    fn pt_chunk_at_byte_slice(ref gen_text in "\\PC*\\n?\\PC*\\n?\\PC*", range in (0usize..1000000, 0usize..1000000)) {
        let r = Rope::from_str(gen_text);
        let mut idx1 = range.0 % (r.len_chars() + 1);
        let mut idx2 = range.1 % (r.len_chars() + 1);
        if idx1 > idx2 {
            std::mem::swap(&mut idx1, &mut idx2)
        };
        let s = r.slice(idx1..idx2);
        let text = string_slice(gen_text, idx1, idx2);

        let mut t = text;
        let mut prev_chunk = "";
        for i in 0..s.len_bytes() {
            let (chunk, b, c, l) = s.chunk_at_byte(i);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
            if chunk != prev_chunk {
                assert_eq!(chunk, &t[..chunk.len()]);
                t = &t[chunk.len()..];
                prev_chunk = chunk;
            }

            let c1 = {
                let i2 = byte_to_char_idx(text, i);
                text.chars().nth(i2).unwrap()
            };
            let c2 = {
                let i2 = i - b;
                let i3 = byte_to_char_idx(chunk, i2);
                chunk.chars().nth(i3).unwrap()
            };
            assert_eq!(c1, c2);
        }

        assert_eq!(t.len(), 0);
    }

    #[test]
    fn pt_chunk_at_char_slice(ref gen_text in "\\PC*\\n?\\PC*\\n?\\PC*", range in (0usize..1000000, 0usize..1000000)) {
        let r = Rope::from_str(gen_text);
        let mut idx1 = range.0 % (r.len_chars() + 1);
        let mut idx2 = range.1 % (r.len_chars() + 1);
        if idx1 > idx2 {
            std::mem::swap(&mut idx1, &mut idx2)
        };
        let s = r.slice(idx1..idx2);
        let text = string_slice(gen_text, idx1, idx2);

        let mut t = text;
        let mut prev_chunk = "";
        for i in 0..s.len_chars() {
            let (chunk, b, c, l) = s.chunk_at_char(i);
            assert_eq!(b, char_to_byte_idx(text, c));
            assert_eq!(l, char_to_line_idx(text, c));
            if chunk != prev_chunk {
                assert_eq!(chunk, &t[..chunk.len()]);
                t = &t[chunk.len()..];
                prev_chunk = chunk;
            }

            let c1 = text.chars().nth(i).unwrap();
            let c2 = {
                let i2 = i - c;
                chunk.chars().nth(i2).unwrap()
            };
            assert_eq!(c1, c2);
        }
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn pt_chunk_at_line_break_slice(ref gen_text in "\\PC*\\n?\\PC*\\n?\\PC*", range in (0usize..1000000, 0usize..1000000)) {
        let r = Rope::from_str(gen_text);
        let mut idx1 = range.0 % (r.len_chars() + 1);
        let mut idx2 = range.1 % (r.len_chars() + 1);
        if idx1 > idx2 {
            std::mem::swap(&mut idx1, &mut idx2)
        };
        let s = r.slice(idx1..idx2);
        let text = string_slice(gen_text, idx1, idx2);

        // First chunk
        {
            let (chunk, b, c, l) = s.chunk_at_line_break(0);
            assert_eq!(chunk, &text[..chunk.len()]);
            assert_eq!(b, 0);
            assert_eq!(c, 0);
            assert_eq!(l, 0);
        }

        // Middle chunks
        for i in 1..s.len_lines() {
            let (chunk, b, c, l) = s.chunk_at_line_break(i);
            assert_eq!(chunk, &text[b..(b + chunk.len())]);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
            assert!(l < i);
            assert!(i <= byte_to_line_idx(text, b + chunk.len()));
        }

        // Last chunk
        {
            let (chunk, b, c, l) = s.chunk_at_line_break(s.len_lines());
            assert_eq!(chunk, &text[(text.len() - chunk.len())..]);
            assert_eq!(chunk, &text[b..]);
            assert_eq!(c, byte_to_char_idx(text, b));
            assert_eq!(l, byte_to_line_idx(text, b));
        }
    }

    #[test]
    fn pt_slice(ref text in "\\PC*", range in (0usize..1000000, 0usize..1000000)) {
        let rope = Rope::from_str(text);

        let mut idx1 = range.0 % (rope.len_chars() + 1);
        let mut idx2 = range.1 % (rope.len_chars() + 1);
        if idx1 > idx2 {
            std::mem::swap(&mut idx1, &mut idx2)
        };

        let slice = rope.slice(idx1..idx2);
        let text_slice = string_slice(text, idx1, idx2);

        assert_eq!(slice, text_slice);
        assert_eq!(slice.len_bytes(), text_slice.len());
        assert_eq!(slice.len_chars(), text_slice.chars().count());
    }

    #[test]
    fn pt_get_byte_slice(ref text in "\\PC*", range in (0usize..1000000, 0usize..1000000)) {
        let rope = Rope::from_str(text);

        let mut idx1 = range.0 % (rope.len_bytes() + 1);
        let mut idx2 = range.1 % (rope.len_bytes() + 1);
        if idx1 > idx2 {
            std::mem::swap(&mut idx1, &mut idx2)
        };

        if let Some(slice) = rope.get_byte_slice(idx1..idx2) {
            let text_slice = &text[idx1..idx2];

            assert_eq!(slice, text_slice);
            assert_eq!(slice.len_bytes(), text_slice.len());
            assert_eq!(slice.len_chars(), text_slice.chars().count());
        }
    }

    #[test]
    fn pt_cmp(ref text1 in "\\PC*", ref text2 in "\\PC*") {
        let r1 = Rope::from_str(text1);
        let r2 = Rope::from_str(text2);

        assert_eq!(r1.cmp(&r2), text1.cmp(text2));
        assert_eq!(r2.cmp(&r1), text2.cmp(text1));
    }

    #[test]
    fn pt_bytes_iter_next(ref text in
        "\\PC{0,200}",
        idx1 in 0usize..20000, idx2 in 0usize..20000,
    ) {
        let len_chars = byte_to_char_idx(text, text.len());
        let idx1 = if len_chars == 0 { 0 } else { idx1 % len_chars };
        let idx2 = if len_chars == 0 { 0 } else { idx2 % len_chars };
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);


        let r = Rope::from_str(text);
        let text = string_slice(text, start, end);
        let s = r.slice(start..end);

        for (idx, byte) in s.bytes().enumerate() {
            assert_eq!(byte, text.as_bytes()[idx]);
        }
    }

    #[test]
    fn pt_bytes_iter_prev(
        ref directions in vec(0u8..2, 0..1000),
        idx1 in 0usize..CHAR_LEN,
        idx2 in 0usize..CHAR_LEN,
    ) {
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);

        let r = Rope::from_str(TEXT);
        let s = r.slice(start..end);

        let mut itr = s.bytes();
        let mut bytes = Vec::new();
        for i in directions {
            if *i == 0 {
                assert_eq!(itr.prev(), bytes.pop());
            } else if let Some(byte) = itr.next() {
                bytes.push(byte);
            }
        }
    }

    #[test]
    fn pt_chars_iter_next(ref text in
        "\\PC{0,200}",
        idx1 in 0usize..20000, idx2 in 0usize..20000,
    ) {
        let len_chars = byte_to_char_idx(text, text.len());
        let idx1 = if len_chars == 0 { 0 } else { idx1 % len_chars };
        let idx2 = if len_chars == 0 { 0 } else { idx2 % len_chars };
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);


        let r = Rope::from_str(text);
        let text = string_slice(text, start, end);
        let s = r.slice(start..end);

        for (c1, c2) in s.chars().zip(text.chars()) {
            assert_eq!(c1, c2);
        }
    }

    #[test]
    fn pt_chars_iter_prev(
        ref directions in vec(0u8..2, 0..1000),
        idx1 in 0usize..CHAR_LEN,
        idx2 in 0usize..CHAR_LEN,
    ) {
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);

        let r = Rope::from_str(TEXT);
        let s = r.slice(start..end);

        let mut itr = s.chars();
        let mut chars = Vec::new();
        for i in directions {
            if *i == 0 {
                assert_eq!(itr.prev(), chars.pop());
            } else if let Some(c) = itr.next() {
                chars.push(c);
            }
        }
    }

    #[test]
    fn pt_chunks_iter_next_01(ref text in
        "\\PC{0,200}",
        idx1 in 0usize..20000, idx2 in 0usize..20000,
    ) {
        let len_chars = byte_to_char_idx(text, text.len());
        let idx1 = if len_chars == 0 { 0 } else { idx1 % len_chars };
        let idx2 = if len_chars == 0 { 0 } else { idx2 % len_chars };
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);


        let r = Rope::from_str(text);
        let text = string_slice(text, start, end);
        let s = r.slice(start..end);

        let mut idx = 0;
        for chunk in s.chunks() {
            assert_eq!(chunk, &text[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }
    }

    #[test]
    fn pt_chunks_iter_next_02(idx1 in 0usize..CHAR_LEN, idx2 in 0usize..CHAR_LEN) {
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);

        let r = Rope::from_str(TEXT);
        let text = string_slice(TEXT, start, end);
        let s = r.slice(start..end);

        let mut idx = 0;
        for chunk in s.chunks() {
            assert_eq!(chunk, &text[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }
    }

    #[test]
    fn pt_chunks_iter_prev_01(ref text in
        "\\PC{0,200}",
        ref directions in vec(0u8..2, 0..1000),
        idx1 in 0usize..20000, idx2 in 0usize..20000,
    ) {

        let r = Rope::from_str(text);

        let idx1 = if r.len_chars() == 0 { 0 } else { idx1 % r.len_chars() };
        let idx2 = if r.len_chars() == 0 { 0 } else { idx2 % r.len_chars() };
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);

        let s = r.slice(start..end);

        let mut itr = s.chunks();
        let mut chunks = Vec::new();
        for i in directions {
            if *i == 0 {
                assert_eq!(itr.prev(), chunks.pop());
            } else if let Some(chunk) = itr.next() {
                chunks.push(chunk);
            }
        }
    }

    #[test]
    fn pt_chunks_iter_prev_02(
        ref directions in vec(0u8..2, 0..1000),
        idx1 in 0usize..CHAR_LEN,
        idx2 in 0usize..CHAR_LEN,
    ) {
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);

        let r = Rope::from_str(TEXT);
        let s = r.slice(start..end);

        let mut itr = s.chunks();
        let mut chunks = Vec::new();
        for i in directions {
            if *i == 0 {
                assert_eq!(itr.prev(), chunks.pop());
            } else if let Some(chunk) = itr.next() {
                chunks.push(chunk);
            }
        }
    }

    #[test]
    fn pt_lines_iter_01(ref text in
        "\n{0,2}\\PC{0,200}\n{0,2}\\PC{0,10}\n{0,2}\\PC{0,200}\n{0,2}",
        idx1 in 0usize..CHAR_LEN,
        idx2 in 0usize..CHAR_LEN,
        ref directions in vec(0u8..2, 1..50),
    ) {
        let r = Rope::from_str(text);

        let idx1 = if r.len_chars() == 0 { 0 } else { idx1 % r.len_chars() };
        let idx2 = if r.len_chars() == 0 { 0 } else { idx2 % r.len_chars() };
        let start = idx1.min(idx2);
        let end = idx1.max(idx2);

        let s = r.slice(start..end);
        let text = string_slice(text, start, end);

        let mut itr1 = ropey::iter::Lines::from_str_pt(text);
        let mut itr2 = s.lines();

        for &dir in directions {
            if dir == 0 {
                assert_eq!(itr1.next(), itr2.next());
            } else {
                assert_eq!(itr1.prev(), itr2.prev());
            }
        }
    }

    #[test]
    fn pt_bytes_at_01(idx in 0usize..TEXT.len()) {
        let r = Rope::from_str(TEXT);
        let mut bytes_r = r.bytes_at(idx);
        let text_bytes = TEXT.as_bytes();

        #[allow(clippy::needless_range_loop)]
        for i in idx..r.len_bytes() {
            assert_eq!(bytes_r.next(), Some(text_bytes[i]));
        }
    }

    #[test]
    fn pt_bytes_at_02(idx in 0usize..TEXT.len()) {
        let r = Rope::from_str(TEXT);
        let mut bytes_r = r.bytes_at(idx + 1);
        let text_bytes = TEXT.as_bytes();

        let mut i = idx + 1;
        while i > 0 {
            i -= 1;
            assert_eq!(bytes_r.prev(), Some(text_bytes[i]));
        }
    }

    #[test]
    fn pt_chars_at_01(idx in 0usize..CHAR_LEN) {
        let r = Rope::from_str(TEXT);
        let mut chars_r = r.chars_at(idx);
        let chars_t = (&TEXT[char_to_byte_idx(TEXT, idx)..]).chars();

        for c in chars_t {
            assert_eq!(chars_r.next(), Some(c));
        }
    }

    #[test]
    fn pt_chars_at_02(idx in 0usize..CHAR_LEN) {
        let r = Rope::from_str(TEXT);
        let mut chars_r = r.chars_at(idx);
        let mut chars_t = (&TEXT[..char_to_byte_idx(TEXT, idx)]).chars();

        while let Some(c) = chars_t.next_back() {
            assert_eq!(chars_r.prev(), Some(c));
        }
    }

    #[test]
    fn pt_bytes_iter_exact_01(idx in 1024usize..(CHAR_LEN - 1024)) {
        let r = Rope::from_str(TEXT);
        let s = r.slice(idx..(idx + 373));

        // Forward
        {
            let mut byte_count = s.len_bytes();
            let mut bytes = s.bytes();

            assert_eq!(byte_count, bytes.len());

            while let Some(_) = bytes.next() {
                byte_count -= 1;
                assert_eq!(byte_count, bytes.len());
            }

            assert_eq!(byte_count, 0);
            assert_eq!(bytes.len(), 0);
        }

        // Backward
        {
            let mut byte_count = 0;
            let mut bytes = s.bytes_at(s.len_bytes());

            assert_eq!(byte_count, bytes.len());

            while bytes.prev().is_some() {
                byte_count += 1;
                assert_eq!(byte_count, bytes.len());
            }

            assert_eq!(byte_count, s.len_bytes());
            assert_eq!(bytes.len(), s.len_bytes());
            bytes.prev();
            assert_eq!(bytes.len(), s.len_bytes());
        }
    }

    #[test]
    fn pt_chars_iter_exact_01(idx in 1024usize..(CHAR_LEN - 1024)) {
        let r = Rope::from_str(TEXT);
        let s = r.slice(idx..(idx + 373));

        // Forward
        let mut char_count = s.len_chars();
        let mut chars = s.chars();

        assert_eq!(char_count, chars.len());

        while let Some(_) = chars.next() {
            char_count -= 1;
            assert_eq!(char_count, chars.len());
        }

        assert_eq!(char_count, 0);
        assert_eq!(chars.len(), 0);

        // Backward
        let mut char_count = 0;
        let mut chars = s.chars_at(s.len_chars());

        assert_eq!(char_count, chars.len());

        while chars.prev().is_some() {
            char_count += 1;
            assert_eq!(char_count, chars.len());
        }

        assert_eq!(char_count, s.len_chars());
        assert_eq!(chars.len(), s.len_chars());
        chars.prev();
        assert_eq!(chars.len(), s.len_chars());
    }

    #[test]
    fn pt_lines_iter_exact_01(idx in 1024usize..(CHAR_LEN - 1024)) {
        let r = Rope::from_str(TEXT);
        let s = r.slice(idx..(idx + 373));

        // Forward
        let mut line_count = s.len_lines();
        let mut lines = s.lines();

        assert_eq!(line_count, lines.len());

        while let Some(_) = lines.next() {
            line_count -= 1;
            assert_eq!(line_count, lines.len());
        }

        assert_eq!(line_count, 0);
        assert_eq!(lines.len(), 0);

        // Backward
        let mut line_count = 0;
        let mut lines = s.lines_at(s.len_lines());

        assert_eq!(line_count, lines.len());

        while lines.prev().is_some() {
            line_count += 1;
            assert_eq!(line_count, lines.len());
        }

        assert_eq!(line_count, s.len_lines());
        assert_eq!(lines.len(), s.len_lines());
        lines.prev();
        assert_eq!(lines.len(), s.len_lines());
    }
}

//===========================================================================

// Char count of TEXT, below
const CHAR_LEN: usize = 18267;

// 31539 bytes, 18267 chars, 95 lines
// Contains many long graphemes.
const TEXT: &str = "
T̴̷͚͖̜͈̪͎͔̝̫̦̹͔̻̮͂ͬͬ̌ͣ̿ͤ͌ͥ͑̀̂ͬ̚͘͜͞ô̵͚̤̯̹͖͍̦̼̦̖̞̺͕̳̬͇͕̟̜̅̌̈́̑̏̕͘͝ ͍̼̗̫͈̭̦̱̬͚̱̞͓̜̭̼͇̰̞ͮ͗ͣ́ͪ̔ͪ̍̑̏́̀̽̍̔͘͜͜͝ȋ̐̽ͦ̓̔̅͏̧̢̖̭̝̳̹̯̤͈̫͔͔̠͓͉̠͖̠͜ͅn̷̯̗̗̠̱̥͕͉̥͉̳̫̙̅͗̌̒͂̏͑̎̌̌̊͌͘͘ͅͅv̧̜͕͍͙͍̬͕͍̳͉̠͍̹̮̻̜ͨ̏͒̍ͬ̈́͒̈ͥ͗ͣ̄̃ͤ͊̌͆̓o̸̧̎̓͂̊͢҉͍̼̘͇̱̪̠͎̥̹ķ̈́͗͆ͥ͐͑̆̎́͌ͩͯ̊̓͐ͬ̇̕҉̢͏͚̲̰̗̦e̿̀͐̽ͪ̈ͤͬ҉́͟͏̵̫̲̱̻̰̲̦͇̭̟̺͈̞̫̰̜͕͖ͅ ̡̰͎͓͚͓͉͈̮̻̣̮̟̩̬̮̈̋̊͆ͪ̄ͪ͒ͨͧ̇ͪ̇̑̚t̷̬̟͎̞͈̯͙̹̜ͩ̓ͪ͛͐̐ͤ̾̄̈͒̽̈́̑͒̏h̨̢̳͇͓͉̝ͫ̐̓̆̓ͮ̔̓̈́̇ͫe̟̬̣̗͚̬̾̉͋̽ͯ̌ͯͬ̂ͯͭ̓͛́̚͡ ̨̭̱͉̭͈̈̽̆̂͒͗̀ͥͩ͡h̻̼̱̹̗͖̙̗̲̤͓͇͚͚̻̞̥ͥ͛͌ͧ̚͟i̢̯̹̹̘̳̙ͩ̉ͥ͆̽̇̾̎͗̔̓͂͂́̓̌ͬv̧̡̛̟̜̠͉͖̘̲̻̯͚͍͓̯̻̲̹̥͇̻̿̓͛̊̌ͩͩ́ͩ̍͌̚e̵̾́̈́̏͌͌̊͗̏͋ͦ͘͡͏͚̜͚͎͉͍̱͙̖̹̣̘̥̤̹̟͠-̔̌͐́͒ͦͮ̇ͭ̄̏̊̇̍̕͏̩̥̰͚̟m̨̒ͫͦ̔̔͋҉̱̩̗͇̥̰̩̭͍͚͠į̵̷̻̗͉͕͚̣̼̺͉̦̮̠̆̀̐ͩ͒ͯͩͯ͞ͅn̢̫̤̝̝͚̺͍̱̦͚͂̿ͨ̇ͤ͠d̡ͯ͋̋ͧ̈́̒̈͏̛͏̵̤̬͍̗̞̠̟̞̺̠̥̹̱͉̜͍͎̤ ̷̸̢̰͓̘̯͎̤̫̘͓̙̟̳͇̹̥͈͙̮̩̅̋͌͗̓͊̓ͨͣ͗̓͐̈́ͩ̓ͣrͫ͂͌ͪ̏̐̍̾ͥ̓͗̈͆̈ͥ̀̾̚̚҉̴̶̭͇̗͙̘̯̦̭̮̪͚̥̙̯̠͙̪͡e̵̸̲͉̳̙͖͖̫̘̪͕̳͓̻̙͙ͥ̍͂̽ͨ̓̒̒̏ͬ͗ͧ̑̀͠p̵̸̛̦̣͙̳̳̩̣̼̘͈͂ͪͭͤ̎r̶̩̟̞̙͔̼ͫ̆ͦ̐̀̏̾̉̍ͬ̅ͧ͊ͪ̒̈́ͬ̃͞ẻ̴̼͙͍͎̠̀̅̔̃̒͐ͦ̏̆̅̓͋͢ͅš̆̈̆̋ͨ̅̍̇͂̒ͩͨ̂̐̓ͩ͏̸͔͔̯͇͚̤̪̬̗͈̰̦̯͚̕ę̢̱̠͙̲͉̗͚̮̪͖̙̞̦͉͕̗̳͙ͦ̆̋͌ͣ̅̊́ͅņ̴̷̫̪̦͇̺̹͉̗̬̞̲̭̜̪͒̏͂̂̎͊́̋͒̏̅̋̚͘t̷̶̨̟̦̗̦̱͌͌ͩ̀i̴̴̢̖͓͙̘͇̠̦̙̭̼͖̹̾̒̎̐ͥͭ͋ͥ̅͟ͅņ̫͙̹̦̳͈͙̬̫̮͕̰̩̣̘̘͐̀̓ͭͩͬͯ̎͛̿ͫ̊̔̅́̕͠gͥͩ̂͌̒̊̕͏̻͙͖̣͙͍̹͕̝͖̼̙̘͝ ͤ͐̓̒̓͋̐̃̇͊̓ͦ͐̚͢҉̢̨̟̠͉̳͖̲̩͙̕ć̷̡̫̩̞̯̼̝̼͖̤̳̻̘̪̤͈̦̭ͣ́͂͐̽͆̔̀̚͜h̶̢̹̹̙͔̱̓ͦ͌̋̎ͭ͒͋̒ͭ̌̃͌̿ͣ̆̅͑ą̙̳̬̞̬͚̜̤̱̙͇̠̟̈ͤ͋̃̀̓̓ͯ̍̀̽ͣ̐̈̿̌̕ǫ͋͂͐ͬ̿ͯ̂̈́͌̓̌ͧ̕͏̜͔̗͚͔̘̣͕̘̲͖̼͇͖̗̳ͅͅs̷̸̝̙̭̦̣̦̯̭̦͙̹̻͍͇̣̼͗̌͆ͨͭ̃ͮ͐̿̕.̮̝̠̱̺͖͓̼̦̱̉͂͛̓̑̔̓ͮ̈̊̔͗́͝\r
̛̣̺̻̼̙̼͓̱̬͕̩͕̲̳̭̗̍ͤ͋̒̆̄ͨ̿ͧ̓͠ͅI̷̻̤̳̲͔͈̖̬̰͔̪͇͇̟̋ͨ̋̍̉̔͝͞͝ͅn̶͕̭͖̠̣͚̹̪͆ͪ̇̂̅̾ͫ́̅̉ͭ̀͜v̖͉̩͕̣͔̭͕̩̲̖̇̀ͬ́̄͒̆͑͆ͪͤ͆̾̍ͯ̚͜ǫ̡̡̫͎̟̞̰̞̹͇̲̏ͨ̄͊̊̇͒̽͢ķ̶̪̙̰̥͙̞̹̭̺͍͕̙̲̮͊ͭ́͋͛͋̑̒͊̏̒̅͛̄̓͟i̴͎̹̞̥͖̒̄ͮ̒̾ͮͧ̀̚͡n̸̵͓̲̟̞̳͚̼̣͙͖̈ͦ͒̿̅̒̿͛͊̇ͧ̉g̡̧̪̩͚͙͓̪͓͚͉̥̪͍̙̻͖͇͗̑͊͑̾̍͊̀ͅ ̷̵̠͚̘̟͓̫̣̲͎̩̹̣̼̟͊́̏ͫ̆ͩ̓͋͆̿̽̓͘̕t̴̢̝̻̖̲̬̜̺̖̻ͩ̿ͫ͗̈́̔͑̐ͮͦ̽̉̓̚͜h̷̛̲͇̫͈̣̭͂ͭ̂͋ͭ̋̔ͮ̆ͩ͞ë̩͕͉̯͇͔͚̭̼̮̣͓̯́ͭ̀ͣ͗̋̉ͨͬ̒ͥͩ͆̓̓́̀̚͘͝ ̛̫̠̗̥̳͇͉̟̮̪̻̤̪͚̟̜̔̌͌̈͌ͪ̋̎̄ͯ͐ͦ́͞͠fͦ̂̈ͬ̇̅̓̓ͫͣ̉̂̉̚͘͡͡͏̼̖̟͚̙̳͔͎̲̫̦̯͔̣̼̹ě̷̶̫͎̞̺̪̪͇͈̞̳̏̋̋͋̾̓̽̓̑ͮ͊ͣ̋̃̅̀͡e͇̗͎̱͔̦̠̰̩̩͖͙̠̻̝ͯ̿̔̀͋͑ͧ͊̆̇̿ͤ̄ͯ̇̀͢͠ͅl̂̿ͯ͛̊̒̓̈́͏̵̪̦̞̤̫̤͇̙̗͕͎̪͕̙̻̳̗̕͟͢i̞̣̙͎͈̗̮͉̱̜̱̝̞̤͋ͯ͋͐̈́ͫ̉̊̏̀ͯͨ͢͟͝n̳̻̼̥̖͍̭̅͂̓̔̔ͦ̔́ͦ͊̀͛̈́ͬͦ͢͡͡ģ̶̡̳̰̻̙̞̱̳̣̤̫̫͕̤̮̰̬̪̜͋͒̎̈́̉̏̀ͬͯ͌̇͊̚ ́̽ͤͦ̾̔͢҉̛̤͍͉̺̙̮̗̜̟̀͝ơ̢̱͓͓̙͉̖̠̯̦̗͍̓̐̃̉̅̃ͨ͆́ͪ̂̒̀̊̃͆̔͡͡ͅf́ͬ̊ͯͫ̉̈́̽̉̚͢͏̡̺̬̖͇̫͉̱ ̴͇̦̗̙̼̬͓̯͖̮͓͎̗͈̻̈́͆ͭ̐ͦ́͛̀͋̐̌ͬ͑̒̊̿̃͞c̶̸̣͔̬͕̪̱̩̣̑̒̑̓̍̓͂̍̔͌̚͘͜͞h̶͈̱͇͉̳͍͍̰͈͖̬̥͚̯͓̞̹̋̔ͯ̑̃́̒̎̎͊̈́̍̚̕ạ̴̞̱̥͍͙̺͉͚͎̫̦͎̥ͩ̀̀̊ͥ͢o̵̧͕̜͓͈̬̰̫̮͙̹͉̩̝̩͎̓̆͗̿̊̀ͯ̃ͪ̊ͫ̽̉̓ͧ͗́̚͢ͅͅs̡ͫ͋̑ͮ̍̃͊̄ͬ̅̈́ͬ̍̇̔̈̅̍̀҉̜͓̝̘̘̮̼͖͎̻͓͖̖͙̞ͅ.͗ͬͭͩ̌̅͗͏̷̮̗͇͔͇͈̮͢\r
̨͚̲̫̠̼͖̝̻̉ͤ̅̂ͩ̀̇ͬͭ̀͜Ẅ̢́̉͌ͮͬͨ͊̏͌̇̐͊͟͠҉̼̰̦̩͇͕̟̭̪̲͕̥͖̰̪͈̀ͅͅį̷ͣͦ̉̍ͨ͂͂͑̃͂ͪ̊̈̋̄͜҉̨͚̟̲̯̹̺̝̭̺̙͖͍t̼͓̰̩͙̦͓̟͚͖̀ͯ͛̍̈́͑͂̍̋́h̛̼̺̘̥̠̼̼̭͙̮͚̱̍ͯ̓̃̐̂̇͟ ̴̛͖͔̰̠̺̥̲ͮ̍ͫ̽͜õ̒ͯ̒̓ͦ̈́͑̔̒̓̎ͤ͑҉̸̭̱̤̭̬͈ų̙̫̤͖̺̫̱͓͓̗̪͇̩̙̔̉̊͂ͪ̇͢͟͞ͅt̸̬̣̫̞̫̅͐ͮ̌͌̈́̀̀͘ ̷̴̨̖̙̹͚ͬ̈́̈ͯͨͮ̇̈́̋̈́ͭ͛̑̉͊̕ö̡̍ͥ̂ͬͪͧ͒ͧ̏̓̇̂̄͆̌ͫͤ͢͠͝͏̖̱̯̘͙̰̖͎̰͓̟̤ṙ̡̬̟̬̜̪̮̺͖̗̘͈̟ͨ͐͗̑͒̐d̢ͭͫ̊̏ͬͥ͋́̌̈́ͮ̆ͬ̐̌̎͏̵̷̡̞̲̹̙͕̮̮͚ḙ̴̸̠͔͎̥͇͖͕̘̍̓̏̐ͩͩ̈́ͦ̐̋ͤ̎̾̌̏͊̊́̚͞ͅr̸͈̗̣̲̗̣̬̤ͦ̎ͫ̏̀ͥͪ̋ͧ̄͑̋͒͌͋ͦ̉͟͞.ͨͣ̽̈́͒̄ͮ̀͋͋͏̴̧̯̺̙̱̻͙̜\r
̡̣̞̠͓̰͍̠͕̭̺̼͊̽̿͊ͮ̐̓̒̊͒̔̓͐ͨ̈̌́T̸̸̓́̋ͬ́͆ͨͫ͌͂ͣ̋͒҉̺̝͎̟͖͚̠h̸̡̰̜̦͇͕̪̝̳͕͉̲̝̑ͥ͋ͧ̎̆͌͟e̛̹͍͍̫̙̞̪̭̙̟͙̱̺̮̳͕̜ͫ̓ͭ͊ͫ͆̀̚͟͡ ̿͂̄ͧ̔̎ͧ͑̾̀̓͏̦͍̳͈̳͔̘̖̲̯̰̟̝̳̖̦N̶̡̧̦̮̟̦̩̰̣̝̆̀͊̔͢e͛̄ͮͦͨ͂̔̓̍̄̉͆͊̑̑̆̚͏̜̗͎̝̼̯̥̜͖͍̪̝͞ͅͅz̨̛̀̾ͪ͗̉́͠͏͚̫̼̫̜̣pͪͦ͌̄ͥ̆ͣͩ͋̉́̏͞͏̥̜̝̳̱̞̙̳̤͙̟̟̮̦ȅ̷̩̟͉̯͕͔̘̺̥̻̻ͧ̊̅̽ͣ͑̓̑̽ͦ̾͌͜r̴̭̥̲̲̤͚͈̰͇̰͈̰̹ͫ̒ͯ̿͒ͧ̊͆͒ͣ́ḍ̭̟̤̈́̌̓̈́ͫ͐̍͂͞į̛̞̝̮̣͙͙̤̇̂̓̎͋̿̓̎̄̈́ͧ̓ͩ̐̓̄̋ͭ͞͠a͋̔̋ͫ̂͐͂҉̸̛̥̩̯̯̤̝͔̠̝̯̪̥̩̻̼̮n͌ͣ̂͋̿̚҉̛̙̲̺̯͇͓̝̯̪̟͔̩͟ͅ ̢̨͚̻̗̘͖̯̐ͥ͋̽ͯ̎̈́͋̏̄͋̆̑̊̆̚̕͟ͅh̢̛̗̱̭͇͖̰̮̮͈̲͍̯̟ͭ͊̎̽̓ͦͤ͠ï̛̘̝̦͎̦̭̠͖̳͎̮̼̏͐ͧ̒̒͐͑ͪͫ̋̽̚̚͜v̴̮͕̝̮̞͐̄͗̋͒ͤ̎̈̑ͬͮ̄̾ͤ̓̾͊͗͟é̶̷̡̩͖̰̫͓̟ͮͬͣ͊-ͦ͛ͩͤͨͨ̆̄͏̼̜̭͔̳͈͖̳̩͢ͅͅm̷̴̓́̓͛͒̾̍̉҉̛̗̹̠̣̪̺͎̖̝͚̖͙i̛̥͓̬̫͉͕͉͆͒ͧ̂̿̔̔͆̆̓̍͊̀͜n͌ͧͣ̅̌̎ͦͦ͑̑ͭ̆ͬ̀ͤ̀ͣ̚҉͎̰̱͚͈͈̬̹͕̺̙͙̼͘͘͞d̶͖̫̟̲͕̺̠͎̘͕̱̼͙̪̪̩͙̅̅̑̓̇͑̊̉͜͞ ̶̵̷̴̡̠͚̪͕̣̱̖̱̗̤̭̭͔͖͚ͧͤͥ͒̌ͪ͊͂͒̓͂ͧͧ̇̇͐̑̔ͅͅơ̵̲̲͇̯̰͇̜̣͕͕͓̲̤̲͔͚̞͑͗ͤ̓́̚͠ͅf̢̧̛̩̯̼̫͖̾ͣ͌̾̉́̈́̑̈́̚͞͞ͅ ͤͩ́͋͒ͫͬͣ̋̅̆҉̧̱̻͓͕͉̹̫̫̞̯̪̙̩͍̦͔̖̮̀͟ͅc͉̠̜̩̟͕͎̙̣̮̘̼͋ͯ̍ͨ̅̄ͫ̈̋ͫ̊͡͝ȟ̸̨ͯͦ̂̉̇̾̆ͭ̋̐̈̆̀̚͜҉͚͕̻̖a̶̴̛͚̗͙̳̬̲͚ͦ́̐ͥ́̔̅̑̎͐̑ͯ̾ͤͥͧ͡ò̶̧̞̪̦̥̪̻̦̝̳̬̔͛͛ͣ̋̌̔ͫ̂̽ͫ͘͠s̸̖̣̬̤̫͇̫̣̑͆͒̎̏́͟.̴̗̤̭͉̯̻̤͕̌ͯ̍ͤ̓͌ͤ̈̆̉ͦ̇́̚͘͟͝ͅ ̯̹̪͓̬͌̔̌ͬ̀͘͢͡͡Z̡̩̲̩̰̫̩̟͍̰͖͔̭ͣ̆̾ͭ̀́͞ͅa̡̡̙̜̭͇͎͔̙̞̫͓̜͉͔̬ͭ̈ͨ̉͆ͣͫ̃͌̓͌́ͣͥ̒̌͊͘͝l̢̨̡̯̙̫͖̫̺̘̬̟͈͌̊ͧͫͦ̉̃ͩͦ̒ͯ̇̌̓͛͟͝ͅg̵̙̼̼ͪ͂ͭ͗̈̕ȯ̅ͧ̓ͪ́̂͑̐ͩͥͬ̊̑͆̇͒ͫͣ͝҉͎̟̜̥͎̮̣͉̖̟̯̦̖͙͙͞ͅ.̈̑ͩ̇̂ͬ̓ͬ͊͂ͨ̽͠͏̺͎̞̦̜͍͚̯̯͔̝̞̻̩̖\r
̷̰̪͍͎͔͒ͯͥ̾̉͆ͤ̊̓̂͋̀͆H̸̸̹̞̙̺͎̠̯̤ͨ̉̍ͬͤ̓̐͌ͥͮ͞eͣ̈̾͛́͏͕̗͍̜̼͎͚̟̬̣̝̕ͅͅ ̴̛̩̗̼̝̣̩͚͇̯́̉͋̂̍͂̌ͮ͋̾͜͠wͮ̽̓ͭ̿͐̽̐̽͆̓͝҉̡̼̲͖̪̥h̢̢̛͍̰̰̻̱̼̰̹̖̖̪̝̥̘̎̀ͪ͒̾ͫͬ̆̑o̡̗̠̞̱̥͎̰͎͍̫̻͓͇͓͐ͥͯ͂̅͠ͅ ̡̛̏͑ͦ̓͊ͮͫͯͭ̌͒̆̍̈͠҉͖͚̪̫̗̮W̴̐̊͋̾ͥͫ҉͎̞͔̯̫̹͖̰͉̹̼͎̰̱͓̻̀a̶̩̤̙̣͎̳̭̲̗̠͉̳̭̭̦̞͎̮̅͌̾͗̾͛̇̀́͟͞ͅi̷̡ͣ̆̌͋͒͒́͘͏̮̺̩͎͇̜͍̫ṯ̴̢͖̥̖͇͎̦̦̹̖͇̪ͭ̅̍͐̇͒͋̽̏̿̒͆ͧ̄͋ͧͩ͒͜s̙̥̖̘̖͚̭̤̮̖̘̰̫̟̈́ͣ̍ͧ͐ͥ̏͆̃̿͒̔͐́̚͟ͅ ̨ͭ̌ͬͯ͆̒͋ͭ̔̿ͧ̅̓ͣ͡͏͇̟͉̥͔̬̼͚͙͚B̛̜̮̤͓̝̪̪͈͕̘̜͙̰̮̫̘̣͓͔̅ͩ͊̔ͦͯ́̌́͆ͭ̓́e̶̢̡̦͇͕̙͈͖͕̦̬̫͕̣̺̒̿͂͐͒͋͂ͦ́͋ͤ̿ͬ̊ͣ͗̑̽͜ͅͅh̸͑ͫͧ̑ͬͧ̈́̎̃ͣ̊̾͂ͨͤ̓͐̐̑͏̸̭͓̘͉̩i̧̧̭̣͈̝̺̼̺̠͉̞̜̲̳͙̦͐̔ͯ͛̅̾n̸͓̝̤̙͙͔ͪ̋̈́͒̒ͭ̈́̓ͮ̋̀̋̀̈ͩ́͌̄͘d̷̫̳̩̼̥̗̲̰͇͉̼̬̤͇̖ͮ̿ͬ͂ͦ̏̓ͮ̽͂̾̾ͯ͆͜͠ ̨̈́͒̇̏̄̑̓ͮͥ̒ͤͨ̋҉̴̴̟̱͙̟̫̩̗͔̝͔̀Ţ̵̝̟̖̭͇̻̳͖͉̺̖̖͙͙̺̐̈́̓ͯ̆̇̋ͩ͊̄̾̾ͬ̌̚͟ͅh̡͈̗̜͙̬̗̲̦̲̟̗̦̬͓̳ͧ̋̌͂͂ͨͬͦ̿̏̈́̋ͣ̒̕͡ͅͅe̗͇̰̰̥̪̗͑̔̓́̈́ͨ̊́̿̅ͯͥ̈́͐͗͘͢͝ ̡̢̛̯͎͓̰̘͎̦̪̯̪̥̰̲͇̠̲͔ͤͤ̇̅̆̋̂̆̈́ͤ̿͑ͅW̡͓͈̲̲͉̜͔̖͈̻̱͚̿̌͗̉ͤ͢͡ͅͅa̔̾͛̅͊͋͐҉̱̹̬͍͙̻̱l̢͎̟̬̙̼̱̫̮̘̼͔̭̅ͬ͑ͣ̏̾̅̓ͣ̿ͣ̈́̕͢͡ͅͅl̡̥̣͔̭̇̒͛͒͐̄̽͛̋ͥ̌͢͟͡.̷̰̝̮͔̟̦͈̥̬̻̥̬͎͓̻̲̇ͮ̿ͨͦ̽ͫ͟͢͝͠\r
̗̱͖͈͌̈ͦ͛ͮ̌͋̽̃͆̀͂ͨͧ̄̔̔ͭ̏͢Z̃̉̿ͮ̃̀͘͏͕̬̯̖͚̗͔Aͣ̑̈̓̈̑̈̀̿̚҉͙͍̦̗̦͙̠̝̩̯ͅͅL̴͖̞̞͙̱̻̥̬̜̦̐̇̉̈̽ͪ̅ͪ̂̔͌͑ͭ͐ͤ̈́̿̉͞ͅG̴̵̲̰̹̖͎͕ͯ̆̓̽͢͠Ŏ̶̡̺̼͙̣̞̩͕̥̟̝͕͔̯̞ͨ͒͊̂̊͂͗̒͆̾͆̌͆̃̎ͣͫ͜͡ͅ!̓̽̎̑̏́̓̓ͣ̀͏̱̩̭̣̹̺̗͜͞͞\r

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Maecenas sit\r
amet tellus  nec turpis feugiat semper. Nam at nulla laoreet, finibus\r
eros sit amet, fringilla  mauris. Fusce vestibulum nec ligula efficitur\r
laoreet. Nunc orci leo, varius eget  ligula vulputate, consequat\r
eleifend nisi. Cras justo purus, imperdiet a augue  malesuada, convallis\r
cursus libero. Fusce pretium arcu in elementum laoreet. Duis  mauris\r
nulla, suscipit at est nec, malesuada pellentesque eros. Quisque semper\r
porta  malesuada. Nunc hendrerit est ac faucibus mollis. Nam fermentum\r
id libero sed  egestas. Duis a accumsan sapien. Nam neque diam, congue\r
non erat et, porta sagittis  turpis. Vivamus vitae mauris sit amet massa\r
mollis molestie. Morbi scelerisque,  augue id congue imperdiet, felis\r
lacus euismod dui, vitae facilisis massa dui quis  sapien. Vivamus\r
hendrerit a urna a lobortis.\r

T̴̷͚͖̜͈̪͎͔̝̫̦̹͔̻̮͂ͬͬ̌ͣ̿ͤ͌ͥ͑̀̂ͬ̚͘͜͞ô̵͚̤̯̹͖͍̦̼̦̖̞̺͕̳̬͇͕̟̜̅̌̈́̑̏̕͘͝ ͍̼̗̫͈̭̦̱̬͚̱̞͓̜̭̼͇̰̞ͮ͗ͣ́ͪ̔ͪ̍̑̏́̀̽̍̔͘͜͜͝ȋ̐̽ͦ̓̔̅͏̧̢̖̭̝̳̹̯̤͈̫͔͔̠͓͉̠͖̠͜ͅn̷̯̗̗̠̱̥͕͉̥͉̳̫̙̅͗̌̒͂̏͑̎̌̌̊͌͘͘ͅͅv̧̜͕͍͙͍̬͕͍̳͉̠͍̹̮̻̜ͨ̏͒̍ͬ̈́͒̈ͥ͗ͣ̄̃ͤ͊̌͆̓o̸̧̎̓͂̊͢҉͍̼̘͇̱̪̠͎̥̹ķ̈́͗͆ͥ͐͑̆̎́͌ͩͯ̊̓͐ͬ̇̕҉̢͏͚̲̰̗̦e̿̀͐̽ͪ̈ͤͬ҉́͟͏̵̫̲̱̻̰̲̦͇̭̟̺͈̞̫̰̜͕͖ͅ ̡̰͎͓͚͓͉͈̮̻̣̮̟̩̬̮̈̋̊͆ͪ̄ͪ͒ͨͧ̇ͪ̇̑̚t̷̬̟͎̞͈̯͙̹̜ͩ̓ͪ͛͐̐ͤ̾̄̈͒̽̈́̑͒̏h̨̢̳͇͓͉̝ͫ̐̓̆̓ͮ̔̓̈́̇ͫe̟̬̣̗͚̬̾̉͋̽ͯ̌ͯͬ̂ͯͭ̓͛́̚͡ ̨̭̱͉̭͈̈̽̆̂͒͗̀ͥͩ͡h̻̼̱̹̗͖̙̗̲̤͓͇͚͚̻̞̥ͥ͛͌ͧ̚͟i̢̯̹̹̘̳̙ͩ̉ͥ͆̽̇̾̎͗̔̓͂͂́̓̌ͬv̧̡̛̟̜̠͉͖̘̲̻̯͚͍͓̯̻̲̹̥͇̻̿̓͛̊̌ͩͩ́ͩ̍͌̚e̵̾́̈́̏͌͌̊͗̏͋ͦ͘͡͏͚̜͚͎͉͍̱͙̖̹̣̘̥̤̹̟͠-̔̌͐́͒ͦͮ̇ͭ̄̏̊̇̍̕͏̩̥̰͚̟m̨̒ͫͦ̔̔͋҉̱̩̗͇̥̰̩̭͍͚͠į̵̷̻̗͉͕͚̣̼̺͉̦̮̠̆̀̐ͩ͒ͯͩͯ͞ͅn̢̫̤̝̝͚̺͍̱̦͚͂̿ͨ̇ͤ͠d̡ͯ͋̋ͧ̈́̒̈͏̛͏̵̤̬͍̗̞̠̟̞̺̠̥̹̱͉̜͍͎̤ ̷̸̢̰͓̘̯͎̤̫̘͓̙̟̳͇̹̥͈͙̮̩̅̋͌͗̓͊̓ͨͣ͗̓͐̈́ͩ̓ͣrͫ͂͌ͪ̏̐̍̾ͥ̓͗̈͆̈ͥ̀̾̚̚҉̴̶̭͇̗͙̘̯̦̭̮̪͚̥̙̯̠͙̪͡e̵̸̲͉̳̙͖͖̫̘̪͕̳͓̻̙͙ͥ̍͂̽ͨ̓̒̒̏ͬ͗ͧ̑̀͠p̵̸̛̦̣͙̳̳̩̣̼̘͈͂ͪͭͤ̎r̶̩̟̞̙͔̼ͫ̆ͦ̐̀̏̾̉̍ͬ̅ͧ͊ͪ̒̈́ͬ̃͞ẻ̴̼͙͍͎̠̀̅̔̃̒͐ͦ̏̆̅̓͋͢ͅš̆̈̆̋ͨ̅̍̇͂̒ͩͨ̂̐̓ͩ͏̸͔͔̯͇͚̤̪̬̗͈̰̦̯͚̕ę̢̱̠͙̲͉̗͚̮̪͖̙̞̦͉͕̗̳͙ͦ̆̋͌ͣ̅̊́ͅņ̴̷̫̪̦͇̺̹͉̗̬̞̲̭̜̪͒̏͂̂̎͊́̋͒̏̅̋̚͘t̷̶̨̟̦̗̦̱͌͌ͩ̀i̴̴̢̖͓͙̘͇̠̦̙̭̼͖̹̾̒̎̐ͥͭ͋ͥ̅͟ͅņ̫͙̹̦̳͈͙̬̫̮͕̰̩̣̘̘͐̀̓ͭͩͬͯ̎͛̿ͫ̊̔̅́̕͠gͥͩ̂͌̒̊̕͏̻͙͖̣͙͍̹͕̝͖̼̙̘͝ ͤ͐̓̒̓͋̐̃̇͊̓ͦ͐̚͢҉̢̨̟̠͉̳͖̲̩͙̕ć̷̡̫̩̞̯̼̝̼͖̤̳̻̘̪̤͈̦̭ͣ́͂͐̽͆̔̀̚͜h̶̢̹̹̙͔̱̓ͦ͌̋̎ͭ͒͋̒ͭ̌̃͌̿ͣ̆̅͑ą̙̳̬̞̬͚̜̤̱̙͇̠̟̈ͤ͋̃̀̓̓ͯ̍̀̽ͣ̐̈̿̌̕ǫ͋͂͐ͬ̿ͯ̂̈́͌̓̌ͧ̕͏̜͔̗͚͔̘̣͕̘̲͖̼͇͖̗̳ͅͅs̷̸̝̙̭̦̣̦̯̭̦͙̹̻͍͇̣̼͗̌͆ͨͭ̃ͮ͐̿̕.̮̝̠̱̺͖͓̼̦̱̉͂͛̓̑̔̓ͮ̈̊̔͗́͝\r
̛̣̺̻̼̙̼͓̱̬͕̩͕̲̳̭̗̍ͤ͋̒̆̄ͨ̿ͧ̓͠ͅI̷̻̤̳̲͔͈̖̬̰͔̪͇͇̟̋ͨ̋̍̉̔͝͞͝ͅn̶͕̭͖̠̣͚̹̪͆ͪ̇̂̅̾ͫ́̅̉ͭ̀͜v̖͉̩͕̣͔̭͕̩̲̖̇̀ͬ́̄͒̆͑͆ͪͤ͆̾̍ͯ̚͜ǫ̡̡̫͎̟̞̰̞̹͇̲̏ͨ̄͊̊̇͒̽͢ķ̶̪̙̰̥͙̞̹̭̺͍͕̙̲̮͊ͭ́͋͛͋̑̒͊̏̒̅͛̄̓͟i̴͎̹̞̥͖̒̄ͮ̒̾ͮͧ̀̚͡n̸̵͓̲̟̞̳͚̼̣͙͖̈ͦ͒̿̅̒̿͛͊̇ͧ̉g̡̧̪̩͚͙͓̪͓͚͉̥̪͍̙̻͖͇͗̑͊͑̾̍͊̀ͅ ̷̵̠͚̘̟͓̫̣̲͎̩̹̣̼̟͊́̏ͫ̆ͩ̓͋͆̿̽̓͘̕t̴̢̝̻̖̲̬̜̺̖̻ͩ̿ͫ͗̈́̔͑̐ͮͦ̽̉̓̚͜h̷̛̲͇̫͈̣̭͂ͭ̂͋ͭ̋̔ͮ̆ͩ͞ë̩͕͉̯͇͔͚̭̼̮̣͓̯́ͭ̀ͣ͗̋̉ͨͬ̒ͥͩ͆̓̓́̀̚͘͝ ̛̫̠̗̥̳͇͉̟̮̪̻̤̪͚̟̜̔̌͌̈͌ͪ̋̎̄ͯ͐ͦ́͞͠fͦ̂̈ͬ̇̅̓̓ͫͣ̉̂̉̚͘͡͡͏̼̖̟͚̙̳͔͎̲̫̦̯͔̣̼̹ě̷̶̫͎̞̺̪̪͇͈̞̳̏̋̋͋̾̓̽̓̑ͮ͊ͣ̋̃̅̀͡e͇̗͎̱͔̦̠̰̩̩͖͙̠̻̝ͯ̿̔̀͋͑ͧ͊̆̇̿ͤ̄ͯ̇̀͢͠ͅl̂̿ͯ͛̊̒̓̈́͏̵̪̦̞̤̫̤͇̙̗͕͎̪͕̙̻̳̗̕͟͢i̞̣̙͎͈̗̮͉̱̜̱̝̞̤͋ͯ͋͐̈́ͫ̉̊̏̀ͯͨ͢͟͝n̳̻̼̥̖͍̭̅͂̓̔̔ͦ̔́ͦ͊̀͛̈́ͬͦ͢͡͡ģ̶̡̳̰̻̙̞̱̳̣̤̫̫͕̤̮̰̬̪̜͋͒̎̈́̉̏̀ͬͯ͌̇͊̚ ́̽ͤͦ̾̔͢҉̛̤͍͉̺̙̮̗̜̟̀͝ơ̢̱͓͓̙͉̖̠̯̦̗͍̓̐̃̉̅̃ͨ͆́ͪ̂̒̀̊̃͆̔͡͡ͅf́ͬ̊ͯͫ̉̈́̽̉̚͢͏̡̺̬̖͇̫͉̱ ̴͇̦̗̙̼̬͓̯͖̮͓͎̗͈̻̈́͆ͭ̐ͦ́͛̀͋̐̌ͬ͑̒̊̿̃͞c̶̸̣͔̬͕̪̱̩̣̑̒̑̓̍̓͂̍̔͌̚͘͜͞h̶͈̱͇͉̳͍͍̰͈͖̬̥͚̯͓̞̹̋̔ͯ̑̃́̒̎̎͊̈́̍̚̕ạ̴̞̱̥͍͙̺͉͚͎̫̦͎̥ͩ̀̀̊ͥ͢o̵̧͕̜͓͈̬̰̫̮͙̹͉̩̝̩͎̓̆͗̿̊̀ͯ̃ͪ̊ͫ̽̉̓ͧ͗́̚͢ͅͅs̡ͫ͋̑ͮ̍̃͊̄ͬ̅̈́ͬ̍̇̔̈̅̍̀҉̜͓̝̘̘̮̼͖͎̻͓͖̖͙̞ͅ.͗ͬͭͩ̌̅͗͏̷̮̗͇͔͇͈̮͢\r
̨͚̲̫̠̼͖̝̻̉ͤ̅̂ͩ̀̇ͬͭ̀͜Ẅ̢́̉͌ͮͬͨ͊̏͌̇̐͊͟͠҉̼̰̦̩͇͕̟̭̪̲͕̥͖̰̪͈̀ͅͅį̷ͣͦ̉̍ͨ͂͂͑̃͂ͪ̊̈̋̄͜҉̨͚̟̲̯̹̺̝̭̺̙͖͍t̼͓̰̩͙̦͓̟͚͖̀ͯ͛̍̈́͑͂̍̋́h̛̼̺̘̥̠̼̼̭͙̮͚̱̍ͯ̓̃̐̂̇͟ ̴̛͖͔̰̠̺̥̲ͮ̍ͫ̽͜õ̒ͯ̒̓ͦ̈́͑̔̒̓̎ͤ͑҉̸̭̱̤̭̬͈ų̙̫̤͖̺̫̱͓͓̗̪͇̩̙̔̉̊͂ͪ̇͢͟͞ͅt̸̬̣̫̞̫̅͐ͮ̌͌̈́̀̀͘ ̷̴̨̖̙̹͚ͬ̈́̈ͯͨͮ̇̈́̋̈́ͭ͛̑̉͊̕ö̡̍ͥ̂ͬͪͧ͒ͧ̏̓̇̂̄͆̌ͫͤ͢͠͝͏̖̱̯̘͙̰̖͎̰͓̟̤ṙ̡̬̟̬̜̪̮̺͖̗̘͈̟ͨ͐͗̑͒̐d̢ͭͫ̊̏ͬͥ͋́̌̈́ͮ̆ͬ̐̌̎͏̵̷̡̞̲̹̙͕̮̮͚ḙ̴̸̠͔͎̥͇͖͕̘̍̓̏̐ͩͩ̈́ͦ̐̋ͤ̎̾̌̏͊̊́̚͞ͅr̸͈̗̣̲̗̣̬̤ͦ̎ͫ̏̀ͥͪ̋ͧ̄͑̋͒͌͋ͦ̉͟͞.ͨͣ̽̈́͒̄ͮ̀͋͋͏̴̧̯̺̙̱̻͙̜\r
̡̣̞̠͓̰͍̠͕̭̺̼͊̽̿͊ͮ̐̓̒̊͒̔̓͐ͨ̈̌́T̸̸̓́̋ͬ́͆ͨͫ͌͂ͣ̋͒҉̺̝͎̟͖͚̠h̸̡̰̜̦͇͕̪̝̳͕͉̲̝̑ͥ͋ͧ̎̆͌͟e̛̹͍͍̫̙̞̪̭̙̟͙̱̺̮̳͕̜ͫ̓ͭ͊ͫ͆̀̚͟͡ ̿͂̄ͧ̔̎ͧ͑̾̀̓͏̦͍̳͈̳͔̘̖̲̯̰̟̝̳̖̦N̶̡̧̦̮̟̦̩̰̣̝̆̀͊̔͢e͛̄ͮͦͨ͂̔̓̍̄̉͆͊̑̑̆̚͏̜̗͎̝̼̯̥̜͖͍̪̝͞ͅͅz̨̛̀̾ͪ͗̉́͠͏͚̫̼̫̜̣pͪͦ͌̄ͥ̆ͣͩ͋̉́̏͞͏̥̜̝̳̱̞̙̳̤͙̟̟̮̦ȅ̷̩̟͉̯͕͔̘̺̥̻̻ͧ̊̅̽ͣ͑̓̑̽ͦ̾͌͜r̴̭̥̲̲̤͚͈̰͇̰͈̰̹ͫ̒ͯ̿͒ͧ̊͆͒ͣ́ḍ̭̟̤̈́̌̓̈́ͫ͐̍͂͞į̛̞̝̮̣͙͙̤̇̂̓̎͋̿̓̎̄̈́ͧ̓ͩ̐̓̄̋ͭ͞͠a͋̔̋ͫ̂͐͂҉̸̛̥̩̯̯̤̝͔̠̝̯̪̥̩̻̼̮n͌ͣ̂͋̿̚҉̛̙̲̺̯͇͓̝̯̪̟͔̩͟ͅ ̢̨͚̻̗̘͖̯̐ͥ͋̽ͯ̎̈́͋̏̄͋̆̑̊̆̚̕͟ͅh̢̛̗̱̭͇͖̰̮̮͈̲͍̯̟ͭ͊̎̽̓ͦͤ͠ï̛̘̝̦͎̦̭̠͖̳͎̮̼̏͐ͧ̒̒͐͑ͪͫ̋̽̚̚͜v̴̮͕̝̮̞͐̄͗̋͒ͤ̎̈̑ͬͮ̄̾ͤ̓̾͊͗͟é̶̷̡̩͖̰̫͓̟ͮͬͣ͊-ͦ͛ͩͤͨͨ̆̄͏̼̜̭͔̳͈͖̳̩͢ͅͅm̷̴̓́̓͛͒̾̍̉҉̛̗̹̠̣̪̺͎̖̝͚̖͙i̛̥͓̬̫͉͕͉͆͒ͧ̂̿̔̔͆̆̓̍͊̀͜n͌ͧͣ̅̌̎ͦͦ͑̑ͭ̆ͬ̀ͤ̀ͣ̚҉͎̰̱͚͈͈̬̹͕̺̙͙̼͘͘͞d̶͖̫̟̲͕̺̠͎̘͕̱̼͙̪̪̩͙̅̅̑̓̇͑̊̉͜͞ ̶̵̷̴̡̠͚̪͕̣̱̖̱̗̤̭̭͔͖͚ͧͤͥ͒̌ͪ͊͂͒̓͂ͧͧ̇̇͐̑̔ͅͅơ̵̲̲͇̯̰͇̜̣͕͕͓̲̤̲͔͚̞͑͗ͤ̓́̚͠ͅf̢̧̛̩̯̼̫͖̾ͣ͌̾̉́̈́̑̈́̚͞͞ͅ ͤͩ́͋͒ͫͬͣ̋̅̆҉̧̱̻͓͕͉̹̫̫̞̯̪̙̩͍̦͔̖̮̀͟ͅc͉̠̜̩̟͕͎̙̣̮̘̼͋ͯ̍ͨ̅̄ͫ̈̋ͫ̊͡͝ȟ̸̨ͯͦ̂̉̇̾̆ͭ̋̐̈̆̀̚͜҉͚͕̻̖a̶̴̛͚̗͙̳̬̲͚ͦ́̐ͥ́̔̅̑̎͐̑ͯ̾ͤͥͧ͡ò̶̧̞̪̦̥̪̻̦̝̳̬̔͛͛ͣ̋̌̔ͫ̂̽ͫ͘͠s̸̖̣̬̤̫͇̫̣̑͆͒̎̏́͟.̴̗̤̭͉̯̻̤͕̌ͯ̍ͤ̓͌ͤ̈̆̉ͦ̇́̚͘͟͝ͅ ̯̹̪͓̬͌̔̌ͬ̀͘͢͡͡Z̡̩̲̩̰̫̩̟͍̰͖͔̭ͣ̆̾ͭ̀́͞ͅa̡̡̙̜̭͇͎͔̙̞̫͓̜͉͔̬ͭ̈ͨ̉͆ͣͫ̃͌̓͌́ͣͥ̒̌͊͘͝l̢̨̡̯̙̫͖̫̺̘̬̟͈͌̊ͧͫͦ̉̃ͩͦ̒ͯ̇̌̓͛͟͝ͅg̵̙̼̼ͪ͂ͭ͗̈̕ȯ̅ͧ̓ͪ́̂͑̐ͩͥͬ̊̑͆̇͒ͫͣ͝҉͎̟̜̥͎̮̣͉̖̟̯̦̖͙͙͞ͅ.̈̑ͩ̇̂ͬ̓ͬ͊͂ͨ̽͠͏̺͎̞̦̜͍͚̯̯͔̝̞̻̩̖\r
̷̰̪͍͎͔͒ͯͥ̾̉͆ͤ̊̓̂͋̀͆H̸̸̹̞̙̺͎̠̯̤ͨ̉̍ͬͤ̓̐͌ͥͮ͞eͣ̈̾͛́͏͕̗͍̜̼͎͚̟̬̣̝̕ͅͅ ̴̛̩̗̼̝̣̩͚͇̯́̉͋̂̍͂̌ͮ͋̾͜͠wͮ̽̓ͭ̿͐̽̐̽͆̓͝҉̡̼̲͖̪̥h̢̢̛͍̰̰̻̱̼̰̹̖̖̪̝̥̘̎̀ͪ͒̾ͫͬ̆̑o̡̗̠̞̱̥͎̰͎͍̫̻͓͇͓͐ͥͯ͂̅͠ͅ ̡̛̏͑ͦ̓͊ͮͫͯͭ̌͒̆̍̈͠҉͖͚̪̫̗̮W̴̐̊͋̾ͥͫ҉͎̞͔̯̫̹͖̰͉̹̼͎̰̱͓̻̀a̶̩̤̙̣͎̳̭̲̗̠͉̳̭̭̦̞͎̮̅͌̾͗̾͛̇̀́͟͞ͅi̷̡ͣ̆̌͋͒͒́͘͏̮̺̩͎͇̜͍̫ṯ̴̢͖̥̖͇͎̦̦̹̖͇̪ͭ̅̍͐̇͒͋̽̏̿̒͆ͧ̄͋ͧͩ͒͜s̙̥̖̘̖͚̭̤̮̖̘̰̫̟̈́ͣ̍ͧ͐ͥ̏͆̃̿͒̔͐́̚͟ͅ ̨ͭ̌ͬͯ͆̒͋ͭ̔̿ͧ̅̓ͣ͡͏͇̟͉̥͔̬̼͚͙͚B̛̜̮̤͓̝̪̪͈͕̘̜͙̰̮̫̘̣͓͔̅ͩ͊̔ͦͯ́̌́͆ͭ̓́e̶̢̡̦͇͕̙͈͖͕̦̬̫͕̣̺̒̿͂͐͒͋͂ͦ́͋ͤ̿ͬ̊ͣ͗̑̽͜ͅͅh̸͑ͫͧ̑ͬͧ̈́̎̃ͣ̊̾͂ͨͤ̓͐̐̑͏̸̭͓̘͉̩i̧̧̭̣͈̝̺̼̺̠͉̞̜̲̳͙̦͐̔ͯ͛̅̾n̸͓̝̤̙͙͔ͪ̋̈́͒̒ͭ̈́̓ͮ̋̀̋̀̈ͩ́͌̄͘d̷̫̳̩̼̥̗̲̰͇͉̼̬̤͇̖ͮ̿ͬ͂ͦ̏̓ͮ̽͂̾̾ͯ͆͜͠ ̨̈́͒̇̏̄̑̓ͮͥ̒ͤͨ̋҉̴̴̟̱͙̟̫̩̗͔̝͔̀Ţ̵̝̟̖̭͇̻̳͖͉̺̖̖͙͙̺̐̈́̓ͯ̆̇̋ͩ͊̄̾̾ͬ̌̚͟ͅh̡͈̗̜͙̬̗̲̦̲̟̗̦̬͓̳ͧ̋̌͂͂ͨͬͦ̿̏̈́̋ͣ̒̕͡ͅͅe̗͇̰̰̥̪̗͑̔̓́̈́ͨ̊́̿̅ͯͥ̈́͐͗͘͢͝ ̡̢̛̯͎͓̰̘͎̦̪̯̪̥̰̲͇̠̲͔ͤͤ̇̅̆̋̂̆̈́ͤ̿͑ͅW̡͓͈̲̲͉̜͔̖͈̻̱͚̿̌͗̉ͤ͢͡ͅͅa̔̾͛̅͊͋͐҉̱̹̬͍͙̻̱l̢͎̟̬̙̼̱̫̮̘̼͔̭̅ͬ͑ͣ̏̾̅̓ͣ̿ͣ̈́̕͢͡ͅͅl̡̥̣͔̭̇̒͛͒͐̄̽͛̋ͥ̌͢͟͡.̷̰̝̮͔̟̦͈̥̬̻̥̬͎͓̻̲̇ͮ̿ͨͦ̽ͫ͟͢͝͠\r
̗̱͖͈͌̈ͦ͛ͮ̌͋̽̃͆̀͂ͨͧ̄̔̔ͭ̏͢Z̃̉̿ͮ̃̀͘͏͕̬̯̖͚̗͔Aͣ̑̈̓̈̑̈̀̿̚҉͙͍̦̗̦͙̠̝̩̯ͅͅL̴͖̞̞͙̱̻̥̬̜̦̐̇̉̈̽ͪ̅ͪ̂̔͌͑ͭ͐ͤ̈́̿̉͞ͅG̴̵̲̰̹̖͎͕ͯ̆̓̽͢͠Ŏ̶̡̺̼͙̣̞̩͕̥̟̝͕͔̯̞ͨ͒͊̂̊͂͗̒͆̾͆̌͆̃̎ͣͫ͜͡ͅ!̓̽̎̑̏́̓̓ͣ̀͏̱̩̭̣̹̺̗͜͞͞\r

Pellentesque nec viverra metus. Sed aliquet pellentesque scelerisque.\r
Duis efficitur  erat sit amet dui maximus egestas. Nullam blandit ante\r
tortor. Suspendisse vitae  consectetur sem, at sollicitudin neque.\r
Suspendisse sodales faucibus eros vitae  pellentesque. Cras non quam\r
dictum, pellentesque urna in, ornare erat. Praesent leo  est, aliquet et\r
euismod non, hendrerit sed urna. Sed convallis porttitor est, vel\r
aliquet felis cursus ac. Vivamus feugiat eget nisi eu molestie.\r
Phasellus tincidunt  nisl eget molestie consectetur. Phasellus vitae ex\r
ut odio sollicitudin vulputate.  Sed et nulla accumsan, eleifend arcu\r
eget, gravida neque. Donec sit amet tincidunt  eros. Ut in volutpat\r
ante.\r

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Maecenas sit\r
amet tellus  nec turpis feugiat semper. Nam at nulla laoreet, finibus\r
eros sit amet, fringilla  mauris. Fusce vestibulum nec ligula efficitur\r
laoreet. Nunc orci leo, varius eget  ligula vulputate, consequat\r
eleifend nisi. Cras justo purus, imperdiet a augue  malesuada, convallis\r
cursus libero. Fusce pretium arcu in elementum laoreet. Duis  mauris\r
nulla, suscipit at est nec, malesuada pellentesque eros. Quisque semper\r
porta  malesuada. Nunc hendrerit est ac faucibus mollis. Nam fermentum\r
id libero sed  egestas. Duis a accumsan sapien. Nam neque diam, congue\r
non erat et, porta sagittis  turpis. Vivamus vitae mauris sit amet massa\r
mollis molestie. Morbi scelerisque,  augue id congue imperdiet, felis\r
lacus euismod dui, vitae facilisis massa dui quis  sapien. Vivamus\r
hendrerit a urna a lobortis.\r

T̴̷͚͖̜͈̪͎͔̝̫̦̹͔̻̮͂ͬͬ̌ͣ̿ͤ͌ͥ͑̀̂ͬ̚͘͜͞ô̵͚̤̯̹͖͍̦̼̦̖̞̺͕̳̬͇͕̟̜̅̌̈́̑̏̕͘͝ ͍̼̗̫͈̭̦̱̬͚̱̞͓̜̭̼͇̰̞ͮ͗ͣ́ͪ̔ͪ̍̑̏́̀̽̍̔͘͜͜͝ȋ̐̽ͦ̓̔̅͏̧̢̖̭̝̳̹̯̤͈̫͔͔̠͓͉̠͖̠͜ͅn̷̯̗̗̠̱̥͕͉̥͉̳̫̙̅͗̌̒͂̏͑̎̌̌̊͌͘͘ͅͅv̧̜͕͍͙͍̬͕͍̳͉̠͍̹̮̻̜ͨ̏͒̍ͬ̈́͒̈ͥ͗ͣ̄̃ͤ͊̌͆̓o̸̧̎̓͂̊͢҉͍̼̘͇̱̪̠͎̥̹ķ̈́͗͆ͥ͐͑̆̎́͌ͩͯ̊̓͐ͬ̇̕҉̢͏͚̲̰̗̦e̿̀͐̽ͪ̈ͤͬ҉́͟͏̵̫̲̱̻̰̲̦͇̭̟̺͈̞̫̰̜͕͖ͅ ̡̰͎͓͚͓͉͈̮̻̣̮̟̩̬̮̈̋̊͆ͪ̄ͪ͒ͨͧ̇ͪ̇̑̚t̷̬̟͎̞͈̯͙̹̜ͩ̓ͪ͛͐̐ͤ̾̄̈͒̽̈́̑͒̏h̨̢̳͇͓͉̝ͫ̐̓̆̓ͮ̔̓̈́̇ͫe̟̬̣̗͚̬̾̉͋̽ͯ̌ͯͬ̂ͯͭ̓͛́̚͡ ̨̭̱͉̭͈̈̽̆̂͒͗̀ͥͩ͡h̻̼̱̹̗͖̙̗̲̤͓͇͚͚̻̞̥ͥ͛͌ͧ̚͟i̢̯̹̹̘̳̙ͩ̉ͥ͆̽̇̾̎͗̔̓͂͂́̓̌ͬv̧̡̛̟̜̠͉͖̘̲̻̯͚͍͓̯̻̲̹̥͇̻̿̓͛̊̌ͩͩ́ͩ̍͌̚e̵̾́̈́̏͌͌̊͗̏͋ͦ͘͡͏͚̜͚͎͉͍̱͙̖̹̣̘̥̤̹̟͠-̔̌͐́͒ͦͮ̇ͭ̄̏̊̇̍̕͏̩̥̰͚̟m̨̒ͫͦ̔̔͋҉̱̩̗͇̥̰̩̭͍͚͠į̵̷̻̗͉͕͚̣̼̺͉̦̮̠̆̀̐ͩ͒ͯͩͯ͞ͅn̢̫̤̝̝͚̺͍̱̦͚͂̿ͨ̇ͤ͠d̡ͯ͋̋ͧ̈́̒̈͏̛͏̵̤̬͍̗̞̠̟̞̺̠̥̹̱͉̜͍͎̤ ̷̸̢̰͓̘̯͎̤̫̘͓̙̟̳͇̹̥͈͙̮̩̅̋͌͗̓͊̓ͨͣ͗̓͐̈́ͩ̓ͣrͫ͂͌ͪ̏̐̍̾ͥ̓͗̈͆̈ͥ̀̾̚̚҉̴̶̭͇̗͙̘̯̦̭̮̪͚̥̙̯̠͙̪͡e̵̸̲͉̳̙͖͖̫̘̪͕̳͓̻̙͙ͥ̍͂̽ͨ̓̒̒̏ͬ͗ͧ̑̀͠p̵̸̛̦̣͙̳̳̩̣̼̘͈͂ͪͭͤ̎r̶̩̟̞̙͔̼ͫ̆ͦ̐̀̏̾̉̍ͬ̅ͧ͊ͪ̒̈́ͬ̃͞ẻ̴̼͙͍͎̠̀̅̔̃̒͐ͦ̏̆̅̓͋͢ͅš̆̈̆̋ͨ̅̍̇͂̒ͩͨ̂̐̓ͩ͏̸͔͔̯͇͚̤̪̬̗͈̰̦̯͚̕ę̢̱̠͙̲͉̗͚̮̪͖̙̞̦͉͕̗̳͙ͦ̆̋͌ͣ̅̊́ͅņ̴̷̫̪̦͇̺̹͉̗̬̞̲̭̜̪͒̏͂̂̎͊́̋͒̏̅̋̚͘t̷̶̨̟̦̗̦̱͌͌ͩ̀i̴̴̢̖͓͙̘͇̠̦̙̭̼͖̹̾̒̎̐ͥͭ͋ͥ̅͟ͅņ̫͙̹̦̳͈͙̬̫̮͕̰̩̣̘̘͐̀̓ͭͩͬͯ̎͛̿ͫ̊̔̅́̕͠gͥͩ̂͌̒̊̕͏̻͙͖̣͙͍̹͕̝͖̼̙̘͝ ͤ͐̓̒̓͋̐̃̇͊̓ͦ͐̚͢҉̢̨̟̠͉̳͖̲̩͙̕ć̷̡̫̩̞̯̼̝̼͖̤̳̻̘̪̤͈̦̭ͣ́͂͐̽͆̔̀̚͜h̶̢̹̹̙͔̱̓ͦ͌̋̎ͭ͒͋̒ͭ̌̃͌̿ͣ̆̅͑ą̙̳̬̞̬͚̜̤̱̙͇̠̟̈ͤ͋̃̀̓̓ͯ̍̀̽ͣ̐̈̿̌̕ǫ͋͂͐ͬ̿ͯ̂̈́͌̓̌ͧ̕͏̜͔̗͚͔̘̣͕̘̲͖̼͇͖̗̳ͅͅs̷̸̝̙̭̦̣̦̯̭̦͙̹̻͍͇̣̼͗̌͆ͨͭ̃ͮ͐̿̕.̮̝̠̱̺͖͓̼̦̱̉͂͛̓̑̔̓ͮ̈̊̔͗́͝\r
̛̣̺̻̼̙̼͓̱̬͕̩͕̲̳̭̗̍ͤ͋̒̆̄ͨ̿ͧ̓͠ͅI̷̻̤̳̲͔͈̖̬̰͔̪͇͇̟̋ͨ̋̍̉̔͝͞͝ͅn̶͕̭͖̠̣͚̹̪͆ͪ̇̂̅̾ͫ́̅̉ͭ̀͜v̖͉̩͕̣͔̭͕̩̲̖̇̀ͬ́̄͒̆͑͆ͪͤ͆̾̍ͯ̚͜ǫ̡̡̫͎̟̞̰̞̹͇̲̏ͨ̄͊̊̇͒̽͢ķ̶̪̙̰̥͙̞̹̭̺͍͕̙̲̮͊ͭ́͋͛͋̑̒͊̏̒̅͛̄̓͟i̴͎̹̞̥͖̒̄ͮ̒̾ͮͧ̀̚͡n̸̵͓̲̟̞̳͚̼̣͙͖̈ͦ͒̿̅̒̿͛͊̇ͧ̉g̡̧̪̩͚͙͓̪͓͚͉̥̪͍̙̻͖͇͗̑͊͑̾̍͊̀ͅ ̷̵̠͚̘̟͓̫̣̲͎̩̹̣̼̟͊́̏ͫ̆ͩ̓͋͆̿̽̓͘̕t̴̢̝̻̖̲̬̜̺̖̻ͩ̿ͫ͗̈́̔͑̐ͮͦ̽̉̓̚͜h̷̛̲͇̫͈̣̭͂ͭ̂͋ͭ̋̔ͮ̆ͩ͞ë̩͕͉̯͇͔͚̭̼̮̣͓̯́ͭ̀ͣ͗̋̉ͨͬ̒ͥͩ͆̓̓́̀̚͘͝ ̛̫̠̗̥̳͇͉̟̮̪̻̤̪͚̟̜̔̌͌̈͌ͪ̋̎̄ͯ͐ͦ́͞͠fͦ̂̈ͬ̇̅̓̓ͫͣ̉̂̉̚͘͡͡͏̼̖̟͚̙̳͔͎̲̫̦̯͔̣̼̹ě̷̶̫͎̞̺̪̪͇͈̞̳̏̋̋͋̾̓̽̓̑ͮ͊ͣ̋̃̅̀͡e͇̗͎̱͔̦̠̰̩̩͖͙̠̻̝ͯ̿̔̀͋͑ͧ͊̆̇̿ͤ̄ͯ̇̀͢͠ͅl̂̿ͯ͛̊̒̓̈́͏̵̪̦̞̤̫̤͇̙̗͕͎̪͕̙̻̳̗̕͟͢i̞̣̙͎͈̗̮͉̱̜̱̝̞̤͋ͯ͋͐̈́ͫ̉̊̏̀ͯͨ͢͟͝n̳̻̼̥̖͍̭̅͂̓̔̔ͦ̔́ͦ͊̀͛̈́ͬͦ͢͡͡ģ̶̡̳̰̻̙̞̱̳̣̤̫̫͕̤̮̰̬̪̜͋͒̎̈́̉̏̀ͬͯ͌̇͊̚ ́̽ͤͦ̾̔͢҉̛̤͍͉̺̙̮̗̜̟̀͝ơ̢̱͓͓̙͉̖̠̯̦̗͍̓̐̃̉̅̃ͨ͆́ͪ̂̒̀̊̃͆̔͡͡ͅf́ͬ̊ͯͫ̉̈́̽̉̚͢͏̡̺̬̖͇̫͉̱ ̴͇̦̗̙̼̬͓̯͖̮͓͎̗͈̻̈́͆ͭ̐ͦ́͛̀͋̐̌ͬ͑̒̊̿̃͞c̶̸̣͔̬͕̪̱̩̣̑̒̑̓̍̓͂̍̔͌̚͘͜͞h̶͈̱͇͉̳͍͍̰͈͖̬̥͚̯͓̞̹̋̔ͯ̑̃́̒̎̎͊̈́̍̚̕ạ̴̞̱̥͍͙̺͉͚͎̫̦͎̥ͩ̀̀̊ͥ͢o̵̧͕̜͓͈̬̰̫̮͙̹͉̩̝̩͎̓̆͗̿̊̀ͯ̃ͪ̊ͫ̽̉̓ͧ͗́̚͢ͅͅs̡ͫ͋̑ͮ̍̃͊̄ͬ̅̈́ͬ̍̇̔̈̅̍̀҉̜͓̝̘̘̮̼͖͎̻͓͖̖͙̞ͅ.͗ͬͭͩ̌̅͗͏̷̮̗͇͔͇͈̮͢\r
̨͚̲̫̠̼͖̝̻̉ͤ̅̂ͩ̀̇ͬͭ̀͜Ẅ̢́̉͌ͮͬͨ͊̏͌̇̐͊͟͠҉̼̰̦̩͇͕̟̭̪̲͕̥͖̰̪͈̀ͅͅį̷ͣͦ̉̍ͨ͂͂͑̃͂ͪ̊̈̋̄͜҉̨͚̟̲̯̹̺̝̭̺̙͖͍t̼͓̰̩͙̦͓̟͚͖̀ͯ͛̍̈́͑͂̍̋́h̛̼̺̘̥̠̼̼̭͙̮͚̱̍ͯ̓̃̐̂̇͟ ̴̛͖͔̰̠̺̥̲ͮ̍ͫ̽͜õ̒ͯ̒̓ͦ̈́͑̔̒̓̎ͤ͑҉̸̭̱̤̭̬͈ų̙̫̤͖̺̫̱͓͓̗̪͇̩̙̔̉̊͂ͪ̇͢͟͞ͅt̸̬̣̫̞̫̅͐ͮ̌͌̈́̀̀͘ ̷̴̨̖̙̹͚ͬ̈́̈ͯͨͮ̇̈́̋̈́ͭ͛̑̉͊̕ö̡̍ͥ̂ͬͪͧ͒ͧ̏̓̇̂̄͆̌ͫͤ͢͠͝͏̖̱̯̘͙̰̖͎̰͓̟̤ṙ̡̬̟̬̜̪̮̺͖̗̘͈̟ͨ͐͗̑͒̐d̢ͭͫ̊̏ͬͥ͋́̌̈́ͮ̆ͬ̐̌̎͏̵̷̡̞̲̹̙͕̮̮͚ḙ̴̸̠͔͎̥͇͖͕̘̍̓̏̐ͩͩ̈́ͦ̐̋ͤ̎̾̌̏͊̊́̚͞ͅr̸͈̗̣̲̗̣̬̤ͦ̎ͫ̏̀ͥͪ̋ͧ̄͑̋͒͌͋ͦ̉͟͞.ͨͣ̽̈́͒̄ͮ̀͋͋͏̴̧̯̺̙̱̻͙̜\r
̡̣̞̠͓̰͍̠͕̭̺̼͊̽̿͊ͮ̐̓̒̊͒̔̓͐ͨ̈̌́T̸̸̓́̋ͬ́͆ͨͫ͌͂ͣ̋͒҉̺̝͎̟͖͚̠h̸̡̰̜̦͇͕̪̝̳͕͉̲̝̑ͥ͋ͧ̎̆͌͟e̛̹͍͍̫̙̞̪̭̙̟͙̱̺̮̳͕̜ͫ̓ͭ͊ͫ͆̀̚͟͡ ̿͂̄ͧ̔̎ͧ͑̾̀̓͏̦͍̳͈̳͔̘̖̲̯̰̟̝̳̖̦N̶̡̧̦̮̟̦̩̰̣̝̆̀͊̔͢e͛̄ͮͦͨ͂̔̓̍̄̉͆͊̑̑̆̚͏̜̗͎̝̼̯̥̜͖͍̪̝͞ͅͅz̨̛̀̾ͪ͗̉́͠͏͚̫̼̫̜̣pͪͦ͌̄ͥ̆ͣͩ͋̉́̏͞͏̥̜̝̳̱̞̙̳̤͙̟̟̮̦ȅ̷̩̟͉̯͕͔̘̺̥̻̻ͧ̊̅̽ͣ͑̓̑̽ͦ̾͌͜r̴̭̥̲̲̤͚͈̰͇̰͈̰̹ͫ̒ͯ̿͒ͧ̊͆͒ͣ́ḍ̭̟̤̈́̌̓̈́ͫ͐̍͂͞į̛̞̝̮̣͙͙̤̇̂̓̎͋̿̓̎̄̈́ͧ̓ͩ̐̓̄̋ͭ͞͠a͋̔̋ͫ̂͐͂҉̸̛̥̩̯̯̤̝͔̠̝̯̪̥̩̻̼̮n͌ͣ̂͋̿̚҉̛̙̲̺̯͇͓̝̯̪̟͔̩͟ͅ ̢̨͚̻̗̘͖̯̐ͥ͋̽ͯ̎̈́͋̏̄͋̆̑̊̆̚̕͟ͅh̢̛̗̱̭͇͖̰̮̮͈̲͍̯̟ͭ͊̎̽̓ͦͤ͠ï̛̘̝̦͎̦̭̠͖̳͎̮̼̏͐ͧ̒̒͐͑ͪͫ̋̽̚̚͜v̴̮͕̝̮̞͐̄͗̋͒ͤ̎̈̑ͬͮ̄̾ͤ̓̾͊͗͟é̶̷̡̩͖̰̫͓̟ͮͬͣ͊-ͦ͛ͩͤͨͨ̆̄͏̼̜̭͔̳͈͖̳̩͢ͅͅm̷̴̓́̓͛͒̾̍̉҉̛̗̹̠̣̪̺͎̖̝͚̖͙i̛̥͓̬̫͉͕͉͆͒ͧ̂̿̔̔͆̆̓̍͊̀͜n͌ͧͣ̅̌̎ͦͦ͑̑ͭ̆ͬ̀ͤ̀ͣ̚҉͎̰̱͚͈͈̬̹͕̺̙͙̼͘͘͞d̶͖̫̟̲͕̺̠͎̘͕̱̼͙̪̪̩͙̅̅̑̓̇͑̊̉͜͞ ̶̵̷̴̡̠͚̪͕̣̱̖̱̗̤̭̭͔͖͚ͧͤͥ͒̌ͪ͊͂͒̓͂ͧͧ̇̇͐̑̔ͅͅơ̵̲̲͇̯̰͇̜̣͕͕͓̲̤̲͔͚̞͑͗ͤ̓́̚͠ͅf̢̧̛̩̯̼̫͖̾ͣ͌̾̉́̈́̑̈́̚͞͞ͅ ͤͩ́͋͒ͫͬͣ̋̅̆҉̧̱̻͓͕͉̹̫̫̞̯̪̙̩͍̦͔̖̮̀͟ͅc͉̠̜̩̟͕͎̙̣̮̘̼͋ͯ̍ͨ̅̄ͫ̈̋ͫ̊͡͝ȟ̸̨ͯͦ̂̉̇̾̆ͭ̋̐̈̆̀̚͜҉͚͕̻̖a̶̴̛͚̗͙̳̬̲͚ͦ́̐ͥ́̔̅̑̎͐̑ͯ̾ͤͥͧ͡ò̶̧̞̪̦̥̪̻̦̝̳̬̔͛͛ͣ̋̌̔ͫ̂̽ͫ͘͠s̸̖̣̬̤̫͇̫̣̑͆͒̎̏́͟.̴̗̤̭͉̯̻̤͕̌ͯ̍ͤ̓͌ͤ̈̆̉ͦ̇́̚͘͟͝ͅ ̯̹̪͓̬͌̔̌ͬ̀͘͢͡͡Z̡̩̲̩̰̫̩̟͍̰͖͔̭ͣ̆̾ͭ̀́͞ͅa̡̡̙̜̭͇͎͔̙̞̫͓̜͉͔̬ͭ̈ͨ̉͆ͣͫ̃͌̓͌́ͣͥ̒̌͊͘͝l̢̨̡̯̙̫͖̫̺̘̬̟͈͌̊ͧͫͦ̉̃ͩͦ̒ͯ̇̌̓͛͟͝ͅg̵̙̼̼ͪ͂ͭ͗̈̕ȯ̅ͧ̓ͪ́̂͑̐ͩͥͬ̊̑͆̇͒ͫͣ͝҉͎̟̜̥͎̮̣͉̖̟̯̦̖͙͙͞ͅ.̈̑ͩ̇̂ͬ̓ͬ͊͂ͨ̽͠͏̺͎̞̦̜͍͚̯̯͔̝̞̻̩̖\r
̷̰̪͍͎͔͒ͯͥ̾̉͆ͤ̊̓̂͋̀͆H̸̸̹̞̙̺͎̠̯̤ͨ̉̍ͬͤ̓̐͌ͥͮ͞eͣ̈̾͛́͏͕̗͍̜̼͎͚̟̬̣̝̕ͅͅ ̴̛̩̗̼̝̣̩͚͇̯́̉͋̂̍͂̌ͮ͋̾͜͠wͮ̽̓ͭ̿͐̽̐̽͆̓͝҉̡̼̲͖̪̥h̢̢̛͍̰̰̻̱̼̰̹̖̖̪̝̥̘̎̀ͪ͒̾ͫͬ̆̑o̡̗̠̞̱̥͎̰͎͍̫̻͓͇͓͐ͥͯ͂̅͠ͅ ̡̛̏͑ͦ̓͊ͮͫͯͭ̌͒̆̍̈͠҉͖͚̪̫̗̮W̴̐̊͋̾ͥͫ҉͎̞͔̯̫̹͖̰͉̹̼͎̰̱͓̻̀a̶̩̤̙̣͎̳̭̲̗̠͉̳̭̭̦̞͎̮̅͌̾͗̾͛̇̀́͟͞ͅi̷̡ͣ̆̌͋͒͒́͘͏̮̺̩͎͇̜͍̫ṯ̴̢͖̥̖͇͎̦̦̹̖͇̪ͭ̅̍͐̇͒͋̽̏̿̒͆ͧ̄͋ͧͩ͒͜s̙̥̖̘̖͚̭̤̮̖̘̰̫̟̈́ͣ̍ͧ͐ͥ̏͆̃̿͒̔͐́̚͟ͅ ̨ͭ̌ͬͯ͆̒͋ͭ̔̿ͧ̅̓ͣ͡͏͇̟͉̥͔̬̼͚͙͚B̛̜̮̤͓̝̪̪͈͕̘̜͙̰̮̫̘̣͓͔̅ͩ͊̔ͦͯ́̌́͆ͭ̓́e̶̢̡̦͇͕̙͈͖͕̦̬̫͕̣̺̒̿͂͐͒͋͂ͦ́͋ͤ̿ͬ̊ͣ͗̑̽͜ͅͅh̸͑ͫͧ̑ͬͧ̈́̎̃ͣ̊̾͂ͨͤ̓͐̐̑͏̸̭͓̘͉̩i̧̧̭̣͈̝̺̼̺̠͉̞̜̲̳͙̦͐̔ͯ͛̅̾n̸͓̝̤̙͙͔ͪ̋̈́͒̒ͭ̈́̓ͮ̋̀̋̀̈ͩ́͌̄͘d̷̫̳̩̼̥̗̲̰͇͉̼̬̤͇̖ͮ̿ͬ͂ͦ̏̓ͮ̽͂̾̾ͯ͆͜͠ ̨̈́͒̇̏̄̑̓ͮͥ̒ͤͨ̋҉̴̴̟̱͙̟̫̩̗͔̝͔̀Ţ̵̝̟̖̭͇̻̳͖͉̺̖̖͙͙̺̐̈́̓ͯ̆̇̋ͩ͊̄̾̾ͬ̌̚͟ͅh̡͈̗̜͙̬̗̲̦̲̟̗̦̬͓̳ͧ̋̌͂͂ͨͬͦ̿̏̈́̋ͣ̒̕͡ͅͅe̗͇̰̰̥̪̗͑̔̓́̈́ͨ̊́̿̅ͯͥ̈́͐͗͘͢͝ ̡̢̛̯͎͓̰̘͎̦̪̯̪̥̰̲͇̠̲͔ͤͤ̇̅̆̋̂̆̈́ͤ̿͑ͅW̡͓͈̲̲͉̜͔̖͈̻̱͚̿̌͗̉ͤ͢͡ͅͅa̔̾͛̅͊͋͐҉̱̹̬͍͙̻̱l̢͎̟̬̙̼̱̫̮̘̼͔̭̅ͬ͑ͣ̏̾̅̓ͣ̿ͣ̈́̕͢͡ͅͅl̡̥̣͔̭̇̒͛͒͐̄̽͛̋ͥ̌͢͟͡.̷̰̝̮͔̟̦͈̥̬̻̥̬͎͓̻̲̇ͮ̿ͨͦ̽ͫ͟͢͝͠\r
̗̱͖͈͌̈ͦ͛ͮ̌͋̽̃͆̀͂ͨͧ̄̔̔ͭ̏͢Z̃̉̿ͮ̃̀͘͏͕̬̯̖͚̗͔Aͣ̑̈̓̈̑̈̀̿̚҉͙͍̦̗̦͙̠̝̩̯ͅͅL̴͖̞̞͙̱̻̥̬̜̦̐̇̉̈̽ͪ̅ͪ̂̔͌͑ͭ͐ͤ̈́̿̉͞ͅG̴̵̲̰̹̖͎͕ͯ̆̓̽͢͠Ŏ̶̡̺̼͙̣̞̩͕̥̟̝͕͔̯̞ͨ͒͊̂̊͂͗̒͆̾͆̌͆̃̎ͣͫ͜͡ͅ!̓̽̎̑̏́̓̓ͣ̀͏̱̩̭̣̹̺̗͜͞͞\r

Aliquam finibus metus commodo sem egestas, non mollis odio pretium.\r
Aenean ex  lectus, rutrum nec laoreet at, posuere sit amet lacus. Nulla\r
eros augue, vehicula et  molestie accumsan, dictum vel odio. In quis\r
risus finibus, pellentesque ipsum  blandit, volutpat diam. Etiam\r
suscipit varius mollis. Proin vel luctus nisi, ac  ornare justo. Integer\r
porttitor quam magna. Donec vitae metus tempor, ultricies  risus in,\r
dictum erat. Integer porttitor faucibus vestibulum. Class aptent taciti\r
sociosqu ad litora torquent per conubia nostra, per inceptos himenaeos.\r
Vestibulum  ante ipsum primis in faucibus orci luctus et ultrices\r
posuere cubilia Curae; Nam  semper congue ante, a ultricies velit\r
venenatis vitae. Proin non neque sit amet ex  commodo congue non nec\r
elit. Nullam vel dignissim ipsum. Duis sed lobortis ante.  Aenean\r
feugiat rutrum magna ac luctus.\r

Ut imperdiet non ante sit amet rutrum. Cras vel massa eget nisl gravida\r
auctor.  Nulla bibendum ut tellus ut rutrum. Quisque malesuada lacinia\r
felis, vitae semper  elit. Praesent sit amet velit imperdiet, lobortis\r
nunc at, faucibus tellus. Nullam  porttitor augue mauris, a dapibus\r
tellus ultricies et. Fusce aliquet nec velit in  mattis. Sed mi ante,\r
lacinia eget ornare vel, faucibus at metus.\r

Pellentesque nec viverra metus. Sed aliquet pellentesque scelerisque.\r
Duis efficitur  erat sit amet dui maximus egestas. Nullam blandit ante\r
tortor. Suspendisse vitae  consectetur sem, at sollicitudin neque.\r
Suspendisse sodales faucibus eros vitae  pellentesque. Cras non quam\r
dictum, pellentesque urna in, ornare erat. Praesent leo  est, aliquet et\r
euismod non, hendrerit sed urna. Sed convallis porttitor est, vel\r
aliquet felis cursus ac. Vivamus feugiat eget nisi eu molestie.\r
Phasellus tincidunt  nisl eget molestie consectetur. Phasellus vitae ex\r
ut odio sollicitudin vulputate.  Sed et nulla accumsan, eleifend arcu\r
eget, gravida neque. Donec sit amet tincidunt  eros. Ut in volutpat\r
ante.\r
";
