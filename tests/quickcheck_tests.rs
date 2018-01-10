extern crate quickcheck;
extern crate rand;
extern crate unicode_segmentation;

extern crate ropey;

use rand::thread_rng;
use quickcheck::{QuickCheck, StdGen};
use unicode_segmentation::UnicodeSegmentation;
use ropey::Rope;

// Helper function used in the tests below
fn char_to_byte_idx(idx: usize, text: &str) -> usize {
    text.char_indices().nth(idx).unwrap_or((text.len(), '\0')).0
}

fn string_insert(text: &mut String, char_idx: usize, text_ins: &str) {
    let byte_idx = char_to_byte_idx(char_idx, text);
    text.insert_str(byte_idx, text_ins);
}

fn string_remove(text: &mut String, char_start: usize, char_end: usize) {
    let byte_start = char_to_byte_idx(char_start, text);
    let byte_end = char_to_byte_idx(char_end, text);
    let text_r = text.split_off(byte_end);
    text.truncate(byte_start);
    text.push_str(&text_r);
}

fn string_slice(text: &str, char_start: usize, char_end: usize) -> &str {
    let byte_start = char_to_byte_idx(char_start, text);
    let byte_end = char_to_byte_idx(char_end, text);
    &text[byte_start..byte_end]
}

fn graphemes_match(rope: &Rope, text: &str) -> bool {
    rope.graphemes()
        .zip(UnicodeSegmentation::graphemes(text, true))
        .all(|(a, b)| a == b)
}

//===========================================================================

#[test]
fn qc_from_str() {
    fn p(text: String) -> bool {
        let rope = Rope::from_str(&text);

        rope.assert_integrity();
        rope.assert_invariants();
        assert!(graphemes_match(&rope, text.as_str()));

        rope == text.as_str()
    }

    QuickCheck::new()
        .gen(StdGen::new(thread_rng(), 1 << 16))
        .tests(64)
        .quickcheck(p as fn(String) -> bool);
}

#[test]
fn qc_insert() {
    fn p(ins_text: String, char_idx: usize) -> bool {
        let mut rope = Rope::from_str(TEXT);
        let mut text = String::from(TEXT);

        let len = rope.len_chars();
        rope.insert(char_idx % (len + 1), &ins_text);
        string_insert(&mut text, char_idx % (len + 1), &ins_text);

        rope.assert_integrity();
        rope.assert_invariants();
        assert!(graphemes_match(&rope, text.as_str()));

        rope == text.as_str()
    }

    QuickCheck::new()
        .gen(StdGen::new(thread_rng(), TEXT.len()))
        .quickcheck(p as fn(String, usize) -> bool);
}

#[test]
fn qc_remove() {
    fn p(range: (usize, usize)) -> bool {
        let mut rope = Rope::from_str(TEXT);
        let mut text = String::from(TEXT);

        let mut idx1 = range.0 % (rope.len_chars() + 1);
        let mut idx2 = range.1 % (rope.len_chars() + 1);
        if idx1 > idx2 {
            std::mem::swap(&mut idx1, &mut idx2)
        };

        rope.remove(idx1, idx2);
        string_remove(&mut text, idx1, idx2);

        rope.assert_integrity();
        rope.assert_invariants();
        assert!(graphemes_match(&rope, text.as_str()));

        rope == text.as_str()
    }

    QuickCheck::new()
        .gen(StdGen::new(thread_rng(), TEXT.len()))
        .quickcheck(p as fn((usize, usize)) -> bool);
}

#[test]
fn qc_split_off_and_append() {
    fn p(mut idx: usize) -> bool {
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
        assert!(graphemes_match(&rope, TEXT));

        rope == TEXT
    }

    QuickCheck::new()
        .gen(StdGen::new(thread_rng(), TEXT.len()))
        .quickcheck(p as fn(usize) -> bool);
}

#[test]
fn qc_shrink_to_fit_01() {
    fn p(char_idxs: Vec<usize>) -> bool {
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

        let max_leaf_bytes = 768 - 33;
        (rope.capacity() - rope.len_bytes()) < max_leaf_bytes && rope.capacity() <= capacity_before
            && rope == rope_clone
    }

    QuickCheck::new()
        .gen(StdGen::new(thread_rng(), 500))
        .quickcheck(p as fn(Vec<usize>) -> bool);
}

#[test]
fn qc_shrink_to_fit_02() {
    fn p(char_idxs: Vec<usize>) -> bool {
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

        let max_leaf_bytes = 768 - 33;
        let max_diff = max_leaf_bytes + ((rope.len_bytes() / max_leaf_bytes) * ins_text.len());

        (rope.capacity() - rope.len_bytes()) < max_diff && rope == rope_clone
    }

    QuickCheck::new()
        .gen(StdGen::new(thread_rng(), 500))
        .quickcheck(p as fn(Vec<usize>) -> bool);
}

#[test]
fn qc_slice() {
    fn p(text: String, range: (usize, usize)) -> bool {
        let rope = Rope::from_str(&text);

        let mut idx1 = range.0 % (rope.len_chars() + 1);
        let mut idx2 = range.1 % (rope.len_chars() + 1);
        if idx1 > idx2 {
            std::mem::swap(&mut idx1, &mut idx2)
        };

        let slice = rope.slice(idx1, idx2);
        let text_slice = string_slice(&text, idx1, idx2);

        slice == text_slice && slice.len_bytes() == text_slice.len()
            && slice.len_chars() == text_slice.chars().count()
    }

    QuickCheck::new()
        .gen(StdGen::new(thread_rng(), 1 << 16))
        .quickcheck(p as fn(String, (usize, usize)) -> bool);
}

//===========================================================================

