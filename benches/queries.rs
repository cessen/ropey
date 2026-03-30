extern crate criterion;
extern crate fastrand;
extern crate ropey;

use criterion::{criterion_group, criterion_main, Criterion};
use ropey::Rope;

const TEXT: &str = include_str!("large.txt");
const SMALL_TEXT: &str = include_str!("small.txt");

//----

fn index_convert(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_convert");

    group.bench_function("byte_to_char", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.byte_to_char(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("byte_to_line", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.byte_to_line(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("char_to_byte", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_chars();
        bench.iter(|| {
            rope.char_to_byte(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("char_to_line", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_chars();
        bench.iter(|| {
            rope.char_to_line(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("line_to_byte", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_lines();
        bench.iter(|| {
            rope.line_to_byte(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("line_to_char", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_lines();
        bench.iter(|| {
            rope.line_to_char(rng.usize(0..(len + 1)));
        })
    });
}

fn get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get");

    group.bench_function("byte", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.byte(rng.usize(0..len));
        })
    });

    group.bench_function("char", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_chars();
        bench.iter(|| {
            rope.char(rng.usize(0..len));
        })
    });

    group.bench_function("line", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_lines();
        bench.iter(|| {
            rope.line(rng.usize(0..len));
        })
    });

    group.bench_function("chunk_at_byte", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.chunk_at_byte(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("chunk_at_byte_slice", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let slice = rope.slice(324..(rope.len_chars() - 213));
        let len = slice.len_bytes();
        bench.iter(|| {
            slice.chunk_at_byte(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("chunk_at_char", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_chars();
        bench.iter(|| {
            rope.chunk_at_char(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("chunk_at_char_slice", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let slice = rope.slice(324..(rope.len_chars() - 213));
        let len = slice.len_chars();
        bench.iter(|| {
            slice.chunk_at_char(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("chunk_at_line_break", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_lines();
        bench.iter(|| {
            rope.chunk_at_line_break(rng.usize(0..(len + 1)));
        })
    });

    group.bench_function("chunk_at_line_break_slice", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let slice = rope.slice(324..(rope.len_chars() - 213));
        let len = slice.len_lines();
        bench.iter(|| {
            slice.chunk_at_line_break(rng.usize(0..(len + 1)));
        })
    });
}

fn slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("slice");

    group.bench_function("slice", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_chars();
        bench.iter(|| {
            let mut start = rng.usize(0..(len + 1));
            let mut end = rng.usize(0..(len + 1));
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
            rope.slice(start..end);
        })
    });

    group.bench_function("slice_small", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(TEXT);
        let len = rope.len_chars();
        bench.iter(|| {
            let mut start = rng.usize(0..(len + 1));
            if start > (len - 65) {
                start = len - 65;
            }
            let end = start + 64;
            rope.slice(start..end);
        })
    });

    group.bench_function("slice_from_small_rope", |bench| {
        let mut rng = fastrand::Rng::new();
        let rope = Rope::from_str(SMALL_TEXT);
        let len = rope.len_chars();
        bench.iter(|| {
            let mut start = rng.usize(0..(len + 1));
            let mut end = rng.usize(0..(len + 1));
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
            rope.slice(start..end);
        })
    });

    group.bench_function("slice_whole_rope", |bench| {
        let rope = Rope::from_str(TEXT);
        bench.iter(|| {
            rope.slice(..);
        })
    });

    group.bench_function("slice_whole_slice", |bench| {
        let rope = Rope::from_str(TEXT);
        let len = rope.len_chars();
        let slice = rope.slice(1..len - 1);
        bench.iter(|| {
            slice.slice(..);
        })
    });
}

//----

criterion_group!(benches, index_convert, get, slice,);
criterion_main!(benches);
