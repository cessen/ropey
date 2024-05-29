extern crate criterion;
extern crate rand;
extern crate ropey;

use criterion::{criterion_group, criterion_main, Criterion};
use rand::random;
use ropey::Rope;

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
use ropey::LineType;

const TEXT_SMALL: &str = include_str!("small.txt");

fn large_string() -> String {
    let mut text = String::new();
    for _ in 0..1000 {
        text.push_str(TEXT_SMALL);
    }
    text
}

//----

fn index_convert(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_convert");

    #[cfg(feature = "metric_chars")]
    group.bench_function("byte_to_char", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.byte_to_char(random::<usize>() % (len + 1));
        })
    });

    #[cfg(feature = "metric_chars")]
    group.bench_function("char_to_byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_chars();
        bench.iter(|| {
            rope.char_to_byte(random::<usize>() % (len + 1));
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("byte_to_line_lf", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.byte_to_line(random::<usize>() % (len + 1), LineType::LF);
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("line_lf_to_byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::LF);
        bench.iter(|| {
            rope.line_to_byte(random::<usize>() % (len + 1), LineType::LF);
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("byte_to_line_cr_lf", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.byte_to_line(random::<usize>() % (len + 1), LineType::LF_CR);
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("line_cr_lf_to_byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::LF_CR);
        bench.iter(|| {
            rope.line_to_byte(random::<usize>() % (len + 1), LineType::LF_CR);
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("byte_to_line_all", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.byte_to_line(random::<usize>() % (len + 1), LineType::All);
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("line_all_to_byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::LF_CR);
        bench.iter(|| {
            rope.line_to_byte(random::<usize>() % (len + 1), LineType::All);
        })
    });
}

fn get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get");

    group.bench_function("byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.byte(random::<usize>() % len);
        })
    });

    #[cfg(feature = "metric_chars")]
    group.bench_function("char", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_chars();
        bench.iter(|| {
            rope.char(random::<usize>() % len);
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("line_lf", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::LF);
        bench.iter(|| {
            rope.line(random::<usize>() % len, LineType::LF);
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("line_cr_lf", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::LF_CR);
        bench.iter(|| {
            rope.line(random::<usize>() % len, LineType::LF_CR);
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("line_lf", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::All);
        bench.iter(|| {
            rope.line(random::<usize>() % len, LineType::All);
        })
    });

    group.bench_function("chunk", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        bench.iter(|| {
            rope.chunk(random::<usize>() % (len + 1));
        })
    });

    group.bench_function("chunk_slice", |bench| {
        let rope = Rope::from_str(&large_string());
        let slice = rope.slice(324..(rope.len_bytes() - 213));
        let len = slice.len_bytes();
        bench.iter(|| {
            slice.chunk(random::<usize>() % (len + 1));
        })
    });
}

fn slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("slice");

    group.bench_function("slice", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        bench.iter(|| {
            let mut start = random::<usize>() % (len + 1);
            let mut end = random::<usize>() % (len + 1);
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
            rope.slice(start..end);
        })
    });

    group.bench_function("slice_small", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        bench.iter(|| {
            let mut start = random::<usize>() % (len + 1);
            if start > (len - 65) {
                start = len - 65;
            }
            let end = start + 64;
            rope.slice(start..end);
        })
    });

    group.bench_function("slice_from_small_rope", |bench| {
        let rope = Rope::from_str(TEXT_SMALL);
        let len = rope.len_bytes();
        bench.iter(|| {
            let mut start = random::<usize>() % (len + 1);
            let mut end = random::<usize>() % (len + 1);
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
            rope.slice(start..end);
        })
    });

    group.bench_function("slice_whole_rope", |bench| {
        let rope = Rope::from_str(&large_string());
        bench.iter(|| {
            rope.slice(..);
        })
    });

    group.bench_function("slice_whole_slice", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_bytes();
        let slice = rope.slice(1..len - 1);
        bench.iter(|| {
            slice.slice(..);
        })
    });
}

//----

criterion_group!(benches, index_convert, get, slice,);
criterion_main!(benches);