// 31138 bytes, 18021 chars, 95 lines
// Contains many long graphemes.
const TEXT: &str = "
T̴̷͚͖̜͈̪͎͔̝̫̦̹͔̻̮͂ͬͬ̌ͣ̿ͤ͌ͥ͑̀̂ͬ̚͘͜͞ô̵͚̤̯̹͖͍̦̼̦̖̞̺͕̳̬͇͕̟̜̅̌̈́̑̏̕͘͝ ͍̼̗̫͈̭̦̱̬͚̱̞͓̜̭̼͇̰̞ͮ͗ͣ́ͪ̔ͪ̍̑̏́̀̽̍̔͘͜͜͝ȋ̐̽ͦ̓̔̅͏̧̢̖̭̝̳̹̯̤͈̫͔͔̠͓͉̠͖̠͜ͅn̷̯̗̗̠̱̥͕͉̥͉̳̫̙̅͗̌̒͂̏͑̎̌̌̊͌͘͘ͅͅv̧̜͕͍͙͍̬͕͍̳͉̠͍̹̮̻̜ͨ̏͒̍ͬ̈́͒̈ͥ͗ͣ̄̃ͤ͊̌͆̓o̸̧̎̓͂̊͢҉͍̼̘͇̱̪̠͎̥̹ķ̈́͗͆ͥ͐͑̆̎́͌ͩͯ̊̓͐ͬ̇̕҉̢͏͚̲̰̗̦e̿̀͐̽ͪ̈ͤͬ҉́͟͏̵̫̲̱̻̰̲̦͇̭̟̺͈̞̫̰̜͕͖ͅ ̡̰͎͓͚͓͉͈̮̻̣̮̟̩̬̮̈̋̊͆ͪ̄ͪ͒ͨͧ̇ͪ̇̑̚t̷̬̟͎̞͈̯͙̹̜ͩ̓ͪ͛͐̐ͤ̾̄̈͒̽̈́̑͒̏h̨̢̳͇͓͉̝ͫ̐̓̆̓ͮ̔̓̈́̇ͫe̟̬̣̗͚̬̾̉͋̽ͯ̌ͯͬ̂ͯͭ̓͛́̚͡ ̨̭̱͉̭͈̈̽̆̂͒͗̀ͥͩ͡h̻̼̱̹̗͖̙̗̲̤͓͇͚͚̻̞̥ͥ͛͌ͧ̚͟i̢̯̹̹̘̳̙ͩ̉ͥ͆̽̇̾̎͗̔̓͂͂́̓̌ͬv̧̡̛̟̜̠͉͖̘̲̻̯͚͍͓̯̻̲̹̥͇̻̿̓͛̊̌ͩͩ́ͩ̍͌̚e̵̾́̈́̏͌͌̊͗̏͋ͦ͘͡͏͚̜͚͎͉͍̱͙̖̹̣̘̥̤̹̟͠-̔̌͐́͒ͦͮ̇ͭ̄̏̊̇̍̕͏̩̥̰͚̟m̨̒ͫͦ̔̔͋҉̱̩̗͇̥̰̩̭͍͚͠į̵̷̻̗͉͕͚̣̼̺͉̦̮̠̆̀̐ͩ͒ͯͩͯ͞ͅn̢̫̤̝̝͚̺͍̱̦͚͂̿ͨ̇ͤ͠d̡ͯ͋̋ͧ̈́̒̈͏̛͏̵̤̬͍̗̞̠̟̞̺̠̥̹̱͉̜͍͎̤ ̷̸̢̰͓̘̯͎̤̫̘͓̙̟̳͇̹̥͈͙̮̩̅̋͌͗̓͊̓ͨͣ͗̓͐̈́ͩ̓ͣrͫ͂͌ͪ̏̐̍̾ͥ̓͗̈͆̈ͥ̀̾̚̚҉̴̶̭͇̗͙̘̯̦̭̮̪͚̥̙̯̠͙̪͡e̵̸̲͉̳̙͖͖̫̘̪͕̳͓̻̙͙ͥ̍͂̽ͨ̓̒̒̏ͬ͗ͧ̑̀͠p̵̸̛̦̣͙̳̳̩̣̼̘͈͂ͪͭͤ̎r̶̩̟̞̙͔̼ͫ̆ͦ̐̀̏̾̉̍ͬ̅ͧ͊ͪ̒̈́ͬ̃͞ẻ̴̼͙͍͎̠̀̅̔̃̒͐ͦ̏̆̅̓͋͢ͅš̆̈̆̋ͨ̅̍̇͂̒ͩͨ̂̐̓ͩ͏̸͔͔̯͇͚̤̪̬̗͈̰̦̯͚̕ę̢̱̠͙̲͉̗͚̮̪͖̙̞̦͉͕̗̳͙ͦ̆̋͌ͣ̅̊́ͅņ̴̷̫̪̦͇̺̹͉̗̬̞̲̭̜̪͒̏͂̂̎͊́̋͒̏̅̋̚͘t̷̶̨̟̦̗̦̱͌͌ͩ̀i̴̴̢̖͓͙̘͇̠̦̙̭̼͖̹̾̒̎̐ͥͭ͋ͥ̅͟ͅņ̫͙̹̦̳͈͙̬̫̮͕̰̩̣̘̘͐̀̓ͭͩͬͯ̎͛̿ͫ̊̔̅́̕͠gͥͩ̂͌̒̊̕͏̻͙͖̣͙͍̹͕̝͖̼̙̘͝ ͤ͐̓̒̓͋̐̃̇͊̓ͦ͐̚͢҉̢̨̟̠͉̳͖̲̩͙̕ć̷̡̫̩̞̯̼̝̼͖̤̳̻̘̪̤͈̦̭ͣ́͂͐̽͆̔̀̚͜h̶̢̹̹̙͔̱̓ͦ͌̋̎ͭ͒͋̒ͭ̌̃͌̿ͣ̆̅͑ą̙̳̬̞̬͚̜̤̱̙͇̠̟̈ͤ͋̃̀̓̓ͯ̍̀̽ͣ̐̈̿̌̕ǫ͋͂͐ͬ̿ͯ̂̈́͌̓̌ͧ̕͏̜͔̗͚͔̘̣͕̘̲͖̼͇͖̗̳ͅͅs̷̸̝̙̭̦̣̦̯̭̦͙̹̻͍͇̣̼͗̌͆ͨͭ̃ͮ͐̿̕.̮̝̠̱̺͖͓̼̦̱̉͂͛̓̑̔̓ͮ̈̊̔͗́͝
̛̣̺̻̼̙̼͓̱̬͕̩͕̲̳̭̗̍ͤ͋̒̆̄ͨ̿ͧ̓͠ͅI̷̻̤̳̲͔͈̖̬̰͔̪͇͇̟̋ͨ̋̍̉̔͝͞͝ͅn̶͕̭͖̠̣͚̹̪͆ͪ̇̂̅̾ͫ́̅̉ͭ̀͜v̖͉̩͕̣͔̭͕̩̲̖̇̀ͬ́̄͒̆͑͆ͪͤ͆̾̍ͯ̚͜ǫ̡̡̫͎̟̞̰̞̹͇̲̏ͨ̄͊̊̇͒̽͢ķ̶̪̙̰̥͙̞̹̭̺͍͕̙̲̮͊ͭ́͋͛͋̑̒͊̏̒̅͛̄̓͟i̴͎̹̞̥͖̒̄ͮ̒̾ͮͧ̀̚͡n̸̵͓̲̟̞̳͚̼̣͙͖̈ͦ͒̿̅̒̿͛͊̇ͧ̉g̡̧̪̩͚͙͓̪͓͚͉̥̪͍̙̻͖͇͗̑͊͑̾̍͊̀ͅ ̷̵̠͚̘̟͓̫̣̲͎̩̹̣̼̟͊́̏ͫ̆ͩ̓͋͆̿̽̓͘̕t̴̢̝̻̖̲̬̜̺̖̻ͩ̿ͫ͗̈́̔͑̐ͮͦ̽̉̓̚͜h̷̛̲͇̫͈̣̭͂ͭ̂͋ͭ̋̔ͮ̆ͩ͞ë̩͕͉̯͇͔͚̭̼̮̣͓̯́ͭ̀ͣ͗̋̉ͨͬ̒ͥͩ͆̓̓́̀̚͘͝ ̛̫̠̗̥̳͇͉̟̮̪̻̤̪͚̟̜̔̌͌̈͌ͪ̋̎̄ͯ͐ͦ́͞͠fͦ̂̈ͬ̇̅̓̓ͫͣ̉̂̉̚͘͡͡͏̼̖̟͚̙̳͔͎̲̫̦̯͔̣̼̹ě̷̶̫͎̞̺̪̪͇͈̞̳̏̋̋͋̾̓̽̓̑ͮ͊ͣ̋̃̅̀͡e͇̗͎̱͔̦̠̰̩̩͖͙̠̻̝ͯ̿̔̀͋͑ͧ͊̆̇̿ͤ̄ͯ̇̀͢͠ͅl̂̿ͯ͛̊̒̓̈́͏̵̪̦̞̤̫̤͇̙̗͕͎̪͕̙̻̳̗̕͟͢i̞̣̙͎͈̗̮͉̱̜̱̝̞̤͋ͯ͋͐̈́ͫ̉̊̏̀ͯͨ͢͟͝n̳̻̼̥̖͍̭̅͂̓̔̔ͦ̔́ͦ͊̀͛̈́ͬͦ͢͡͡ģ̶̡̳̰̻̙̞̱̳̣̤̫̫͕̤̮̰̬̪̜͋͒̎̈́̉̏̀ͬͯ͌̇͊̚ ́̽ͤͦ̾̔͢҉̛̤͍͉̺̙̮̗̜̟̀͝ơ̢̱͓͓̙͉̖̠̯̦̗͍̓̐̃̉̅̃ͨ͆́ͪ̂̒̀̊̃͆̔͡͡ͅf́ͬ̊ͯͫ̉̈́̽̉̚͢͏̡̺̬̖͇̫͉̱ ̴͇̦̗̙̼̬͓̯͖̮͓͎̗͈̻̈́͆ͭ̐ͦ́͛̀͋̐̌ͬ͑̒̊̿̃͞c̶̸̣͔̬͕̪̱̩̣̑̒̑̓̍̓͂̍̔͌̚͘͜͞h̶͈̱͇͉̳͍͍̰͈͖̬̥͚̯͓̞̹̋̔ͯ̑̃́̒̎̎͊̈́̍̚̕ạ̴̞̱̥͍͙̺͉͚͎̫̦͎̥ͩ̀̀̊ͥ͢o̵̧͕̜͓͈̬̰̫̮͙̹͉̩̝̩͎̓̆͗̿̊̀ͯ̃ͪ̊ͫ̽̉̓ͧ͗́̚͢ͅͅs̡ͫ͋̑ͮ̍̃͊̄ͬ̅̈́ͬ̍̇̔̈̅̍̀҉̜͓̝̘̘̮̼͖͎̻͓͖̖͙̞ͅ.͗ͬͭͩ̌̅͗͏̷̮̗͇͔͇͈̮͢
̨͚̲̫̠̼͖̝̻̉ͤ̅̂ͩ̀̇ͬͭ̀͜Ẅ̢́̉͌ͮͬͨ͊̏͌̇̐͊͟͠҉̼̰̦̩͇͕̟̭̪̲͕̥͖̰̪͈̀ͅͅį̷ͣͦ̉̍ͨ͂͂͑̃͂ͪ̊̈̋̄͜҉̨͚̟̲̯̹̺̝̭̺̙͖͍t̼͓̰̩͙̦͓̟͚͖̀ͯ͛̍̈́͑͂̍̋́h̛̼̺̘̥̠̼̼̭͙̮͚̱̍ͯ̓̃̐̂̇͟ ̴̛͖͔̰̠̺̥̲ͮ̍ͫ̽͜õ̒ͯ̒̓ͦ̈́͑̔̒̓̎ͤ͑҉̸̭̱̤̭̬͈ų̙̫̤͖̺̫̱͓͓̗̪͇̩̙̔̉̊͂ͪ̇͢͟͞ͅt̸̬̣̫̞̫̅͐ͮ̌͌̈́̀̀͘ ̷̴̨̖̙̹͚ͬ̈́̈ͯͨͮ̇̈́̋̈́ͭ͛̑̉͊̕ö̡̍ͥ̂ͬͪͧ͒ͧ̏̓̇̂̄͆̌ͫͤ͢͠͝͏̖̱̯̘͙̰̖͎̰͓̟̤ṙ̡̬̟̬̜̪̮̺͖̗̘͈̟ͨ͐͗̑͒̐d̢ͭͫ̊̏ͬͥ͋́̌̈́ͮ̆ͬ̐̌̎͏̵̷̡̞̲̹̙͕̮̮͚ḙ̴̸̠͔͎̥͇͖͕̘̍̓̏̐ͩͩ̈́ͦ̐̋ͤ̎̾̌̏͊̊́̚͞ͅr̸͈̗̣̲̗̣̬̤ͦ̎ͫ̏̀ͥͪ̋ͧ̄͑̋͒͌͋ͦ̉͟͞.ͨͣ̽̈́͒̄ͮ̀͋͋͏̴̧̯̺̙̱̻͙̜
̡̣̞̠͓̰͍̠͕̭̺̼͊̽̿͊ͮ̐̓̒̊͒̔̓͐ͨ̈̌́T̸̸̓́̋ͬ́͆ͨͫ͌͂ͣ̋͒҉̺̝͎̟͖͚̠h̸̡̰̜̦͇͕̪̝̳͕͉̲̝̑ͥ͋ͧ̎̆͌͟e̛̹͍͍̫̙̞̪̭̙̟͙̱̺̮̳͕̜ͫ̓ͭ͊ͫ͆̀̚͟͡ ̿͂̄ͧ̔̎ͧ͑̾̀̓͏̦͍̳͈̳͔̘̖̲̯̰̟̝̳̖̦N̶̡̧̦̮̟̦̩̰̣̝̆̀͊̔͢e͛̄ͮͦͨ͂̔̓̍̄̉͆͊̑̑̆̚͏̜̗͎̝̼̯̥̜͖͍̪̝͞ͅͅz̨̛̀̾ͪ͗̉́͠͏͚̫̼̫̜̣pͪͦ͌̄ͥ̆ͣͩ͋̉́̏͞͏̥̜̝̳̱̞̙̳̤͙̟̟̮̦ȅ̷̩̟͉̯͕͔̘̺̥̻̻ͧ̊̅̽ͣ͑̓̑̽ͦ̾͌͜r̴̭̥̲̲̤͚͈̰͇̰͈̰̹ͫ̒ͯ̿͒ͧ̊͆͒ͣ́ḍ̭̟̤̈́̌̓̈́ͫ͐̍͂͞į̛̞̝̮̣͙͙̤̇̂̓̎͋̿̓̎̄̈́ͧ̓ͩ̐̓̄̋ͭ͞͠a͋̔̋ͫ̂͐͂҉̸̛̥̩̯̯̤̝͔̠̝̯̪̥̩̻̼̮n͌ͣ̂͋̿̚҉̛̙̲̺̯͇͓̝̯̪̟͔̩͟ͅ ̢̨͚̻̗̘͖̯̐ͥ͋̽ͯ̎̈́͋̏̄͋̆̑̊̆̚̕͟ͅh̢̛̗̱̭͇͖̰̮̮͈̲͍̯̟ͭ͊̎̽̓ͦͤ͠ï̛̘̝̦͎̦̭̠͖̳͎̮̼̏͐ͧ̒̒͐͑ͪͫ̋̽̚̚͜v̴̮͕̝̮̞͐̄͗̋͒ͤ̎̈̑ͬͮ̄̾ͤ̓̾͊͗͟é̶̷̡̩͖̰̫͓̟ͮͬͣ͊-ͦ͛ͩͤͨͨ̆̄͏̼̜̭͔̳͈͖̳̩͢ͅͅm̷̴̓́̓͛͒̾̍̉҉̛̗̹̠̣̪̺͎̖̝͚̖͙i̛̥͓̬̫͉͕͉͆͒ͧ̂̿̔̔͆̆̓̍͊̀͜n͌ͧͣ̅̌̎ͦͦ͑̑ͭ̆ͬ̀ͤ̀ͣ̚҉͎̰̱͚͈͈̬̹͕̺̙͙̼͘͘͞d̶͖̫̟̲͕̺̠͎̘͕̱̼͙̪̪̩͙̅̅̑̓̇͑̊̉͜͞ ̶̵̷̴̡̠͚̪͕̣̱̖̱̗̤̭̭͔͖͚ͧͤͥ͒̌ͪ͊͂͒̓͂ͧͧ̇̇͐̑̔ͅͅơ̵̲̲͇̯̰͇̜̣͕͕͓̲̤̲͔͚̞͑͗ͤ̓́̚͠ͅf̢̧̛̩̯̼̫͖̾ͣ͌̾̉́̈́̑̈́̚͞͞ͅ ͤͩ́͋͒ͫͬͣ̋̅̆҉̧̱̻͓͕͉̹̫̫̞̯̪̙̩͍̦͔̖̮̀͟ͅc͉̠̜̩̟͕͎̙̣̮̘̼͋ͯ̍ͨ̅̄ͫ̈̋ͫ̊͡͝ȟ̸̨ͯͦ̂̉̇̾̆ͭ̋̐̈̆̀̚͜҉͚͕̻̖a̶̴̛͚̗͙̳̬̲͚ͦ́̐ͥ́̔̅̑̎͐̑ͯ̾ͤͥͧ͡ò̶̧̞̪̦̥̪̻̦̝̳̬̔͛͛ͣ̋̌̔ͫ̂̽ͫ͘͠s̸̖̣̬̤̫͇̫̣̑͆͒̎̏́͟.̴̗̤̭͉̯̻̤͕̌ͯ̍ͤ̓͌ͤ̈̆̉ͦ̇́̚͘͟͝ͅ ̯̹̪͓̬͌̔̌ͬ̀͘͢͡͡Z̡̩̲̩̰̫̩̟͍̰͖͔̭ͣ̆̾ͭ̀́͞ͅa̡̡̙̜̭͇͎͔̙̞̫͓̜͉͔̬ͭ̈ͨ̉͆ͣͫ̃͌̓͌́ͣͥ̒̌͊͘͝l̢̨̡̯̙̫͖̫̺̘̬̟͈͌̊ͧͫͦ̉̃ͩͦ̒ͯ̇̌̓͛͟͝ͅg̵̙̼̼ͪ͂ͭ͗̈̕ȯ̅ͧ̓ͪ́̂͑̐ͩͥͬ̊̑͆̇͒ͫͣ͝҉͎̟̜̥͎̮̣͉̖̟̯̦̖͙͙͞ͅ.̈̑ͩ̇̂ͬ̓ͬ͊͂ͨ̽͠͏̺͎̞̦̜͍͚̯̯͔̝̞̻̩̖
̷̰̪͍͎͔͒ͯͥ̾̉͆ͤ̊̓̂͋̀͆H̸̸̹̞̙̺͎̠̯̤ͨ̉̍ͬͤ̓̐͌ͥͮ͞eͣ̈̾͛́͏͕̗͍̜̼͎͚̟̬̣̝̕ͅͅ ̴̛̩̗̼̝̣̩͚͇̯́̉͋̂̍͂̌ͮ͋̾͜͠wͮ̽̓ͭ̿͐̽̐̽͆̓͝҉̡̼̲͖̪̥h̢̢̛͍̰̰̻̱̼̰̹̖̖̪̝̥̘̎̀ͪ͒̾ͫͬ̆̑o̡̗̠̞̱̥͎̰͎͍̫̻͓͇͓͐ͥͯ͂̅͠ͅ ̡̛̏͑ͦ̓͊ͮͫͯͭ̌͒̆̍̈͠҉͖͚̪̫̗̮W̴̐̊͋̾ͥͫ҉͎̞͔̯̫̹͖̰͉̹̼͎̰̱͓̻̀a̶̩̤̙̣͎̳̭̲̗̠͉̳̭̭̦̞͎̮̅͌̾͗̾͛̇̀́͟͞ͅi̷̡ͣ̆̌͋͒͒́͘͏̮̺̩͎͇̜͍̫ṯ̴̢͖̥̖͇͎̦̦̹̖͇̪ͭ̅̍͐̇͒͋̽̏̿̒͆ͧ̄͋ͧͩ͒͜s̙̥̖̘̖͚̭̤̮̖̘̰̫̟̈́ͣ̍ͧ͐ͥ̏͆̃̿͒̔͐́̚͟ͅ ̨ͭ̌ͬͯ͆̒͋ͭ̔̿ͧ̅̓ͣ͡͏͇̟͉̥͔̬̼͚͙͚B̛̜̮̤͓̝̪̪͈͕̘̜͙̰̮̫̘̣͓͔̅ͩ͊̔ͦͯ́̌́͆ͭ̓́e̶̢̡̦͇͕̙͈͖͕̦̬̫͕̣̺̒̿͂͐͒͋͂ͦ́͋ͤ̿ͬ̊ͣ͗̑̽͜ͅͅh̸͑ͫͧ̑ͬͧ̈́̎̃ͣ̊̾͂ͨͤ̓͐̐̑͏̸̭͓̘͉̩i̧̧̭̣͈̝̺̼̺̠͉̞̜̲̳͙̦͐̔ͯ͛̅̾n̸͓̝̤̙͙͔ͪ̋̈́͒̒ͭ̈́̓ͮ̋̀̋̀̈ͩ́͌̄͘d̷̫̳̩̼̥̗̲̰͇͉̼̬̤͇̖ͮ̿ͬ͂ͦ̏̓ͮ̽͂̾̾ͯ͆͜͠ ̨̈́͒̇̏̄̑̓ͮͥ̒ͤͨ̋҉̴̴̟̱͙̟̫̩̗͔̝͔̀Ţ̵̝̟̖̭͇̻̳͖͉̺̖̖͙͙̺̐̈́̓ͯ̆̇̋ͩ͊̄̾̾ͬ̌̚͟ͅh̡͈̗̜͙̬̗̲̦̲̟̗̦̬͓̳ͧ̋̌͂͂ͨͬͦ̿̏̈́̋ͣ̒̕͡ͅͅe̗͇̰̰̥̪̗͑̔̓́̈́ͨ̊́̿̅ͯͥ̈́͐͗͘͢͝ ̡̢̛̯͎͓̰̘͎̦̪̯̪̥̰̲͇̠̲͔ͤͤ̇̅̆̋̂̆̈́ͤ̿͑ͅW̡͓͈̲̲͉̜͔̖͈̻̱͚̿̌͗̉ͤ͢͡ͅͅa̔̾͛̅͊͋͐҉̱̹̬͍͙̻̱l̢͎̟̬̙̼̱̫̮̘̼͔̭̅ͬ͑ͣ̏̾̅̓ͣ̿ͣ̈́̕͢͡ͅͅl̡̥̣͔̭̇̒͛͒͐̄̽͛̋ͥ̌͢͟͡.̷̰̝̮͔̟̦͈̥̬̻̥̬͎͓̻̲̇ͮ̿ͨͦ̽ͫ͟͢͝͠
͌̈ͦ͛ͮ̌͋̽̃͆

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Maecenas sit
amet tellus  nec turpis feugiat semper. Nam at nulla laoreet, finibus
eros sit amet, fringilla  mauris. Fusce vestibulum nec ligula efficitur
laoreet. Nunc orci leo, varius eget  ligula vulputate, consequat
eleifend nisi. Cras justo purus, imperdiet a augue  malesuada, convallis
cursus libero. Fusce pretium arcu in elementum laoreet. Duis  mauris
nulla, suscipit at est nec, malesuada pellentesque eros. Quisque semper
porta  malesuada. Nunc hendrerit est ac faucibus mollis. Nam fermentum
id libero sed  egestas. Duis a accumsan sapien. Nam neque diam, congue
non erat et, porta sagittis  turpis. Vivamus vitae mauris sit amet massa
mollis molestie. Morbi scelerisque,  augue id congue imperdiet, felis
lacus euismod dui, vitae facilisis massa dui quis  sapien. Vivamus
hendrerit a urna a lobortis.

