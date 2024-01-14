#![cfg(not(miri))]

use proptest::test_runner::Config;

use ropey::Rope;

#[cfg(feature = "metric_lines_lf")]
use str_indices::lines_lf;

#[cfg(feature = "metric_lines_cr_lf")]
use str_indices::lines_crlf;

#[cfg(feature = "metric_lines_unicode")]
use str_indices::lines;

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
use ropey::LineType;

fn closest_char_boundary(text: &str, mut byte_idx: usize) -> usize {
    byte_idx = byte_idx.min(text.len());
    while !text.is_char_boundary(byte_idx) {
        byte_idx += 1;
    }
    byte_idx
}

fn string_insert(text: &mut String, byte_idx: usize, text_ins: &str) {
    text.insert_str(byte_idx, text_ins);
}

fn string_remove(text: &mut String, byte_start: usize, byte_end: usize) {
    let text_r = text.split_off(byte_end);
    text.truncate(byte_start);
    text.push_str(&text_r);
}

fn assert_metrics_eq(rope: &Rope, text: &str) {
    assert_eq!(rope.len_bytes(), text.len());

    #[cfg(feature = "metric_chars")]
    {
        assert_eq!(rope.len_chars(), str_indices::chars::count(text));
    }

    #[cfg(feature = "metric_utf16")]
    {
        assert_eq!(rope.len_utf16(), str_indices::utf16::count(text));
    }

    #[cfg(feature = "metric_lines_lf")]
    {
        assert_eq!(
            rope.len_lines(LineType::LF),
            str_indices::lines_lf::count_breaks(text) + 1
        );
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    {
        assert_eq!(
            rope.len_lines(LineType::CRLF),
            str_indices::lines_crlf::count_breaks(text) + 1
        );
    }

    #[cfg(feature = "metric_lines_unicode")]
    {
        assert_eq!(
            rope.len_lines(LineType::All),
            str_indices::lines::count_breaks(text) + 1
        );
    }
}

//===========================================================================

proptest::proptest! {
    #![proptest_config(Config::with_cases(512))]

    #[test]
    fn pt_from_str(ref text in "\\PC{0,200}") {
        let rope = Rope::from_str(text);

        assert_eq!(rope, text.as_str());
        assert_metrics_eq(&rope, text.as_str());
        rope.assert_invariants();
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    #[test]
    fn pt_from_str_crlf(ref text in "(\\u{000A}|\\u{000D}|\\u{000A}\\u{000D}){0,200}") {
        let rope = Rope::from_str(text);

        assert_eq!(rope, text.as_str());
        assert_metrics_eq(&rope, text.as_str());
        rope.assert_invariants();
    }

    #[test]
    fn pt_insert(idx in 0usize..(TEXT.len()+1), ref ins_text in "\\PC{0,50}") {
        let mut rope = Rope::from_str(TEXT);
        let mut text = String::from(TEXT);

        let idx = closest_char_boundary(TEXT, idx);

        rope.insert(idx, ins_text);
        string_insert(&mut text, idx, ins_text);

        assert_eq!(rope, text.as_str());
        assert_metrics_eq(&rope, text.as_str());
        rope.assert_invariants();
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    #[test]
    fn pt_insert_crlf(cr_or_lf: bool, idx: usize, ref start_text in "(\\u{000A}|\\u{000D}|\\u{000A}\\u{000D}){0,200}") {
        let mut rope = Rope::from_str(start_text);
        let mut text = String::from(start_text);

        let idx = closest_char_boundary(start_text, idx % (start_text.len() + 1));
        let ins_text = if cr_or_lf { "\r" } else { "\n" };

        rope.insert(idx, ins_text);
        string_insert(&mut text, idx, ins_text);

        assert_eq!(rope, text.as_str());
        assert_metrics_eq(&rope, text.as_str());
        rope.assert_invariants();
    }

    #[test]
    fn pt_remove(idx1 in 0usize..(TEXT.len()+1), idx2 in 0usize..(TEXT.len()+1)) {
        let mut rope = Rope::from_str(TEXT);
        let mut text = String::from(TEXT);

        let idx_left = closest_char_boundary(TEXT, idx1.min(idx2));
        let idx_right = closest_char_boundary(TEXT, idx1.max(idx2));

        rope.remove(idx_left..idx_right);
        string_remove(&mut text, idx_left, idx_right);

        assert_eq!(rope, text.as_str());
        assert_metrics_eq(&rope, text.as_str());
        rope.assert_invariants();
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    #[test]
    fn pt_remove_crlf(cr_or_lf: bool, idx: usize, ref start_text in "(\\u{000A}|\\u{000D}|\\u{000A}\\u{000D}){0,200}") {
        let mut rope = Rope::from_str(start_text);
        let mut text = String::from(start_text);

        let idx = closest_char_boundary(start_text, idx % (start_text.len() + 1));
        let ins_text = if cr_or_lf { "\r" } else { "\n" };

        rope.insert(idx, ins_text);
        string_insert(&mut text, idx, ins_text);

        assert_eq!(rope, text.as_str());
        assert_metrics_eq(&rope, text.as_str());
        rope.assert_invariants();
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn pt_byte_to_line(ref text in "(\\u{000A}|\\u{000D}|\\u{000A}\\u{000D}|\\u{2028}){0,200}") {
        let rope = Rope::from_str(text);

        for i in 0..=text.len() {
            #[cfg(feature = "metric_lines_lf")]
            assert_eq!(lines_lf::from_byte_idx(text, i), rope.byte_to_line(i, LineType::LF));
            #[cfg(feature = "metric_lines_cr_lf")]
            assert_eq!(lines_crlf::from_byte_idx(text, i), rope.byte_to_line(i, LineType::CRLF));
            #[cfg(feature = "metric_lines_unicode")]
            assert_eq!(lines::from_byte_idx(text, i), rope.byte_to_line(i, LineType::All));
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn pt_line_to_byte(ref text in "(\\u{000A}|\\u{000D}|\\u{000A}\\u{000D}|\\u{2028}){0,200}") {
        let rope = Rope::from_str(text);

        #[cfg(feature = "metric_lines_lf")]
        {
            let line_count = lines_lf::count_breaks(text) + 1;
            for i in 0..=line_count {
                assert_eq!(lines_lf::to_byte_idx(text, i), rope.line_to_byte(i, LineType::LF));
            }
        }
        #[cfg(feature = "metric_lines_crlf")]
        {
            let line_count = lines_crlf::count_breaks(text) + 1;
            for i in 0..=line_count {
                assert_eq!(lines_crlf::to_byte_idx(text, i), rope.line_to_byte(i, LineType::CRLF));
            }
        }
        #[cfg(feature = "metric_lines_unicode")]
        {
            let line_count = lines::count_breaks(text) + 1;
            for i in 0..=line_count {
                assert_eq!(lines::to_byte_idx(text, i), rope.line_to_byte(i, LineType::All));
            }
        }
    }

    #[test]
    fn pt_chunks_iter_next(ref text in "\\PC{0,200}") {
        let r = Rope::from_str(text);

        let mut idx = 0;
        for chunk in r.chunks().flatten() {
            assert_eq!(chunk, &text[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }

        assert_eq!(idx, text.len());
    }
}

//===========================================================================

// 5815 bytes, 3398 chars, 19 lines
// Contains many long graphemes.
const TEXT: &str = "
T̴̷͚͖̜͈̪͎͔̝̫̦̹͔̻̮͂ͬͬ̌ͣ̿ͤ͌ͥ͑̀̂ͬ̚͘͜͞ô̵͚̤̯̹͖͍̦̼̦̖̞̺͕̳̬͇͕̟̜̅̌̈́̑̏̕͘͝ ͍̼̗̫͈̭̦̱̬͚̱̞͓̜̭̼͇̰̞ͮ͗ͣ́ͪ̔ͪ̍̑̏́̀̽̍̔͘͜͜͝ȋ̐̽ͦ̓̔̅͏̧̢̖̭̝̳̹̯̤͈̫͔͔̠͓͉̠͖̠͜ͅn̷̯̗̗̠̱̥͕͉̥͉̳̫̙̅͗̌̒͂̏͑̎̌̌̊͌͘͘ͅͅv̧̜͕͍͙͍̬͕͍̳͉̠͍̹̮̻̜ͨ̏͒̍ͬ̈́͒̈ͥ͗ͣ̄̃ͤ͊̌͆̓o̸̧̎̓͂̊͢҉͍̼̘͇̱̪̠͎̥̹ķ̈́͗͆ͥ͐͑̆̎́͌ͩͯ̊̓͐ͬ̇̕҉̢͏͚̲̰̗̦e̿̀͐̽ͪ̈ͤͬ҉́͟͏̵̫̲̱̻̰̲̦͇̭̟̺͈̞̫̰̜͕͖ͅ ̡̰͎͓͚͓͉͈̮̻̣̮̟̩̬̮̈̋̊͆ͪ̄ͪ͒ͨͧ̇ͪ̇̑̚t̷̬̟͎̞͈̯͙̹̜ͩ̓ͪ͛͐̐ͤ̾̄̈͒̽̈́̑͒̏h̨̢̳͇͓͉̝ͫ̐̓̆̓ͮ̔̓̈́̇ͫe̟̬̣̗͚̬̾̉͋̽ͯ̌ͯͬ̂ͯͭ̓͛́̚͡ ̨̭̱͉̭͈̈̽̆̂͒͗̀ͥͩ͡h̻̼̱̹̗͖̙̗̲̤͓͇͚͚̻̞̥ͥ͛͌ͧ̚͟i̢̯̹̹̘̳̙ͩ̉ͥ͆̽̇̾̎͗̔̓͂͂́̓̌ͬv̧̡̛̟̜̠͉͖̘̲̻̯͚͍͓̯̻̲̹̥͇̻̿̓͛̊̌ͩͩ́ͩ̍͌̚e̵̾́̈́̏͌͌̊͗̏͋ͦ͘͡͏͚̜͚͎͉͍̱͙̖̹̣̘̥̤̹̟͠-̔̌͐́͒ͦͮ̇ͭ̄̏̊̇̍̕͏̩̥̰͚̟m̨̒ͫͦ̔̔͋҉̱̩̗͇̥̰̩̭͍͚͠į̵̷̻̗͉͕͚̣̼̺͉̦̮̠̆̀̐ͩ͒ͯͩͯ͞ͅn̢̫̤̝̝͚̺͍̱̦͚͂̿ͨ̇ͤ͠d̡ͯ͋̋ͧ̈́̒̈͏̛͏̵̤̬͍̗̞̠̟̞̺̠̥̹̱͉̜͍͎̤ ̷̸̢̰͓̘̯͎̤̫̘͓̙̟̳͇̹̥͈͙̮̩̅̋͌͗̓͊̓ͨͣ͗̓͐̈́ͩ̓ͣrͫ͂͌ͪ̏̐̍̾ͥ̓͗̈͆̈ͥ̀̾̚̚҉̴̶̭͇̗͙̘̯̦̭̮̪͚̥̙̯̠͙̪͡e̵̸̲͉̳̙͖͖̫̘̪͕̳͓̻̙͙ͥ̍͂̽ͨ̓̒̒̏ͬ͗ͧ̑̀͠p̵̸̛̦̣͙̳̳̩̣̼̘͈͂ͪͭͤ̎r̶̩̟̞̙͔̼ͫ̆ͦ̐̀̏̾̉̍ͬ̅ͧ͊ͪ̒̈́ͬ̃͞ẻ̴̼͙͍͎̠̀̅̔̃̒͐ͦ̏̆̅̓͋͢ͅš̆̈̆̋ͨ̅̍̇͂̒ͩͨ̂̐̓ͩ͏̸͔͔̯͇͚̤̪̬̗͈̰̦̯͚̕ę̢̱̠͙̲͉̗͚̮̪͖̙̞̦͉͕̗̳͙ͦ̆̋͌ͣ̅̊́ͅņ̴̷̫̪̦͇̺̹͉̗̬̞̲̭̜̪͒̏͂̂̎͊́̋͒̏̅̋̚͘t̷̶̨̟̦̗̦̱͌͌ͩ̀i̴̴̢̖͓͙̘͇̠̦̙̭̼͖̹̾̒̎̐ͥͭ͋ͥ̅͟ͅņ̫͙̹̦̳͈͙̬̫̮͕̰̩̣̘̘͐̀̓ͭͩͬͯ̎͛̿ͫ̊̔̅́̕͠gͥͩ̂͌̒̊̕͏̻͙͖̣͙͍̹͕̝͖̼̙̘͝ ͤ͐̓̒̓͋̐̃̇͊̓ͦ͐̚͢҉̢̨̟̠͉̳͖̲̩͙̕ć̷̡̫̩̞̯̼̝̼͖̤̳̻̘̪̤͈̦̭ͣ́͂͐̽͆̔̀̚͜h̶̢̹̹̙͔̱̓ͦ͌̋̎ͭ͒͋̒ͭ̌̃͌̿ͣ̆̅͑ą̙̳̬̞̬͚̜̤̱̙͇̠̟̈ͤ͋̃̀̓̓ͯ̍̀̽ͣ̐̈̿̌̕ǫ͋͂͐ͬ̿ͯ̂̈́͌̓̌ͧ̕͏̜͔̗͚͔̘̣͕̘̲͖̼͇͖̗̳ͅͅs̷̸̝̙̭̦̣̦̯̭̦͙̹̻͍͇̣̼͗̌͆ͨͭ̃ͮ͐̿̕.̮̝̠̱̺͖͓̼̦̱̉͂͛̓̑̔̓ͮ̈̊̔͗́͝\r
̛̣̺̻̼̙̼͓̱̬͕̩͕̲̳̭̗̍ͤ͋̒̆̄ͨ̿ͧ̓͠ͅI̷̻̤̳̲͔͈̖̬̰͔̪͇͇̟̋ͨ̋̍̉̔͝͞͝ͅn̶͕̭͖̠̣͚̹̪͆ͪ̇̂̅̾ͫ́̅̉ͭ̀͜v̖͉̩͕̣͔̭͕̩̲̖̇̀ͬ́̄͒̆͑͆ͪͤ͆̾̍ͯ̚͜ǫ̡̡̫͎̟̞̰̞̹͇̲̏ͨ̄͊̊̇͒̽͢ķ̶̪̙̰̥͙̞̹̭̺͍͕̙̲̮͊ͭ́͋͛͋̑̒͊̏̒̅͛̄̓͟i̴͎̹̞̥͖̒̄ͮ̒̾ͮͧ̀̚͡n̸̵͓̲̟̞̳͚̼̣͙͖̈ͦ͒̿̅̒̿͛͊̇ͧ̉g̡̧̪̩͚͙͓̪͓͚͉̥̪͍̙̻͖͇͗̑͊͑̾̍͊̀ͅ ̷̵̠͚̘̟͓̫̣̲͎̩̹̣̼̟͊́̏ͫ̆ͩ̓͋͆̿̽̓͘̕t̴̢̝̻̖̲̬̜̺̖̻ͩ̿ͫ͗̈́̔͑̐ͮͦ̽̉̓̚͜h̷̛̲͇̫͈̣̭͂ͭ̂͋ͭ̋̔ͮ̆ͩ͞ë̩͕͉̯͇͔͚̭̼̮̣͓̯́ͭ̀ͣ͗̋̉ͨͬ̒ͥͩ͆̓̓́̀̚͘͝ ̛̫̠̗̥̳͇͉̟̮̪̻̤̪͚̟̜̔̌͌̈͌ͪ̋̎̄ͯ͐ͦ́͞͠fͦ̂̈ͬ̇̅̓̓ͫͣ̉̂̉̚͘͡͡͏̼̖̟͚̙̳͔͎̲̫̦̯͔̣̼̹ě̷̶̫͎̞̺̪̪͇͈̞̳̏̋̋͋̾̓̽̓̑ͮ͊ͣ̋̃̅̀͡e͇̗͎̱͔̦̠̰̩̩͖͙̠̻̝ͯ̿̔̀͋͑ͧ͊̆̇̿ͤ̄ͯ̇̀͢͠ͅl̂̿ͯ͛̊̒̓̈́͏̵̪̦̞̤̫̤͇̙̗͕͎̪͕̙̻̳̗̕͟͢i̞̣̙͎͈̗̮͉̱̜̱̝̞̤͋ͯ͋͐̈́ͫ̉̊̏̀ͯͨ͢͟͝n̳̻̼̥̖͍̭̅͂̓̔̔ͦ̔́ͦ͊̀͛̈́ͬͦ͢͡͡ģ̶̡̳̰̻̙̞̱̳̣̤̫̫͕̤̮̰̬̪̜͋͒̎̈́̉̏̀ͬͯ͌̇͊̚ ́̽ͤͦ̾̔͢҉̛̤͍͉̺̙̮̗̜̟̀͝ơ̢̱͓͓̙͉̖̠̯̦̗͍̓̐̃̉̅̃ͨ͆́ͪ̂̒̀̊̃͆̔͡͡ͅf́ͬ̊ͯͫ̉̈́̽̉̚͢͏̡̺̬̖͇̫͉̱ ̴͇̦̗̙̼̬͓̯͖̮͓͎̗͈̻̈́͆ͭ̐ͦ́͛̀͋̐̌ͬ͑̒̊̿̃͞c̶̸̣͔̬͕̪̱̩̣̑̒̑̓̍̓͂̍̔͌̚͘͜͞h̶͈̱͇͉̳͍͍̰͈͖̬̥͚̯͓̞̹̋̔ͯ̑̃́̒̎̎͊̈́̍̚̕ạ̴̞̱̥͍͙̺͉͚͎̫̦͎̥ͩ̀̀̊ͥ͢o̵̧͕̜͓͈̬̰̫̮͙̹͉̩̝̩͎̓̆͗̿̊̀ͯ̃ͪ̊ͫ̽̉̓ͧ͗́̚͢ͅͅs̡ͫ͋̑ͮ̍̃͊̄ͬ̅̈́ͬ̍̇̔̈̅̍̀҉̜͓̝̘̘̮̼͖͎̻͓͖̖͙̞ͅ.͗ͬͭͩ̌̅͗͏̷̮̗͇͔͇͈̮͢\r
̨͚̲̫̠̼͖̝̻̉ͤ̅̂ͩ̀̇ͬͭ̀͜Ẅ̢́̉͌ͮͬͨ͊̏͌̇̐͊͟͠҉̼̰̦̩͇͕̟̭̪̲͕̥͖̰̪͈̀ͅͅį̷ͣͦ̉̍ͨ͂͂͑̃͂ͪ̊̈̋̄͜҉̨͚̟̲̯̹̺̝̭̺̙͖͍t̼͓̰̩͙̦͓̟͚͖̀ͯ͛̍̈́͑͂̍̋́h̛̼̺̘̥̠̼̼̭͙̮͚̱̍ͯ̓̃̐̂̇͟ ̴̛͖͔̰̠̺̥̲ͮ̍ͫ̽͜õ̒ͯ̒̓ͦ̈́͑̔̒̓̎ͤ͑҉̸̭̱̤̭̬͈ų̙̫̤͖̺̫̱͓͓̗̪͇̩̙̔̉̊͂ͪ̇͢͟͞ͅt̸̬̣̫̞̫̅͐ͮ̌͌̈́̀̀͘ ̷̴̨̖̙̹͚ͬ̈́̈ͯͨͮ̇̈́̋̈́ͭ͛̑̉͊̕ö̡̍ͥ̂ͬͪͧ͒ͧ̏̓̇̂̄͆̌ͫͤ͢͠͝͏̖̱̯̘͙̰̖͎̰͓̟̤ṙ̡̬̟̬̜̪̮̺͖̗̘͈̟ͨ͐͗̑͒̐d̢ͭͫ̊̏ͬͥ͋́̌̈́ͮ̆ͬ̐̌̎͏̵̷̡̞̲̹̙͕̮̮͚ḙ̴̸̠͔͎̥͇͖͕̘̍̓̏̐ͩͩ̈́ͦ̐̋ͤ̎̾̌̏͊̊́̚͞ͅr̸͈̗̣̲̗̣̬̤ͦ̎ͫ̏̀ͥͪ̋ͧ̄͑̋͒͌͋ͦ̉͟͞.ͨͣ̽̈́͒̄ͮ̀͋͋͏̴̧̯̺̙̱̻͙̜\r

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
";
