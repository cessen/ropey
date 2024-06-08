extern crate criterion;
extern crate ropey;

use criterion::{criterion_group, criterion_main, Criterion};
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

fn iter_prev(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_prev");

    group.bench_function("bytes", |bench| {
        let r = Rope::from_str(&large_string());
        let itr_src = r.bytes_at(r.len());
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });

    group.bench_function("chars", |bench| {
        let r = Rope::from_str(&large_string());
        let itr_src = r.chars_at(r.len());
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("lines_lf", |bench| {
        let r = Rope::from_str(&large_string());
        let itr_src = r.lines_at(r.len_lines(LineType::LF), LineType::LF);
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("lines_cr_lf", |bench| {
        let r = Rope::from_str(&large_string());
        let itr_src = r.lines_at(r.len_lines(LineType::LF_CR), LineType::LF_CR);
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("lines_all", |bench| {
        let r = Rope::from_str(&large_string());
        let itr_src = r.lines_at(r.len_lines(LineType::All), LineType::All);
        let mut itr = itr_src.clone();
        bench.iter(|| {
            if itr.prev().is_none() {
                itr = itr_src.clone();
            }
        })
    });

    group.bench_function("chunks", |bench| {
        let r = Rope::from_str(&large_string());
        let itr_src = r.chunks_at(r.len());
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
        let r = Rope::from_str(&large_string());
        let mut itr = r.bytes().cycle();
        bench.iter(|| {
            itr.next();
        })
    });

    group.bench_function("chars", |bench| {
        let r = Rope::from_str(&large_string());
        let mut itr = r.chars().cycle();
        bench.iter(|| {
            itr.next();
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("lines_lf", |bench| {
        let r = Rope::from_str(&large_string());
        let mut itr = r.lines(LineType::LF).cycle();
        bench.iter(|| {
            itr.next();
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("lines_cr_lf", |bench| {
        let r = Rope::from_str(&large_string());
        let mut itr = r.lines(LineType::LF_CR).cycle();
        bench.iter(|| {
            itr.next();
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("lines_all", |bench| {
        let r = Rope::from_str(&large_string());
        let mut itr = r.lines(LineType::All).cycle();
        bench.iter(|| {
            itr.next();
        })
    });

    group.bench_function("chunks", |bench| {
        let r = Rope::from_str(&large_string());
        let mut itr = r.chunks().cycle();
        bench.iter(|| {
            itr.next();
        })
    });
}

fn iter_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_create");

    group.bench_function("bytes", |bench| {
        let r = Rope::from_str(&large_string());
        bench.iter(|| {
            r.bytes();
        })
    });

    group.bench_function("chars", |bench| {
        let r = Rope::from_str(&large_string());
        bench.iter(|| {
            r.chars();
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("lines_lf", |bench| {
        let r = Rope::from_str(&large_string());
        bench.iter(|| {
            r.lines(LineType::LF);
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("lines_cr_lf", |bench| {
        let r = Rope::from_str(&large_string());
        bench.iter(|| {
            r.lines(LineType::LF_CR);
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("lines_all", |bench| {
        let r = Rope::from_str(&large_string());
        bench.iter(|| {
            r.lines(LineType::All);
        })
    });

    group.bench_function("chunks", |bench| {
        let r = Rope::from_str(&large_string());
        bench.iter(|| {
            r.chunks();
        })
    });
}

fn iter_create_at(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_create_at");

    group.bench_function("bytes", |bench| {
        let r = Rope::from_str(&large_string());
        let len = r.len();
        let mut i = 0;
        bench.iter(|| {
            r.bytes_at(i % (len + 1));
            i += 1;
        })
    });

    group.bench_function("chars", |bench| {
        let r = Rope::from_str(&large_string());
        let len = r.len();
        let mut i = 0;
        bench.iter(|| {
            r.chars_at(i % (len + 1));
            i += 1;
        })
    });

    #[cfg(feature = "metric_lines_lf")]
    group.bench_function("lines_lf", |bench| {
        let r = Rope::from_str(&large_string());
        let len = r.len_lines(LineType::LF);
        let mut i = 0;
        bench.iter(|| {
            r.lines_at(i % (len + 1), LineType::LF);
            i += 1;
        })
    });

    #[cfg(feature = "metric_lines_lf_cr")]
    group.bench_function("lines_cr_lf", |bench| {
        let r = Rope::from_str(&large_string());
        let len = r.len_lines(LineType::LF_CR);
        let mut i = 0;
        bench.iter(|| {
            r.lines_at(i % (len + 1), LineType::LF_CR);
            i += 1;
        })
    });

    #[cfg(feature = "metric_lines_unicode")]
    group.bench_function("lines_all", |bench| {
        let r = Rope::from_str(&large_string());
        let len = r.len_lines(LineType::All);
        let mut i = 0;
        bench.iter(|| {
            r.lines_at(i % (len + 1), LineType::All);
            i += 1;
        })
    });

    group.bench_function("chunks", |bench| {
        let r = Rope::from_str(&large_string());
        let len = r.len();
        let mut i = 0;
        bench.iter(|| {
            r.chunks_at(i % (len + 1));
            i += 1;
        })
    });
}

//----

criterion_group!(benches, iter_prev, iter_next, iter_create, iter_create_at,);
criterion_main!(benches);
