#![cfg(not(miri))]

use proptest::test_runner::Config;

use ropey::Rope;

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
            rope.len_lines_lf(),
            str_indices::lines_lf::count_breaks(text) + 1
        );
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    {
        assert_eq!(
            rope.len_lines_cr_lf(),
            str_indices::lines_crlf::count_breaks(text) + 1
        );
    }

    #[cfg(feature = "metric_lines_unicode")]
    {
        assert_eq!(
            rope.len_lines_unicode(),
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
    }

    #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
    #[test]
    fn pt_from_str_crlf(ref text in "(\\u{000A}|\\u{000D}|\\u{000A}\\u{000D}){0,200}") {
        let rope = Rope::from_str(text);

        assert_eq!(rope, text.as_str());
        assert_metrics_eq(&rope, text.as_str());
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