T̴̷͚͖̜͈̪͎͔̝̫̦̹͔̻̮͂ͬͬ̌ͣ̿ͤ͌ͥ͑̀̂ͬ̚͘͜͞ô̵͚̤̯̹͖͍̦̼̦̖̞̺͕̳̬͇͕̟̜̅̌̈́̑̏̕͘͝ ͍̼̗̫͈̭̦̱̬͚̱̞͓̜̭̼͇̰̞ͮ͗ͣ́ͪ̔ͪ̍̑̏́̀̽̍̔͘͜͜͝ȋ̐̽ͦ̓̔̅͏̧̢̖̭̝̳̹̯̤͈̫͔͔̠͓͉̠͖̠͜ͅn̷̯̗̗̠̱̥͕͉̥͉̳̫̙̅͗̌̒͂̏͑̎̌̌̊͌͘͘ͅͅv̧̜͕͍͙͍̬͕͍̳͉̠͍̹̮̻̜ͨ̏͒̍ͬ̈́͒̈ͥ͗ͣ̄̃ͤ͊̌͆̓o̸̧̎̓͂̊͢҉͍̼̘͇̱̪̠͎̥̹ķ̈́͗͆ͥ͐͑̆̎́͌ͩͯ̊̓͐ͬ̇̕҉̢͏͚̲̰̗̦e̿̀͐̽ͪ̈ͤͬ҉́͟͏̵̫̲̱̻̰̲̦͇̭̟̺͈̞̫̰̜͕͖ͅ ̡̰͎͓͚͓͉͈̮̻̣̮̟̩̬̮̈̋̊͆ͪ̄ͪ͒ͨͧ̇ͪ̇̑̚t̷̬̟͎̞͈̯͙̹̜ͩ̓ͪ͛͐̐ͤ̾̄̈͒̽̈́̑͒̏h̨̢̳͇͓͉̝ͫ̐̓̆̓ͮ̔̓̈́̇ͫe̟̬̣̗͚̬̾̉͋̽ͯ̌ͯͬ̂ͯͭ̓͛́̚͡ ̨̭̱͉̭͈̈̽̆̂͒͗̀ͥͩ͡h̻̼̱̹̗͖̙̗̲̤͓͇͚͚̻̞̥ͥ͛͌ͧ̚͟i̢̯̹̹̘̳̙ͩ̉ͥ͆̽̇̾̎͗̔̓͂͂́̓̌ͬv̧̡̛̟̜̠͉͖̘̲̻̯͚͍͓̯̻̲̹̥͇̻̿̓͛̊̌ͩͩ́ͩ̍͌̚e̵̾́̈́̏͌͌̊͗̏͋ͦ͘͡͏͚̜͚͎͉͍̱͙̖̹̣̘̥̤̹̟͠-̔̌͐́͒ͦͮ̇ͭ̄̏̊̇̍̕͏̩̥̰͚̟m̨̒ͫͦ̔̔͋҉̱̩̗͇̥̰̩̭͍͚͠į̵̷̻̗͉͕͚̣̼̺͉̦̮̠̆̀̐ͩ͒ͯͩͯ͞ͅn̢̫̤̝̝͚̺͍̱̦͚͂̿ͨ̇ͤ͠d̡ͯ͋̋ͧ̈́̒̈͏̛͏̵̤̬͍̗̞̠̟̞̺̠̥̹̱͉̜͍͎̤ ̷̸̢̰͓̘̯͎̤̫̘͓̙̟̳͇̹̥͈͙̮̩̅̋͌͗̓͊̓ͨͣ͗̓͐̈́ͩ̓ͣrͫ͂͌ͪ̏̐̍̾ͥ̓͗̈͆̈ͥ̀̾̚̚҉̴̶̭͇̗͙̘̯̦̭̮̪͚̥̙̯̠͙̪͡e̵̸̲͉̳̙͖͖̫̘̪͕̳͓̻̙͙ͥ̍͂̽ͨ̓̒̒̏ͬ͗ͧ̑̀͠p̵̸̛̦̣͙̳̳̩̣̼̘͈͂ͪͭͤ̎r̶̩̟̞̙͔̼ͫ̆ͦ̐̀̏̾̉̍ͬ̅ͧ͊ͪ̒̈́ͬ̃͞ẻ̴̼͙͍͎̠̀̅̔̃̒͐ͦ̏̆̅̓͋͢ͅš̆̈̆̋ͨ̅̍̇͂̒ͩͨ̂̐̓ͩ͏̸͔͔̯͇͚̤̪̬̗͈̰̦̯͚̕ę̢̱̠͙̲͉̗͚̮̪͖̙̞̦͉͕̗̳͙ͦ̆̋͌ͣ̅̊́ͅņ̴̷̫̪̦͇̺̹͉̗̬̞̲̭̜̪͒̏͂̂̎͊́̋͒̏̅̋̚͘t̷̶̨̟̦̗̦̱͌͌ͩ̀i̴̴̢̖͓͙̘͇̠̦̙̭̼͖̹̾̒̎̐ͥͭ͋ͥ̅͟ͅņ̫͙̹̦̳͈͙̬̫̮͕̰̩̣̘̘͐̀̓ͭͩͬͯ̎͛̿ͫ̊̔̅́̕͠gͥͩ̂͌̒̊̕͏̻͙͖̣͙͍̹͕̝͖̼̙̘͝ ͤ͐̓̒̓͋̐̃̇͊̓ͦ͐̚͢҉̢̨̟̠͉̳͖̲̩͙̕ć̷̡̫̩̞̯̼̝̼͖̤̳̻̘̪̤͈̦̭ͣ́͂͐̽͆̔̀̚͜h̶̢̹̹̙͔̱̓ͦ͌̋̎ͭ͒͋̒ͭ̌̃͌̿ͣ̆̅͑ą̙̳̬̞̬͚̜̤̱̙͇̠̟̈ͤ͋̃̀̓̓ͯ̍̀̽ͣ̐̈̿̌̕ǫ͋͂͐ͬ̿ͯ̂̈́͌̓̌ͧ̕͏̜͔̗͚͔̘̣͕̘̲͖̼͇͖̗̳ͅͅs̷̸̝̙̭̦̣̦̯̭̦͙̹̻͍͇̣̼͗̌͆ͨͭ̃ͮ͐̿̕.̮̝̠̱̺͖͓̼̦̱̉͂͛̓̑̔̓ͮ̈̊̔͗́͝
̛̣̺̻̼̙̼͓̱̬͕̩͕̲̳̭̗̍ͤ͋̒̆̄ͨ̿ͧ̓͠ͅI̷̻̤̳̲͔͈̖̬̰͔̪͇͇̟̋ͨ̋̍̉̔͝͞͝ͅn̶͕̭͖̠̣͚̹̪͆ͪ̇̂̅̾ͫ́̅̉ͭ̀͜v̖͉̩͕̣͔̭͕̩̲̖̇̀ͬ́̄͒̆͑͆ͪͤ͆̾̍ͯ̚͜ǫ̡̡̫͎̟̞̰̞̹͇̲̏ͨ̄͊̊̇͒̽͢ķ̶̪̙̰̥͙̞̹̭̺͍͕̙̲̮͊ͭ́͋͛͋̑̒͊̏̒̅͛̄̓͟i̴͎̹̞̥͖̒̄ͮ̒̾ͮͧ̀̚͡n̸̵͓̲̟̞̳͚̼̣͙͖̈ͦ͒̿̅̒̿͛͊̇ͧ̉g̡̧̪̩͚͙͓̪͓͚͉̥̪͍̙̻͖͇͗̑͊͑̾̍͊̀ͅ ̷̵̠͚̘̟͓̫̣̲͎̩̹̣̼̟͊́̏ͫ̆ͩ̓͋͆̿̽̓͘̕t̴̢̝̻̖̲̬̜̺̖̻ͩ̿ͫ͗̈́̔͑̐ͮͦ̽̉̓̚͜h̷̛̲͇̫͈̣̭͂ͭ̂͋ͭ̋̔ͮ̆ͩ͞ë̩͕͉̯͇͔͚̭̼̮̣͓̯́ͭ̀ͣ͗̋̉ͨͬ̒ͥͩ͆̓̓́̀̚͘͝ ̛̫̠̗̥̳͇͉̟̮̪̻̤̪͚̟̜̔̌͌̈͌ͪ̋̎̄ͯ͐ͦ́͞͠fͦ̂̈ͬ̇̅̓̓ͫͣ̉̂̉̚͘͡͡͏̼̖̟͚̙̳͔͎̲̫̦̯͔̣̼̹ě̷̶̫͎̞̺̪̪͇͈̞̳̏̋̋͋̾̓̽̓̑ͮ͊ͣ̋̃̅̀͡e͇̗͎̱͔̦̠̰̩̩͖͙̠̻̝ͯ̿̔̀͋͑ͧ͊̆̇̿ͤ̄ͯ̇̀͢͠ͅl̂̿ͯ͛̊̒̓̈́͏̵̪̦̞̤̫̤͇̙̗͕͎̪͕̙̻̳̗̕͟͢i̞̣̙͎͈̗̮͉̱̜̱̝̞̤͋ͯ͋͐̈́ͫ̉̊̏̀ͯͨ͢͟͝n̳̻̼̥̖͍̭̅͂̓̔̔ͦ̔́ͦ͊̀͛̈́ͬͦ͢͡͡ģ̶̡̳̰̻̙̞̱̳̣̤̫̫͕̤̮̰̬̪̜͋͒̎̈́̉̏̀ͬͯ͌̇͊̚ ́̽ͤͦ̾̔͢҉̛̤͍͉̺̙̮̗̜̟̀͝ơ̢̱͓͓̙͉̖̠̯̦̗͍̓̐̃̉̅̃ͨ͆́ͪ̂̒̀̊̃͆̔͡͡ͅf́ͬ̊ͯͫ̉̈́̽̉̚͢͏̡̺̬̖͇̫͉̱ ̴͇̦̗̙̼̬͓̯͖̮͓͎̗͈̻̈́͆ͭ̐ͦ́͛̀͋̐̌ͬ͑̒̊̿̃͞c̶̸̣͔̬͕̪̱̩̣̑̒̑̓̍̓͂̍̔͌̚͘͜͞h̶͈̱͇͉̳͍͍̰͈͖̬̥͚̯͓̞̹̋̔ͯ̑̃́̒̎̎͊̈́̍̚̕ạ̴̞̱̥͍͙̺͉͚͎̫̦͎̥ͩ̀̀̊ͥ͢o̵̧͕̜͓͈̬̰̫̮͙̹͉̩̝̩͎̓̆͗̿̊̀ͯ̃ͪ̊ͫ̽̉̓ͧ͗́̚͢ͅͅs̡ͫ͋̑ͮ̍̃͊̄ͬ̅̈́ͬ̍̇̔̈̅̍̀҉̜͓̝̘̘̮̼͖͎̻͓͖̖͙̞ͅ.͗ͬͭͩ̌̅͗͏̷̮̗͇͔͇͈̮͢
̨͚̲̫̠̼͖̝̻̉ͤ̅̂ͩ̀̇ͬͭ̀͜Ẅ̢́̉͌ͮͬͨ͊̏͌̇̐͊͟͠҉̼̰̦̩͇͕̟̭̪̲͕̥͖̰̪͈̀ͅͅį̷ͣͦ̉̍ͨ͂͂͑̃͂ͪ̊̈̋̄͜҉̨͚̟̲̯̹̺̝̭̺̙͖͍t̼͓̰̩͙̦͓̟͚͖̀ͯ͛̍̈́͑͂̍̋́h̛̼̺̘̥̠̼̼̭͙̮͚̱̍ͯ̓̃̐̂̇͟ ̴̛͖͔̰̠̺̥̲ͮ̍ͫ̽͜õ̒ͯ̒̓ͦ̈́͑̔̒̓̎ͤ͑҉̸̭̱̤̭̬͈ų̙̫̤͖̺̫̱͓͓̗̪͇̩̙̔̉̊͂ͪ̇͢͟͞ͅt̸̬̣̫̞̫̅͐ͮ̌͌̈́̀̀͘ ̷̴̨̖̙̹͚ͬ̈́̈ͯͨͮ̇̈́̋̈́ͭ͛̑̉͊̕ö̡̍ͥ̂ͬͪͧ͒ͧ̏̓̇̂̄͆̌ͫͤ͢͠͝͏̖̱̯̘͙̰̖͎̰͓̟̤ṙ̡̬̟̬̜̪̮̺͖̗̘͈̟ͨ͐͗̑͒̐d̢ͭͫ̊̏ͬͥ͋́̌̈́ͮ̆ͬ̐̌̎͏̵̷̡̞̲̹̙͕̮̮͚ḙ̴̸̠͔͎̥͇͖͕̘̍̓̏̐ͩͩ̈́ͦ̐̋ͤ̎̾̌̏͊̊́̚͞ͅr̸͈̗̣̲̗̣̬̤ͦ̎ͫ̏̀ͥͪ̋ͧ̄͑̋͒͌͋ͦ̉͟͞.ͨͣ̽̈́͒̄ͮ̀͋͋͏̴̧̯̺̙̱̻͙̜
̡̣̞̠͓̰͍̠͕̭̺̼͊̽̿͊ͮ̐̓̒̊͒̔̓͐ͨ̈̌́T̸̸̓́̋ͬ́͆ͨͫ͌͂ͣ̋͒҉̺̝͎̟͖͚̠h̸̡̰̜̦͇͕̪̝̳͕͉̲̝̑ͥ͋ͧ̎̆͌͟e̛̹͍͍̫̙̞̪̭̙̟͙̱̺̮̳͕̜ͫ̓ͭ͊ͫ͆̀̚͟͡ ̿͂̄ͧ̔̎ͧ͑̾̀̓͏̦͍̳͈̳͔̘̖̲̯̰̟̝̳̖̦N̶̡̧̦̮̟̦̩̰̣̝̆̀͊̔͢e͛̄ͮͦͨ͂̔̓̍̄̉͆͊̑̑̆̚͏̜̗͎̝̼̯̥̜͖͍̪̝͞ͅͅz̨̛̀̾ͪ͗̉́͠͏͚̫̼̫̜̣pͪͦ͌̄ͥ̆ͣͩ͋̉́̏͞͏̥̜̝̳̱̞̙̳̤͙̟̟̮̦ȅ̷̩̟͉̯͕͔̘̺̥̻̻ͧ̊̅̽ͣ͑̓̑̽ͦ̾͌͜r̴̭̥̲̲̤͚͈̰͇̰͈̰̹ͫ̒ͯ̿͒ͧ̊͆͒ͣ́ḍ̭̟̤̈́̌̓̈́ͫ͐̍͂͞į̛̞̝̮̣͙͙̤̇̂̓̎͋̿̓̎̄̈́ͧ̓ͩ̐̓̄̋ͭ͞͠a͋̔̋ͫ̂͐͂҉̸̛̥̩̯̯̤̝͔̠̝̯̪̥̩̻̼̮n͌ͣ̂͋̿̚҉̛̙̲̺̯͇͓̝̯̪̟͔̩͟ͅ ̢̨͚̻̗̘͖̯̐ͥ͋̽ͯ̎̈́͋̏̄͋̆̑̊̆̚̕͟ͅh̢̛̗̱̭͇͖̰̮̮͈̲͍̯̟ͭ͊̎̽̓ͦͤ͠ï̛̘̝̦͎̦̭̠͖̳͎̮̼̏͐ͧ̒̒͐͑ͪͫ̋̽̚̚͜v̴̮͕̝̮̞͐̄͗̋͒ͤ̎̈̑ͬͮ̄̾ͤ̓̾͊͗͟é̶̷̡̩͖̰̫͓̟ͮͬͣ͊-ͦ͛ͩͤͨͨ̆̄͏̼̜̭͔̳͈͖̳̩͢ͅͅm̷̴̓́̓͛͒̾̍̉҉̛̗̹̠̣̪̺͎̖̝͚̖͙i̛̥͓̬̫͉͕͉͆͒ͧ̂̿̔̔͆̆̓̍͊̀͜n͌ͧͣ̅̌̎ͦͦ͑̑ͭ̆ͬ̀ͤ̀ͣ̚҉͎̰̱͚͈͈̬̹͕̺̙͙̼͘͘͞d̶͖̫̟̲͕̺̠͎̘͕̱̼͙̪̪̩͙̅̅̑̓̇͑̊̉͜͞ ̶̵̷̴̡̠͚̪͕̣̱̖̱̗̤̭̭͔͖͚ͧͤͥ͒̌ͪ͊͂͒̓͂ͧͧ̇̇͐̑̔ͅͅơ̵̲̲͇̯̰͇̜̣͕͕͓̲̤̲͔͚̞͑͗ͤ̓́̚͠ͅf̢̧̛̩̯̼̫͖̾ͣ͌̾̉́̈́̑̈́̚͞͞ͅ ͤͩ́͋͒ͫͬͣ̋̅̆҉̧̱̻͓͕͉̹̫̫̞̯̪̙̩͍̦͔̖̮̀͟ͅc͉̠̜̩̟͕͎̙̣̮̘̼͋ͯ̍ͨ̅̄ͫ̈̋ͫ̊͡͝ȟ̸̨ͯͦ̂̉̇̾̆ͭ̋̐̈̆̀̚͜҉͚͕̻̖a̶̴̛͚̗͙̳̬̲͚ͦ́̐ͥ́̔̅̑̎͐̑ͯ̾ͤͥͧ͡ò̶̧̞̪̦̥̪̻̦̝̳̬̔͛͛ͣ̋̌̔ͫ̂̽ͫ͘͠s̸̖̣̬̤̫͇̫̣̑͆͒̎̏́͟.̴̗̤̭͉̯̻̤͕̌ͯ̍ͤ̓͌ͤ̈̆̉ͦ̇́̚͘͟͝ͅ ̯̹̪͓̬͌̔̌ͬ̀͘͢͡͡Z̡̩̲̩̰̫̩̟͍̰͖͔̭ͣ̆̾ͭ̀́͞ͅa̡̡̙̜̭͇͎͔̙̞̫͓̜͉͔̬ͭ̈ͨ̉͆ͣͫ̃͌̓͌́ͣͥ̒̌͊͘͝l̢̨̡̯̙̫͖̫̺̘̬̟͈͌̊ͧͫͦ̉̃ͩͦ̒ͯ̇̌̓͛͟͝ͅg̵̙̼̼ͪ͂ͭ͗̈̕ȯ̅ͧ̓ͪ́̂͑̐ͩͥͬ̊̑͆̇͒ͫͣ͝҉͎̟̜̥͎̮̣͉̖̟̯̦̖͙͙͞ͅ.̈̑ͩ̇̂ͬ̓ͬ͊͂ͨ̽͠͏̺͎̞̦̜͍͚̯̯͔̝̞̻̩̖
̷̰̪͍͎͔͒ͯͥ̾̉͆ͤ̊̓̂͋̀͆H̸̸̹̞̙̺͎̠̯̤ͨ̉̍ͬͤ̓̐͌ͥͮ͞eͣ̈̾͛́͏͕̗͍̜̼͎͚̟̬̣̝̕ͅͅ ̴̛̩̗̼̝̣̩͚͇̯́̉͋̂̍͂̌ͮ͋̾͜͠wͮ̽̓ͭ̿͐̽̐̽͆̓͝҉̡̼̲͖̪̥h̢̢̛͍̰̰̻̱̼̰̹̖̖̪̝̥̘̎̀ͪ͒̾ͫͬ̆̑o̡̗̠̞̱̥͎̰͎͍̫̻͓͇͓͐ͥͯ͂̅͠ͅ ̡̛̏͑ͦ̓͊ͮͫͯͭ̌͒̆̍̈͠҉͖͚̪̫̗̮W̴̐̊͋̾ͥͫ҉͎̞͔̯̫̹͖̰͉̹̼͎̰̱͓̻̀a̶̩̤̙̣͎̳̭̲̗̠͉̳̭̭̦̞͎̮̅͌̾͗̾͛̇̀́͟͞ͅi̷̡ͣ̆̌͋͒͒́͘͏̮̺̩͎͇̜͍̫ṯ̴̢͖̥̖͇͎̦̦̹̖͇̪ͭ̅̍͐̇͒͋̽̏̿̒͆ͧ̄͋ͧͩ͒͜s̙̥̖̘̖͚̭̤̮̖̘̰̫̟̈́ͣ̍ͧ͐ͥ̏͆̃̿͒̔͐́̚͟ͅ ̨ͭ̌ͬͯ͆̒͋ͭ̔̿ͧ̅̓ͣ͡͏͇̟͉̥͔̬̼͚͙͚B̛̜̮̤͓̝̪̪͈͕̘̜͙̰̮̫̘̣͓͔̅ͩ͊̔ͦͯ́̌́͆ͭ̓́e̶̢̡̦͇͕̙͈͖͕̦̬̫͕̣̺̒̿͂͐͒͋͂ͦ́͋ͤ̿ͬ̊ͣ͗̑̽͜ͅͅh̸͑ͫͧ̑ͬͧ̈́̎̃ͣ̊̾͂ͨͤ̓͐̐̑͏̸̭͓̘͉̩i̧̧̭̣͈̝̺̼̺̠͉̞̜̲̳͙̦͐̔ͯ͛̅̾n̸͓̝̤̙͙͔ͪ̋̈́͒̒ͭ̈́̓ͮ̋̀̋̀̈ͩ́͌̄͘d̷̫̳̩̼̥̗̲̰͇͉̼̬̤͇̖ͮ̿ͬ͂ͦ̏̓ͮ̽͂̾̾ͯ͆͜͠ ̨̈́͒̇̏̄̑̓ͮͥ̒ͤͨ̋҉̴̴̟̱͙̟̫̩̗͔̝͔̀Ţ̵̝̟̖̭͇̻̳͖͉̺̖̖͙͙̺̐̈́̓ͯ̆̇̋ͩ͊̄̾̾ͬ̌̚͟ͅh̡͈̗̜͙̬̗̲̦̲̟̗̦̬͓̳ͧ̋̌͂͂ͨͬͦ̿̏̈́̋ͣ̒̕͡ͅͅe̗͇̰̰̥̪̗͑̔̓́̈́ͨ̊́̿̅ͯͥ̈́͐͗͘͢͝ ̡̢̛̯͎͓̰̘͎̦̪̯̪̥̰̲͇̠̲͔ͤͤ̇̅̆̋̂̆̈́ͤ̿͑ͅW̡͓͈̲̲͉̜͔̖͈̻̱͚̿̌͗̉ͤ͢͡ͅͅa̔̾͛̅͊͋͐҉̱̹̬͍͙̻̱l̢͎̟̬̙̼̱̫̮̘̼͔̭̅ͬ͑ͣ̏̾̅̓ͣ̿ͣ̈́̕͢͡ͅͅl̡̥̣͔̭̇̒͛͒͐̄̽͛̋ͥ̌͢͟͡.̷̰̝̮͔̟̦͈̥̬̻̥̬͎͓̻̲̇ͮ̿ͨͦ̽ͫ͟͢͝͠
̗̱͖͈͌̈ͦ͛ͮ̌͋̽̃͆̀͂ͨͧ̄̔̔ͭ̏͢Z̃̉̿ͮ̃̀͘͏͕̬̯̖͚̗͔Aͣ̑̈̓̈̑̈̀̿̚҉͙͍̦̗̦͙̠̝̩̯ͅͅL̴͖̞̞͙̱̻̥̬̜̦̐̇̉̈̽ͪ̅ͪ̂̔͌͑ͭ͐ͤ̈́̿̉͞ͅG̴̵̲̰̹̖͎͕ͯ̆̓̽͢͠Ŏ̶̡̺̼͙̣̞̩͕̥̟̝͕͔̯̞ͨ͒͊̂̊͂͗̒͆̾͆̌͆̃̎ͣͫ͜͡ͅ!̓̽̎̑̏́̓̓ͣ̀͏̱̩̭̣̹̺̗͜͞͞

