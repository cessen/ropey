extern crate criterion;
extern crate ropey;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ropey::Rope;

const TEXT_SMALL: &str = include_str!("small.txt");
const TEXT_MEDIUM: &str = include_str!("medium.txt");
const TEXT_LARGE: &str = include_str!("large.txt");
const TEXT_LF: &str = include_str!("lf.txt");

//----

fn from_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("from_str");

    group.bench_function("small", |bench| {
        bench.iter(|| {
            Rope::from_str(black_box(TEXT_SMALL));
        })
    });

    group.bench_function("medium", |bench| {
        bench.iter(|| {
            Rope::from_str(black_box(TEXT_MEDIUM));
        })
    });

    group.bench_function("large", |bench| {
        bench.iter(|| {
            Rope::from_str(black_box(TEXT_LARGE));
        })
    });

    group.bench_function("linefeeds", |bench| {
        bench.iter(|| {
            Rope::from_str(black_box(TEXT_LF));
        })
    });
}

fn rope_clone(c: &mut Criterion) {
    let rope = Rope::from_str(TEXT_LARGE);
    c.bench_function("rope_clone", |bench| {
        bench.iter(|| {
            let _ = black_box(&rope).clone();
        })
    });
}

//----

criterion_group!(benches, from_str, rope_clone,);
criterion_main!(benches);
