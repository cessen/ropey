#[macro_use]
extern crate bencher;
extern crate rand;
extern crate ropey;

use bencher::Bencher;
use rand::random;
use ropey::Rope;

const TEXT: &str = include_str!("large.txt");
const SMALL_TEXT: &str = include_str!("small.txt");

//----

fn byte_to_char(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_bytes();
    bench.iter(|| {
        rope.byte_to_char(random::<usize>() % (len + 1));
    })
}

fn byte_to_line(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_bytes();
    bench.iter(|| {
        rope.byte_to_line(random::<usize>() % (len + 1));
    })
}

fn char_to_byte(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_chars();
    bench.iter(|| {
        rope.char_to_byte(random::<usize>() % (len + 1));
    })
}

fn char_to_line(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_chars();
    bench.iter(|| {
        rope.char_to_line(random::<usize>() % (len + 1));
    })
}

fn line_to_byte(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_lines();
    bench.iter(|| {
        rope.line_to_byte(random::<usize>() % (len + 1));
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

fn get_byte(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_bytes();
    bench.iter(|| {
        rope.byte(random::<usize>() % len);
    })
}

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

fn chunk_at_byte(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_bytes();
    bench.iter(|| {
        rope.chunk_at_byte(random::<usize>() % (len + 1));
    })
}

fn chunk_at_byte_slice(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let slice = rope.slice(324..(rope.len_chars() - 213));
    let len = slice.len_bytes();
    bench.iter(|| {
        slice.chunk_at_byte(random::<usize>() % (len + 1));
    })
}

fn chunk_at_char(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_chars();
    bench.iter(|| {
        rope.chunk_at_char(random::<usize>() % (len + 1));
    })
}

fn chunk_at_char_slice(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let slice = rope.slice(324..(rope.len_chars() - 213));
    let len = slice.len_chars();
    bench.iter(|| {
        slice.chunk_at_char(random::<usize>() % (len + 1));
    })
}

fn chunk_at_line_break(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_lines();
    bench.iter(|| {
        rope.chunk_at_line_break(random::<usize>() % (len + 1));
    })
}

fn chunk_at_line_break_slice(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let slice = rope.slice(324..(rope.len_chars() - 213));
    let len = slice.len_lines();
    bench.iter(|| {
        slice.chunk_at_line_break(random::<usize>() % (len + 1));
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

fn slice_small(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_chars();
    bench.iter(|| {
        let mut start = random::<usize>() % (len + 1);
        if start > (len - 65) {
            start = len - 65;
        }
        let end = start + 64;
        rope.slice(start..end);
    })
}

fn slice_from_small_rope(bench: &mut Bencher) {
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

fn slice_whole_rope(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    bench.iter(|| {
        rope.slice(..);
    })
}

fn slice_whole_slice(bench: &mut Bencher) {
    let rope = Rope::from_str(TEXT);
    let len = rope.len_chars();
    let slice = rope.slice(1..len - 1);
    bench.iter(|| {
        slice.slice(..);
    })
}

//----

benchmark_group!(
    benches,
    byte_to_char,
    byte_to_line,
    char_to_byte,
    char_to_line,
    line_to_byte,
    line_to_char,
    get_byte,
    get_char,
    get_line,
    chunk_at_byte,
    chunk_at_byte_slice,
    chunk_at_char,
    chunk_at_char_slice,
    chunk_at_line_break,
    chunk_at_line_break_slice,
    slice,
    slice_small,
    slice_from_small_rope,
    slice_whole_rope,
    slice_whole_slice
);
benchmark_main!(benches);