Pellentesque nec viverra metus. Sed aliquet pellentesque scelerisque.
Duis efficitur  erat sit amet dui maximus egestas. Nullam blandit ante
tortor. Suspendisse vitae  consectetur sem, at sollicitudin neque.
Suspendisse sodales faucibus eros vitae  pellentesque. Cras non quam
dictum, pellentesque urna in, ornare erat. Praesent leo  est, aliquet et
euismod non, hendrerit sed urna. Sed convallis porttitor est, vel
aliquet felis cursus ac. Vivamus feugiat eget nisi eu molestie.
Phasellus tincidunt  nisl eget molestie consectetur. Phasellus vitae ex
ut odio sollicitudin vulputate.  Sed et nulla accumsan, eleifend arcu
eget, gravida neque. Donec sit amet tincidunt  eros. Ut in volutpat
ante.

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Maecenas sit
amet tellus  nec turpis feugiat semper. Nam at nulla laoreet, finibus
eros sit amet, fringilla  mauris. Fusce vestibulum nec ligula efficitur
laoreet. Nunc orci leo, varius eget  ligula vulputate, consequat
eleifend nisi. Cras justo purus, imperdiet a augue  malesuada, convallis
cursus libero. Fusce pretium arcu in elementum laoreet. Duis  mauris
nulla, suscipit at est nec, malesuada pellentesque eros. Quisque semper
porta  malesuada. Nunc hendrerit est ac faucibus mollis. Nam fermentum
id libero sed  egestas. Duis a accumsan sapien. Nam neque diam, congue
non erat et, porta sagittis  turpis. Vivamus vitae mauris sit amet massa
mollis molestie. Morbi scelerisque,  augue id congue imperdiet, felis
lacus euismod dui, vitae facilisis massa dui quis  sapien. Vivamus
hendrerit a urna a lobortis.

