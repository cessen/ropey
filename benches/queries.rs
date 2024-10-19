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
const TEXT_SMALL_MULTIBYTE: &str = include_str!("small_multibyte.txt");

fn large_string() -> String {
    let mut text = String::new();
    for _ in 0..1000 {
        text.push_str(TEXT_SMALL);
    }
    text
}

fn large_string_multibyte() -> String {
    let mut text = String::new();
    for _ in 0..1000 {
        text.push_str(TEXT_SMALL_MULTIBYTE);
    }
    text
}

//----

fn index_convert(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_convert");

    #[cfg(feature = "metric_chars")]
    group.bench_function("byte_to_char", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len();
        bench.iter(|| {
            rope.byte_to_char_idx(random::<usize>() % (len + 1));
        })
    });

    #[cfg(feature = "metric_chars")]
    group.bench_function("char_to_byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_chars();
        bench.iter(|| {
            rope.char_to_byte_idx(random::<usize>() % (len + 1));
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("byte_to_line_lf", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len();
        bench.iter(|| {
            rope.byte_to_line_idx(random::<usize>() % (len + 1), LineType::LF);
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("line_lf_to_byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::LF);
        bench.iter(|| {
            rope.line_to_byte_idx(random::<usize>() % (len + 1), LineType::LF);
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("byte_to_line_cr_lf", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len();
        bench.iter(|| {
            rope.byte_to_line_idx(random::<usize>() % (len + 1), LineType::LF_CR);
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("line_cr_lf_to_byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::LF_CR);
        bench.iter(|| {
            rope.line_to_byte_idx(random::<usize>() % (len + 1), LineType::LF_CR);
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("byte_to_line_all", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len();
        bench.iter(|| {
            rope.byte_to_line_idx(random::<usize>() % (len + 1), LineType::All);
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("line_all_to_byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::LF_CR);
        bench.iter(|| {
            rope.line_to_byte_idx(random::<usize>() % (len + 1), LineType::All);
        })
    });
}

fn index_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_query");

    group.bench_function("is_char_boundary", |bench| {
        let rope = Rope::from_str(&large_string_multibyte());
        let len = rope.len();
        bench.iter(|| {
            rope.is_char_boundary(random::<usize>() % (len + 1));
        })
    });

    group.bench_function("floor_char_boundary", |bench| {
        let rope = Rope::from_str(&large_string_multibyte());
        let len = rope.len();
        bench.iter(|| {
            rope.floor_char_boundary(random::<usize>() % (len + 1));
        })
    });

    group.bench_function("ceil_char_boundary", |bench| {
        let rope = Rope::from_str(&large_string_multibyte());
        let len = rope.len();
        bench.iter(|| {
            rope.ceil_char_boundary(random::<usize>() % (len + 1));
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("trailing_line_break_idx_lf", |bench| {
        let mut text = large_string();
        text.push_str("\n");
        let rope = Rope::from_str(&text);
        bench.iter(|| {
            rope.trailing_line_break_idx(LineType::LF);
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("trailing_line_break_idx_lf_cr", |bench| {
        let mut text = large_string();
        text.push_str("\r\n");
        let rope = Rope::from_str(&text);
        bench.iter(|| {
            rope.trailing_line_break_idx(LineType::LF_CR);
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("trailing_line_break_idx_unicode", |bench| {
        let mut text = large_string();
        text.push_str("\u{2028}");
        let rope = Rope::from_str(&text);
        bench.iter(|| {
            rope.trailing_line_break_idx(LineType::All);
        })
    });
}

fn get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get");

    group.bench_function("byte", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len();
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
    group.bench_function("line_unicode", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len_lines(LineType::All);
        bench.iter(|| {
            rope.line(random::<usize>() % len, LineType::All);
        })
    });

    group.bench_function("chunk", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len();
        bench.iter(|| {
            rope.chunk(random::<usize>() % (len + 1));
        })
    });

    group.bench_function("chunk_slice", |bench| {
        let rope = Rope::from_str(&large_string());
        let slice = rope.slice(324..(rope.len() - 213));
        let len = slice.len();
        bench.iter(|| {
            slice.chunk(random::<usize>() % (len + 1));
        })
    });
}

fn slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("slice");

    group.bench_function("slice", |bench| {
        let rope = Rope::from_str(&large_string());
        let len = rope.len();
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
        let len = rope.len();
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
        let len = rope.len();
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
        let len = rope.len();
        let slice = rope.slice(1..len - 1);
        bench.iter(|| {
            slice.slice(..);
        })
    });
}

//----

criterion_group!(benches, index_convert, index_query, get, slice,);
criterion_main!(benches);
