extern crate criterion;
extern crate ropey;

use criterion::{criterion_group, criterion_main, Criterion};
use ropey::Rope;

const TEXT: &str = include_str!("large.txt");
const TEXT_TINY: &str = include_str!("tiny.txt");

//----

fn iter_prev(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_prev");

    group.bench_function("bytes", |bench| {
        let r = Rope::from_str(TEXT);
        let itr_src = r.bytes_at(r.len_bytes());
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });

    group.bench_function("chars", |bench| {
        let r = Rope::from_str(TEXT);
        let itr_src = r.chars_at(r.len_chars());
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });

    group.bench_function("chunks", |bench| {
        let r = Rope::from_str(TEXT);
        let itr_src = r.chunks_at_char(r.len_chars()).0;
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });
}

fn iter_prev_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_prev_lines");

    group.bench_function("lines", |bench| {
        let r = Rope::from_str(TEXT);
        let itr_src = r.lines_at(r.len_lines());
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });

    group.bench_function("lines_tiny", |bench| {
        let r = Rope::from_str(TEXT_TINY);
        let itr_src = r.lines_at(r.len_lines());
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });
}

fn iter_next(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_next");

    group.bench_function("bytes", |bench| {
        let r = Rope::from_str(TEXT);
        let mut itr = r.bytes().cycle();
        bench.iter(|| {
            itr.next();
        })
    });

    group.bench_function("chars", |bench| {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chars().cycle();
        bench.iter(|| {
            itr.next();
        })
    });

    group.bench_function("chunks", |bench| {
        let r = Rope::from_str(TEXT);
        let mut itr = r.chunks().cycle();
        bench.iter(|| {
            itr.next();
        })
    });
}

fn iter_next_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_next_lines");

    group.bench_function("lines", |bench| {
        let r = Rope::from_str(TEXT);
        let mut itr = r.lines().cycle();
        bench.iter(|| {
            itr.next();
        })
    });

    group.bench_function("lines_tiny", |bench| {
        let r = Rope::from_str(TEXT_TINY);
        let mut itr = r.lines().cycle();
        bench.iter(|| {
            itr.next();
        })
    });
}

fn iter_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_create");

    group.bench_function("bytes", |bench| {
        let r = Rope::from_str(TEXT);
        bench.iter(|| {
            r.bytes();
        })
    });

    group.bench_function("chars", |bench| {
        let r = Rope::from_str(TEXT);
        bench.iter(|| {
            r.chars();
        })
    });

    group.bench_function("lines", |bench| {
        let r = Rope::from_str(TEXT);
        bench.iter(|| {
            r.lines();
        })
    });

    group.bench_function("chunks", |bench| {
        let r = Rope::from_str(TEXT);
        bench.iter(|| {
            r.chunks();
        })
    });
}

fn iter_create_at(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_create_at");

    group.bench_function("bytes", |bench| {
        let r = Rope::from_str(TEXT);
        let len = r.len_bytes();
        let mut i = 0;
        bench.iter(|| {
            r.bytes_at(i % (len + 1));
            i += 1;
        })
    });

    group.bench_function("chars", |bench| {
        let r = Rope::from_str(TEXT);
        let len = r.len_chars();
        let mut i = 0;
        bench.iter(|| {
            r.chars_at(i % (len + 1));
            i += 1;
        })
    });

    group.bench_function("lines", |bench| {
        let r = Rope::from_str(TEXT);
        let len = r.len_lines();
        let mut i = 0;
        bench.iter(|| {
            r.lines_at(i % (len + 1));
            i += 1;
        })
    });

    group.bench_function("chunks_at_byte", |bench| {
        let r = Rope::from_str(TEXT);
        let len = r.len_bytes();
        let mut i = 0;
        bench.iter(|| {
            r.chunks_at_byte(i % (len + 1));
            i += 1;
        })
    });

    group.bench_function("chunks_at_char", |bench| {
        let r = Rope::from_str(TEXT);
        let len = r.len_chars();
        let mut i = 0;
        bench.iter(|| {
            r.chunks_at_char(i % (len + 1));
            i += 1;
        })
    });

    group.bench_function("chunks_at_line_break", |bench| {
        let r = Rope::from_str(TEXT);
        let len = r.len_lines();
        let mut i = 0;
        bench.iter(|| {
            r.chunks_at_line_break(i % (len + 1));
            i += 1;
        })
    });
}

//----

criterion_group!(
    benches,
    iter_prev,
    iter_prev_lines,
    iter_next,
    iter_next_lines,
    iter_create,
    iter_create_at,
);
criterion_main!(benches);
