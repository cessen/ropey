#[macro_use]
extern crate bencher;
extern crate rand;
extern crate ropey;

use bencher::Bencher;
use ropey::Rope;
use rand::random;

const TEXT: &str = include_str!("large.txt");

//----

fn removals_random_small(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 1).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_start_small(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + 1).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_middle_small(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + 1).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_end_small(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - (1).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

//----

fn removals_random_medium(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 15).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_start_medium(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + 15).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_middle_medium(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + 15).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_end_medium(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - (15).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

//----

const LEN_MUL: usize = 2;

fn removals_random_large(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    let tmp = rope.clone();
    for _ in 0..(LEN_MUL - 1) {
        rope.append(tmp.clone());
    }

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 1500).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_start_large(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    let tmp = rope.clone();
    for _ in 0..(LEN_MUL - 1) {
        rope.append(tmp.clone());
    }

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + 1500).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_middle_large(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    let tmp = rope.clone();
    for _ in 0..(LEN_MUL - 1) {
        rope.append(tmp.clone());
    }

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + 1500).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn removals_end_large(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    let tmp = rope.clone();
    for _ in 0..(LEN_MUL - 1) {
        rope.append(tmp.clone());
    }

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - (1500).min(len);
        rope.remove(start..end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
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
    removals_end_large
);
benchmark_main!(benches);
