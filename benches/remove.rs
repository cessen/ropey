#[macro_use]
extern crate bencher;
extern crate rand;
extern crate ropey;

use bencher::Bencher;
use rand::random;
use ropey::Rope;

const TEXT: &str = include_str!("large.txt");
const TEXT_SMALL: &str = include_str!("small.txt");

fn mul_string_length(text: &str, n: usize) -> String {
    let mut mtext = String::new();
    for _ in 0..n {
        mtext.push_str(text);
    }
    mtext
}

//----
const LEN_MUL_SMALL: usize = 1;

fn removals_random_small(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_SMALL);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 1).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == TEXT.len() / 2 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_start_small(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_SMALL);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + 1).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == TEXT.len() / 2 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_middle_small(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_SMALL);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + 1).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == TEXT.len() / 2 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_end_small(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_SMALL);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - (1).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == TEXT.len() / 2 {
            rope = Rope::from_str(&text);
        }
    })
}

//----
const LEN_MUL_MEDIUM: usize = 1;

fn removals_random_medium(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_MEDIUM);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 15).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == TEXT.len() / 2 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_start_medium(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_MEDIUM);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + 15).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == TEXT.len() / 2 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_middle_medium(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_MEDIUM);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + 15).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == TEXT.len() / 2 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_end_medium(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_MEDIUM);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - (15).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == TEXT.len() / 2 {
            rope = Rope::from_str(&text);
        }
    })
}

//----

const LEN_MUL_LARGE: usize = 4;

fn removals_random_large(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_LARGE);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + TEXT_SMALL.len()).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_start_large(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_LARGE);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + TEXT_SMALL.len()).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_middle_large(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_LARGE);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + TEXT_SMALL.len()).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(&text);
        }
    })
}

fn removals_end_large(bench: &mut Bencher) {
    let text = mul_string_length(TEXT, LEN_MUL_LARGE);
    let mut rope = Rope::from_str(&text);

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - TEXT_SMALL.len().min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(&text);
        }
    })
}

//----

fn initial_remove_after_clone(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let mut rope_clone = rope.clone();
    let mut i = 0;
    bench.iter(|| {
        if i > 32 {
            i = 0;
            rope_clone = rope.clone();
        }
        let len = rope_clone.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 1).min(len);
        rope_clone.remove(start..end);
        i += 1;
    })
}

//----

benchmark_group!(
    benches,
    removals_random_small,
    removals_start_small,
    removals_middle_small,
    removals_end_small,
    removals_random_medium,
    removals_start_medium,
    removals_middle_medium,
    removals_end_medium,
    removals_random_large,
    removals_start_large,
    removals_middle_large,
    removals_end_large,
    initial_remove_after_clone
);
benchmark_main!(benches);
