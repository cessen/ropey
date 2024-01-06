extern crate criterion;
extern crate ropey;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ropey::Rope;

const TEXT_SMALL: &str = include_str!("small.txt");

fn medium_string() -> String {
    let mut text = String::new();
    for _ in 0..150 {
        text.push_str(TEXT_SMALL);
    }
    text
}

fn large_string() -> String {
    let mut text = String::new();
    for _ in 0..1000 {
        text.push_str(TEXT_SMALL);
    }
    text
}

fn lf_string() -> String {
    let mut text = String::new();
    for _ in 0..(1 << 10) {
        text.push_str("\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n");
    }
    text
}

//----

fn from_str(c: &mut Criterion) {
    let text_small = TEXT_SMALL;
    let text_medium = medium_string();
    let text_large = large_string();
    let text_lf = lf_string();

    let mut group = c.benchmark_group("from_str");

    group.bench_function("small", |bench| {
        bench.iter(|| {
            Rope::from_str(black_box(&text_small));
        })
    });

    group.bench_function("medium", |bench| {
        bench.iter(|| {
            Rope::from_str(black_box(&text_medium));
        })
    });

    group.bench_function("large", |bench| {
        bench.iter(|| {
            Rope::from_str(black_box(&text_large));
        })
    });

    group.bench_function("linefeeds", |bench| {
        bench.iter(|| {
            Rope::from_str(black_box(&text_lf));
        })
    });
}

fn rope_clone(c: &mut Criterion) {
    let text_large = large_string();

    let rope = Rope::from_str(&text_large);
    c.bench_function("rope_clone", |bench| {
        bench.iter(|| {
            let _ = black_box(&rope).clone();
        })
    });
}

//----

criterion_group!(benches, from_str, rope_clone,);
criterion_main!(benches);
