#[macro_use]
extern crate bencher;
extern crate rand;
extern crate ropey;

use bencher::Bencher;
use ropey::Rope;
use rand::random;

const TEXT: &str = include_str!("large.txt");

//----

fn small_inserts_random(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(random::<usize>() % len, "a");
    })
}

fn small_inserts_start(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        rope.insert(0, "a");
    })
}

fn small_inserts_middle(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(len / 2, "a");
    })
}

fn small_inserts_end(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(len, "a");
    })
}

//----

fn medium_inserts_random(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(random::<usize>() % len, "This is some text.");
    })
}

fn medium_inserts_start(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        rope.insert(0, "This is some text.");
    })
}

fn medium_inserts_middle(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(len / 2, "This is some text.");
    })
}

fn medium_inserts_end(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(len, "This is some text.");
    })
}

//----

const INSERT_TEXT: &str = include_str!("small.txt");

fn large_inserts_random(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(random::<usize>() % len, INSERT_TEXT);
    })
}

fn large_inserts_start(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        rope.insert(0, INSERT_TEXT);
    })
}

fn large_inserts_middle(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(len / 2, INSERT_TEXT);
    })
}

fn large_inserts_end(bench: &mut Bencher) {
    let mut rope = Rope::from_str(TEXT);
    bench.iter(|| {
        let len = rope.len_chars();
        rope.insert(len, INSERT_TEXT);
    })
}

//----

benchmark_group!(
    benches,
    small_inserts_random,
    small_inserts_start,
    small_inserts_middle,
    small_inserts_end,
    medium_inserts_random,
    medium_inserts_start,
    medium_inserts_middle,
    medium_inserts_end,
    large_inserts_random,
    large_inserts_start,
    large_inserts_middle,
    large_inserts_end
);
benchmark_main!(benches);