T̴̷͚͖̜͈̪͎͔̝̫̦̹͔̻̮͂ͬͬ̌ͣ̿ͤ͌ͥ͑̀̂ͬ̚͘͜͞ô̵͚̤̯̹͖͍̦̼̦̖̞̺͕̳̬͇͕̟̜̅̌̈́̑̏̕͘͝ ͍̼̗̫͈̭̦̱̬͚̱̞͓̜̭̼͇̰̞ͮ͗ͣ́ͪ̔ͪ̍̑̏́̀̽̍̔͘͜͜͝ȋ̐̽ͦ̓̔̅͏̧̢̖̭̝̳̹̯̤͈̫͔͔̠͓͉̠͖̠͜ͅn̷̯̗̗̠̱̥͕͉̥͉̳̫̙̅͗̌̒͂̏͑̎̌̌̊͌͘͘ͅͅv̧̜͕͍͙͍̬͕͍̳͉̠͍̹̮̻̜ͨ̏͒̍ͬ̈́͒̈ͥ͗ͣ̄̃ͤ͊̌͆̓o̸̧̎̓͂̊͢҉͍̼̘͇̱̪̠͎̥̹ķ̈́͗͆ͥ͐͑̆̎́͌ͩͯ̊̓͐ͬ̇̕҉̢͏͚̲̰̗̦e̿̀͐̽ͪ̈ͤͬ҉́͟͏̵̫̲̱̻̰̲̦͇̭̟̺͈̞̫̰̜͕͖ͅ ̡̰͎͓͚͓͉͈̮̻̣̮̟̩̬̮̈̋̊͆ͪ̄ͪ͒ͨͧ̇ͪ̇̑̚t̷̬̟͎̞͈̯͙̹̜ͩ̓ͪ͛͐̐ͤ̾̄̈͒̽̈́̑͒̏h̨̢̳͇͓͉̝ͫ̐̓̆̓ͮ̔̓̈́̇ͫe̟̬̣̗͚̬̾̉͋̽ͯ̌ͯͬ̂ͯͭ̓͛́̚͡ ̨̭̱͉̭͈̈̽̆̂͒͗̀ͥͩ͡h̻̼̱̹̗͖̙̗̲̤͓͇͚͚̻̞̥ͥ͛͌ͧ̚͟i̢̯̹̹̘̳̙ͩ̉ͥ͆̽̇̾̎͗̔̓͂͂́̓̌ͬv̧̡̛̟̜̠͉͖̘̲̻̯͚͍͓̯̻̲̹̥͇̻̿̓͛̊̌ͩͩ́ͩ̍͌̚e̵̾́̈́̏͌͌̊͗̏͋ͦ͘͡͏͚̜͚͎͉͍̱͙̖̹̣̘̥̤̹̟͠-̔̌͐́͒ͦͮ̇ͭ̄̏̊̇̍̕͏̩̥̰͚̟m̨̒ͫͦ̔̔͋҉̱̩̗͇̥̰̩̭͍͚͠į̵̷̻̗͉͕͚̣̼̺͉̦̮̠̆̀̐ͩ͒ͯͩͯ͞ͅn̢̫̤̝̝͚̺͍̱̦͚͂̿ͨ̇ͤ͠d̡ͯ͋̋ͧ̈́̒̈͏̛͏̵̤̬͍̗̞̠̟̞̺̠̥̹̱͉̜͍͎̤ ̷̸̢̰͓̘̯͎̤̫̘͓̙̟̳͇̹̥͈͙̮̩̅̋͌͗̓͊̓ͨͣ͗̓͐̈́ͩ̓ͣrͫ͂͌ͪ̏̐̍̾ͥ̓͗̈͆̈ͥ̀̾̚̚҉̴̶̭͇̗͙̘̯̦̭̮̪͚̥̙̯̠͙̪͡e̵̸̲͉̳̙͖͖̫̘̪͕̳͓̻̙͙ͥ̍͂̽ͨ̓̒̒̏ͬ͗ͧ̑̀͠p̵̸̛̦̣͙̳̳̩̣̼̘͈͂ͪͭͤ̎r̶̩̟̞̙͔̼ͫ̆ͦ̐̀̏̾̉̍ͬ̅ͧ͊ͪ̒̈́ͬ̃͞ẻ̴̼͙͍͎̠̀̅̔̃̒͐ͦ̏̆̅̓͋͢ͅš̆̈̆̋ͨ̅̍̇͂̒ͩͨ̂̐̓ͩ͏̸͔͔̯͇͚̤̪̬̗͈̰̦̯͚̕ę̢̱̠͙̲͉̗͚̮̪͖̙̞̦͉͕̗̳͙ͦ̆̋͌ͣ̅̊́ͅņ̴̷̫̪̦͇̺̹͉̗̬̞̲̭̜̪͒̏͂̂̎͊́̋͒̏̅̋̚͘t̷̶̨̟̦̗̦̱͌͌ͩ̀i̴̴̢̖͓͙̘͇̠̦̙̭̼͖̹̾̒̎̐ͥͭ͋ͥ̅͟ͅņ̫͙̹̦̳͈͙̬̫̮͕̰̩̣̘̘͐̀̓ͭͩͬͯ̎͛̿ͫ̊̔̅́̕͠gͥͩ̂͌̒̊̕͏̻͙͖̣͙͍̹͕̝͖̼̙̘͝ ͤ͐̓̒̓͋̐̃̇͊̓ͦ͐̚͢҉̢̨̟̠͉̳͖̲̩͙̕ć̷̡̫̩̞̯̼̝̼͖̤̳̻̘̪̤͈̦̭ͣ́͂͐̽͆̔̀̚͜h̶̢̹̹̙͔̱̓ͦ͌̋̎ͭ͒͋̒ͭ̌̃͌̿ͣ̆̅͑ą̙̳̬̞̬͚̜̤̱̙͇̠̟̈ͤ͋̃̀̓̓ͯ̍̀̽ͣ̐̈̿̌̕ǫ͋͂͐ͬ̿ͯ̂̈́͌̓̌ͧ̕͏̜͔̗͚͔̘̣͕̘̲͖̼͇͖̗̳ͅͅs̷̸̝̙̭̦̣̦̯̭̦͙̹̻͍͇̣̼͗̌͆ͨͭ̃ͮ͐̿̕.̮̝̠̱̺͖͓̼̦̱̉͂͛̓̑̔̓ͮ̈̊̔͗́͝
̛̣̺̻̼̙̼͓̱̬͕̩͕̲̳̭̗̍ͤ͋̒̆̄ͨ̿ͧ̓͠ͅI̷̻̤̳̲͔͈̖̬̰͔̪͇͇̟̋ͨ̋̍̉̔͝͞͝ͅn̶͕̭͖̠̣͚̹̪͆ͪ̇̂̅̾ͫ́̅̉ͭ̀͜v̖͉̩͕̣͔̭͕̩̲̖̇̀ͬ́̄͒̆͑͆ͪͤ͆̾̍ͯ̚͜ǫ̡̡̫͎̟̞̰̞̹͇̲̏ͨ̄͊̊̇͒̽͢ķ̶̪̙̰̥͙̞̹̭̺͍͕̙̲̮͊ͭ́͋͛͋̑̒͊̏̒̅͛̄̓͟i̴͎̹̞̥͖̒̄ͮ̒̾ͮͧ̀̚͡n̸̵͓̲̟̞̳͚̼̣͙͖̈ͦ͒̿̅̒̿͛͊̇ͧ̉g̡̧̪̩͚͙͓̪͓͚͉̥̪͍̙̻͖͇͗̑͊͑̾̍͊̀ͅ ̷̵̠͚̘̟͓̫̣̲͎̩̹̣̼̟͊́̏ͫ̆ͩ̓͋͆̿̽̓͘̕t̴̢̝̻̖̲̬̜̺̖̻ͩ̿ͫ͗̈́̔͑̐ͮͦ̽̉̓̚͜h̷̛̲͇̫͈̣̭͂ͭ̂͋ͭ̋̔ͮ̆ͩ͞ë̩͕͉̯͇͔͚̭̼̮̣͓̯́ͭ̀ͣ͗̋̉ͨͬ̒ͥͩ͆̓̓́̀̚͘͝ ̛̫̠̗̥̳͇͉̟̮̪̻̤̪͚̟̜̔̌͌̈͌ͪ̋̎̄ͯ͐ͦ́͞͠fͦ̂̈ͬ̇̅̓̓ͫͣ̉̂̉̚͘͡͡͏̼̖̟͚̙̳͔͎̲̫̦̯͔̣̼̹ě̷̶̫͎̞̺̪̪͇͈̞̳̏̋̋͋̾̓̽̓̑ͮ͊ͣ̋̃̅̀͡e͇̗͎̱͔̦̠̰̩̩͖͙̠̻̝ͯ̿̔̀͋͑ͧ͊̆̇̿ͤ̄ͯ̇̀͢͠ͅl̂̿ͯ͛̊̒̓̈́͏̵̪̦̞̤̫̤͇̙̗͕͎̪͕̙̻̳̗̕͟͢i̞̣̙͎͈̗̮͉̱̜̱̝̞̤͋ͯ͋͐̈́ͫ̉̊̏̀ͯͨ͢͟͝n̳̻̼̥̖͍̭̅͂̓̔̔ͦ̔́ͦ͊̀͛̈́ͬͦ͢͡͡ģ̶̡̳̰̻̙̞̱̳̣̤̫̫͕̤̮̰̬̪̜͋͒̎̈́̉̏̀ͬͯ͌̇͊̚ ́̽ͤͦ̾̔͢҉̛̤͍͉̺̙̮̗̜̟̀͝ơ̢̱͓͓̙͉̖̠̯̦̗͍̓̐̃̉̅̃ͨ͆́ͪ̂̒̀̊̃͆̔͡͡ͅf́ͬ̊ͯͫ̉̈́̽̉̚͢͏̡̺̬̖͇̫͉̱ ̴͇̦̗̙̼̬͓̯͖̮͓͎̗͈̻̈́͆ͭ̐ͦ́͛̀͋̐̌ͬ͑̒̊̿̃͞c̶̸̣͔̬͕̪̱̩̣̑̒̑̓̍̓͂̍̔͌̚͘͜͞h̶͈̱͇͉̳͍͍̰͈͖̬̥͚̯͓̞̹̋̔ͯ̑̃́̒̎̎͊̈́̍̚̕ạ̴̞̱̥͍͙̺͉͚͎̫̦͎̥ͩ̀̀̊ͥ͢o̵̧͕̜͓͈̬̰̫̮͙̹͉̩̝̩͎̓̆͗̿̊̀ͯ̃ͪ̊ͫ̽̉̓ͧ͗́̚͢ͅͅs̡ͫ͋̑ͮ̍̃͊̄ͬ̅̈́ͬ̍̇̔̈̅̍̀҉̜͓̝̘̘̮̼͖͎̻͓͖̖͙̞ͅ.͗ͬͭͩ̌̅͗͏̷̮̗͇͔͇͈̮͢
̨͚̲̫̠̼͖̝̻̉ͤ̅̂ͩ̀̇ͬͭ̀͜Ẅ̢́̉͌ͮͬͨ͊̏͌̇̐͊͟͠҉̼̰̦̩͇͕̟̭̪̲͕̥͖̰̪͈̀ͅͅį̷ͣͦ̉̍ͨ͂͂͑̃͂ͪ̊̈̋̄͜҉̨͚̟̲̯̹̺̝̭̺̙͖͍t̼͓̰̩͙̦͓̟͚͖̀ͯ͛̍̈́͑͂̍̋́h̛̼̺̘̥̠̼̼̭͙̮͚̱̍ͯ̓̃̐̂̇͟ ̴̛͖͔̰̠̺̥̲ͮ̍ͫ̽͜õ̒ͯ̒̓ͦ̈́͑̔̒̓̎ͤ͑҉̸̭̱̤̭̬͈ų̙̫̤͖̺̫̱͓͓̗̪͇̩̙̔̉̊͂ͪ̇͢͟͞ͅt̸̬̣̫̞̫̅͐ͮ̌͌̈́̀̀͘ ̷̴̨̖̙̹͚ͬ̈́̈ͯͨͮ̇̈́̋̈́ͭ͛̑̉͊̕ö̡̍ͥ̂ͬͪͧ͒ͧ̏̓̇̂̄͆̌ͫͤ͢͠͝͏̖̱̯̘͙̰̖͎̰͓̟̤ṙ̡̬̟̬̜̪̮̺͖̗̘͈̟ͨ͐͗̑͒̐d̢ͭͫ̊̏ͬͥ͋́̌̈́ͮ̆ͬ̐̌̎͏̵̷̡̞̲̹̙͕̮̮͚ḙ̴̸̠͔͎̥͇͖͕̘̍̓̏̐ͩͩ̈́ͦ̐̋ͤ̎̾̌̏͊̊́̚͞ͅr̸͈̗̣̲̗̣̬̤ͦ̎ͫ̏̀ͥͪ̋ͧ̄͑̋͒͌͋ͦ̉͟͞.ͨͣ̽̈́͒̄ͮ̀͋͋͏̴̧̯̺̙̱̻͙̜
̡̣̞̠͓̰͍̠͕̭̺̼͊̽̿͊ͮ̐̓̒̊͒̔̓͐ͨ̈̌́T̸̸̓́̋ͬ́͆ͨͫ͌͂ͣ̋͒҉̺̝͎̟͖͚̠h̸̡̰̜̦͇͕̪̝̳͕͉̲̝̑ͥ͋ͧ̎̆͌͟e̛̹͍͍̫̙̞̪̭̙̟͙̱̺̮̳͕̜ͫ̓ͭ͊ͫ͆̀̚͟͡ ̿͂̄ͧ̔̎ͧ͑̾̀̓͏̦͍̳͈̳͔̘̖̲̯̰̟̝̳̖̦N̶̡̧̦̮̟̦̩̰̣̝̆̀͊̔͢e͛̄ͮͦͨ͂̔̓̍̄̉͆͊̑̑̆̚͏̜̗͎̝̼̯̥̜͖͍̪̝͞ͅͅz̨̛̀̾ͪ͗̉́͠͏͚̫̼̫̜̣pͪͦ͌̄ͥ̆ͣͩ͋̉́̏͞͏̥̜̝̳̱̞̙̳̤͙̟̟̮̦ȅ̷̩̟͉̯͕͔̘̺̥̻̻ͧ̊̅̽ͣ͑̓̑̽ͦ̾͌͜r̴̭̥̲̲̤͚͈̰͇̰͈̰̹ͫ̒ͯ̿͒ͧ̊͆͒ͣ́ḍ̭̟̤̈́̌̓̈́ͫ͐̍͂͞į̛̞̝̮̣͙͙̤̇̂̓̎͋̿̓̎̄̈́ͧ̓ͩ̐̓̄̋ͭ͞͠a͋̔̋ͫ̂͐͂҉̸̛̥̩̯̯̤̝͔̠̝̯̪̥̩̻̼̮n͌ͣ̂͋̿̚҉̛̙̲̺̯͇͓̝̯̪̟͔̩͟ͅ ̢̨͚̻̗̘͖̯̐ͥ͋̽ͯ̎̈́͋̏̄͋̆̑̊̆̚̕͟ͅh̢̛̗̱̭͇͖̰̮̮͈̲͍̯̟ͭ͊̎̽̓ͦͤ͠ï̛̘̝̦͎̦̭̠͖̳͎̮̼̏͐ͧ̒̒͐͑ͪͫ̋̽̚̚͜v̴̮͕̝̮̞͐̄͗̋͒ͤ̎̈̑ͬͮ̄̾ͤ̓̾͊͗͟é̶̷̡̩͖̰̫͓̟ͮͬͣ͊-ͦ͛ͩͤͨͨ̆̄͏̼̜̭͔̳͈͖̳̩͢ͅͅm̷̴̓́̓͛͒̾̍̉҉̛̗̹̠̣̪̺͎̖̝͚̖͙i̛̥͓̬̫͉͕͉͆͒ͧ̂̿̔̔͆̆̓̍͊̀͜n͌ͧͣ̅̌̎ͦͦ͑̑ͭ̆ͬ̀ͤ̀ͣ̚҉͎̰̱͚͈͈̬̹͕̺̙͙̼͘͘͞d̶͖̫̟̲͕̺̠͎̘͕̱̼͙̪̪̩͙̅̅̑̓̇͑̊̉͜͞ ̶̵̷̴̡̠͚̪͕̣̱̖̱̗̤̭̭͔͖͚ͧͤͥ͒̌ͪ͊͂͒̓͂ͧͧ̇̇͐̑̔ͅͅơ̵̲̲͇̯̰͇̜̣͕͕͓̲̤̲͔͚̞͑͗ͤ̓́̚͠ͅf̢̧̛̩̯̼̫͖̾ͣ͌̾̉́̈́̑̈́̚͞͞ͅ ͤͩ́͋͒ͫͬͣ̋̅̆҉̧̱̻͓͕͉̹̫̫̞̯̪̙̩͍̦͔̖̮̀͟ͅc͉̠̜̩̟͕͎̙̣̮̘̼͋ͯ̍ͨ̅̄ͫ̈̋ͫ̊͡͝ȟ̸̨ͯͦ̂̉̇̾̆ͭ̋̐̈̆̀̚͜҉͚͕̻̖a̶̴̛͚̗͙̳̬̲͚ͦ́̐ͥ́̔̅̑̎͐̑ͯ̾ͤͥͧ͡ò̶̧̞̪̦̥̪̻̦̝̳̬̔͛͛ͣ̋̌̔ͫ̂̽ͫ͘͠s̸̖̣̬̤̫͇̫̣̑͆͒̎̏́͟.̴̗̤̭͉̯̻̤͕̌ͯ̍ͤ̓͌ͤ̈̆̉ͦ̇́̚͘͟͝ͅ ̯̹̪͓̬͌̔̌ͬ̀͘͢͡͡Z̡̩̲̩̰̫̩̟͍̰͖͔̭ͣ̆̾ͭ̀́͞ͅa̡̡̙̜̭͇͎͔̙̞̫͓̜͉͔̬ͭ̈ͨ̉͆ͣͫ̃͌̓͌́ͣͥ̒̌͊͘͝l̢̨̡̯̙̫͖̫̺̘̬̟͈͌̊ͧͫͦ̉̃ͩͦ̒ͯ̇̌̓͛͟͝ͅg̵̙̼̼ͪ͂ͭ͗̈̕ȯ̅ͧ̓ͪ́̂͑̐ͩͥͬ̊̑͆̇͒ͫͣ͝҉͎̟̜̥͎̮̣͉̖̟̯̦̖͙͙͞ͅ.̈̑ͩ̇̂ͬ̓ͬ͊͂ͨ̽͠͏̺͎̞̦̜͍͚̯̯͔̝̞̻̩̖
̷̰̪͍͎͔͒ͯͥ̾̉͆ͤ̊̓̂͋̀͆H̸̸̹̞̙̺͎̠̯̤ͨ̉̍ͬͤ̓̐͌ͥͮ͞eͣ̈̾͛́͏͕̗͍̜̼͎͚̟̬̣̝̕ͅͅ ̴̛̩̗̼̝̣̩͚͇̯́̉͋̂̍͂̌ͮ͋̾͜͠wͮ̽̓ͭ̿͐̽̐̽͆̓͝҉̡̼̲͖̪̥h̢̢̛͍̰̰̻̱̼̰̹̖̖̪̝̥̘̎̀ͪ͒̾ͫͬ̆̑o̡̗̠̞̱̥͎̰͎͍̫̻͓͇͓͐ͥͯ͂̅͠ͅ ̡̛̏͑ͦ̓͊ͮͫͯͭ̌͒̆̍̈͠҉͖͚̪̫̗̮W̴̐̊͋̾ͥͫ҉͎̞͔̯̫̹͖̰͉̹̼͎̰̱͓̻̀a̶̩̤̙̣͎̳̭̲̗̠͉̳̭̭̦̞͎̮̅͌̾͗̾͛̇̀́͟͞ͅi̷̡ͣ̆̌͋͒͒́͘͏̮̺̩͎͇̜͍̫ṯ̴̢͖̥̖͇͎̦̦̹̖͇̪ͭ̅̍͐̇͒͋̽̏̿̒͆ͧ̄͋ͧͩ͒͜s̙̥̖̘̖͚̭̤̮̖̘̰̫̟̈́ͣ̍ͧ͐ͥ̏͆̃̿͒̔͐́̚͟ͅ ̨ͭ̌ͬͯ͆̒͋ͭ̔̿ͧ̅̓ͣ͡͏͇̟͉̥͔̬̼͚͙͚B̛̜̮̤͓̝̪̪͈͕̘̜͙̰̮̫̘̣͓͔̅ͩ͊̔ͦͯ́̌́͆ͭ̓́e̶̢̡̦͇͕̙͈͖͕̦̬̫͕̣̺̒̿͂͐͒͋͂ͦ́͋ͤ̿ͬ̊ͣ͗̑̽͜ͅͅh̸͑ͫͧ̑ͬͧ̈́̎̃ͣ̊̾͂ͨͤ̓͐̐̑͏̸̭͓̘͉̩i̧̧̭̣͈̝̺̼̺̠͉̞̜̲̳͙̦͐̔ͯ͛̅̾n̸͓̝̤̙͙͔ͪ̋̈́͒̒ͭ̈́̓ͮ̋̀̋̀̈ͩ́͌̄͘d̷̫̳̩̼̥̗̲̰͇͉̼̬̤͇̖ͮ̿ͬ͂ͦ̏̓ͮ̽͂̾̾ͯ͆͜͠ ̨̈́͒̇̏̄̑̓ͮͥ̒ͤͨ̋҉̴̴̟̱͙̟̫̩̗͔̝͔̀Ţ̵̝̟̖̭͇̻̳͖͉̺̖̖͙͙̺̐̈́̓ͯ̆̇̋ͩ͊̄̾̾ͬ̌̚͟ͅh̡͈̗̜͙̬̗̲̦̲̟̗̦̬͓̳ͧ̋̌͂͂ͨͬͦ̿̏̈́̋ͣ̒̕͡ͅͅe̗͇̰̰̥̪̗͑̔̓́̈́ͨ̊́̿̅ͯͥ̈́͐͗͘͢͝ ̡̢̛̯͎͓̰̘͎̦̪̯̪̥̰̲͇̠̲͔ͤͤ̇̅̆̋̂̆̈́ͤ̿͑ͅW̡͓͈̲̲͉̜͔̖͈̻̱͚̿̌͗̉ͤ͢͡ͅͅa̔̾͛̅͊͋͐҉̱̹̬͍͙̻̱l̢͎̟̬̙̼̱̫̮̘̼͔̭̅ͬ͑ͣ̏̾̅̓ͣ̿ͣ̈́̕͢͡ͅͅl̡̥̣͔̭̇̒͛͒͐̄̽͛̋ͥ̌͢͟͡.̷̰̝̮͔̟̦͈̥̬̻̥̬͎͓̻̲̇ͮ̿ͨͦ̽ͫ͟͢͝͠
̗̱͖͈͌̈ͦ͛ͮ̌͋̽̃͆̀͂ͨͧ̄̔̔ͭ̏͢Z̃̉̿ͮ̃̀͘͏͕̬̯̖͚̗͔Aͣ̑̈̓̈̑̈̀̿̚҉͙͍̦̗̦͙̠̝̩̯ͅͅL̴͖̞̞͙̱̻̥̬̜̦̐̇̉̈̽ͪ̅ͪ̂̔͌͑ͭ͐ͤ̈́̿̉͞ͅG̴̵̲̰̹̖͎͕ͯ̆̓̽͢͠Ŏ̶̡̺̼͙̣̞̩͕̥̟̝͕͔̯̞ͨ͒͊̂̊͂͗̒͆̾͆̌͆̃̎ͣͫ͜͡ͅ!̓̽̎̑̏́̓̓ͣ̀͏̱̩̭̣̹̺̗͜͞͞

