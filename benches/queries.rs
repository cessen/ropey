#[macro_use]
extern crate bencher;
extern crate rand;
extern crate ropey;

use bencher::Bencher;
use ropey::Rope;
use rand::random;

const TEXT: &str = include_str!("large.txt");
const SMALL_TEXT: &str = include_str!("small.txt");

//----

fn char_to_line(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_chars();
    bench.iter(|| {
        rope.char_to_line(random::<usize>() % (len + 1));
    })
}

fn line_to_char(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_lines();
    bench.iter(|| {
        rope.line_to_char(random::<usize>() % (len + 1));
    })
}

//----

fn get_char(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_chars();
    bench.iter(|| {
        rope.char(random::<usize>() % len);
    })
}

fn get_line(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_lines();
    bench.iter(|| {
        rope.line(random::<usize>() % len);
    })
}

//----

fn slice(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_chars();
    bench.iter(|| {
        let mut start = random::<usize>() % (len + 1);
        let mut end = random::<usize>() % (len + 1);
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }
        rope.slice(start..end);
    })
}

fn slice_from_small(bench: &mut Bencher) {
    let rope = Rope::from_str(SMALL_TEXT);
    let len = rope.len_chars();
    bench.iter(|| {
        let mut start = random::<usize>() % (len + 1);
        let mut end = random::<usize>() % (len + 1);
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }
        rope.slice(start..end);
    })
}

//----

benchmark_group!(
    benches,
    char_to_line,
    line_to_char,
    get_char,
    get_line,
    slice,
    slice_from_small
);
benchmark_main!(benches);
