extern crate criterion;
extern crate fnv;
extern crate fxhash;
extern crate ropey;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fnv::FnvHasher;
use fxhash::FxHasher;
use ropey::Rope;

const TEXT: &str = include_str!("large.txt");
const TEXT_SMALL: &str = include_str!("small.txt");
const TEXT_TINY: &str = "hello";

//----

fn hash_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_large");

    group.bench_function("default", |bench| {
        let r = Rope::from_str(TEXT);
        bench.iter(|| {
            let mut hasher = DefaultHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });

    group.bench_function("fnv", |bench| {
        let r = Rope::from_str(TEXT);
        bench.iter(|| {
            let mut hasher = FnvHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });

    group.bench_function("fxhash", |bench| {
        let r = Rope::from_str(TEXT);
        bench.iter(|| {
            let mut hasher = FxHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });
}

fn hash_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_small");

    group.bench_function("default", |bench| {
        let r = Rope::from_str(TEXT_SMALL);
        bench.iter(|| {
            let mut hasher = DefaultHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });

    group.bench_function("fnv", |bench| {
        let r = Rope::from_str(TEXT_SMALL);
        bench.iter(|| {
            let mut hasher = FnvHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });

    group.bench_function("fxhash", |bench| {
        let r = Rope::from_str(TEXT_SMALL);
        bench.iter(|| {
            let mut hasher = FxHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });
}

fn hash_tiny(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_tiny");

    group.bench_function("default", |bench| {
        let r = Rope::from_str(TEXT_TINY);
        bench.iter(|| {
            let mut hasher = DefaultHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });

    group.bench_function("fnv", |bench| {
        let r = Rope::from_str(TEXT_TINY);
        bench.iter(|| {
            let mut hasher = FnvHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });

    group.bench_function("fxhash", |bench| {
        let r = Rope::from_str(TEXT_TINY);
        bench.iter(|| {
            let mut hasher = FxHasher::default();
            r.hash(black_box(&mut hasher));
            black_box(hasher.finish());
        })
    });
}

//----

criterion_group!(benches, hash_large, hash_small, hash_tiny,);
criterion_main!(benches);