Aliquam finibus metus commodo sem egestas, non mollis odio pretium.
Aenean ex  lectus, rutrum nec laoreet at, posuere sit amet lacus. Nulla
eros augue, vehicula et  molestie accumsan, dictum vel odio. In quis
risus finibus, pellentesque ipsum  blandit, volutpat diam. Etiam
suscipit varius mollis. Proin vel luctus nisi, ac  ornare justo. Integer
porttitor quam magna. Donec vitae metus tempor, ultricies  risus in,
dictum erat. Integer porttitor faucibus vestibulum. Class aptent taciti
sociosqu ad litora torquent per conubia nostra, per inceptos himenaeos.
Vestibulum  ante ipsum primis in faucibus orci luctus et ultrices
posuere cubilia Curae; Nam  semper congue ante, a ultricies velit
venenatis vitae. Proin non neque sit amet ex  commodo congue non nec
elit. Nullam vel dignissim ipsum. Duis sed lobortis ante.  Aenean
feugiat rutrum magna ac luctus.

Ut imperdiet non ante sit amet rutrum. Cras vel massa eget nisl gravida
auctor.  Nulla bibendum ut tellus ut rutrum. Quisque malesuada lacinia
felis, vitae semper  elit. Praesent sit amet velit imperdiet, lobortis
nunc at, faucibus tellus. Nullam  porttitor augue mauris, a dapibus
tellus ultricies et. Fusce aliquet nec velit in  mattis. Sed mi ante,
lacinia eget ornare vel, faucibus at metus.

Pellentesque nec viverra metus. Sed aliquet pellentesque scelerisque.
Duis efficitur  erat sit amet dui maximus egestas. Nullam blandit ante
tortor. Suspendisse vitae  consectetur sem, at sollicitudin neque.
Suspendisse sodales faucibus eros vitae  pellentesque. Cras non quam
dictum, pellentesque urna in, ornare erat. Praesent leo  est, aliquet et
euismod non, hendrerit sed urna. Sed convallis porttitor est, vel
aliquet felis cursus ac. Vivamus feugiat eget nisi eu molestie.
Phasellus tincidunt  nisl eget molestie consectetur. Phasellus vitae ex
ut odio sollicitudin vulputate.  Sed et nulla accumsan, eleifend arcu
eget, gravida neque. Donec sit amet tincidunt  eros. Ut in volutpat
ante.
";
