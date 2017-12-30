#[macro_use]
extern crate bencher;
extern crate rand;
extern crate ropey;

use bencher::Bencher;
use ropey::Rope;
use rand::random;

const TEXT: &str = include_str!("large.txt");

//----

fn small_removals_random(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 1).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn small_removals_start(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + 1).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn small_removals_middle(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + 1).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn small_removals_end(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - (1).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

//----

fn medium_removals_random(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 15).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn medium_removals_start(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + 15).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn medium_removals_middle(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + 15).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn medium_removals_end(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - (15).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

//----

const LEN_MUL: usize = 2;

fn large_removals_random(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    let tmp = rope.clone();
    for _ in 0..(LEN_MUL - 1) {
        rope.append(tmp.clone());
    }

    bench.iter(|| {
        let len = rope.len_chars();
        let start = random::<usize>() % (len + 1);
        let end = (start + 1500).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn large_removals_start(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    let tmp = rope.clone();
    for _ in 0..(LEN_MUL - 1) {
        rope.append(tmp.clone());
    }

    bench.iter(|| {
        let len = rope.len_chars();
        let start = 0;
        let end = (start + 1500).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn large_removals_middle(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    let tmp = rope.clone();
    for _ in 0..(LEN_MUL - 1) {
        rope.append(tmp.clone());
    }

    bench.iter(|| {
        let len = rope.len_chars();
        let start = len / 2;
        let end = (start + 1500).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

fn large_removals_end(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    let tmp = rope.clone();
    for _ in 0..(LEN_MUL - 1) {
        rope.append(tmp.clone());
    }

    bench.iter(|| {
        let len = rope.len_chars();
        let end = len;
        let start = end - (1500).min(len);
        rope.remove(start, end);

        if rope.len_bytes() == 0 {
            rope = Rope::from_str(TEXT);
        }
    })
}

//----

benchmark_group!(
    benches,
    small_removals_random,
    small_removals_start,
    small_removals_middle,
    small_removals_end,
    medium_removals_random,
    medium_removals_start,
    medium_removals_middle,
    medium_removals_end,
    large_removals_random,
    large_removals_start,
    large_removals_middle,
    large_removals_end
);
benchmark_main!(benches);
