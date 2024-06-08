extern crate criterion;
extern crate rand;
extern crate ropey;

use criterion::{criterion_group, criterion_main, Criterion};
use rand::random;
use ropey::Rope;

const TEXT_SMALL: &str = include_str!("small.txt");

fn large_string() -> String {
    let mut text = String::new();
    for _ in 0..1000 {
        text.push_str(TEXT_SMALL);
    }
    text
}

//----

fn insert_char(c: &mut Criterion) {
    let text = large_string();

    let mut group = c.benchmark_group("insert_char");

    group.bench_function("random", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert_char(random::<usize>() % len, 'a')
        })
    });

    group.bench_function("start", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            rope.insert_char(0, 'a');
        })
    });

    group.bench_function("middle", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert_char(len / 2, 'a');
        })
    });

    group.bench_function("end", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert_char(len, 'a');
        })
    });
}

fn insert_small(c: &mut Criterion) {
    let text = large_string();

    let mut group = c.benchmark_group("insert_small");

    group.bench_function("random", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(random::<usize>() % len, "a");
        })
    });

    group.bench_function("start", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            rope.insert(0, "a");
        })
    });

    group.bench_function("middle", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(len / 2, "a");
        })
    });

    group.bench_function("end", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(len, "a");
        })
    });
}

fn insert_medium(c: &mut Criterion) {
    let text = large_string();

    let mut group = c.benchmark_group("insert_medium");

    group.bench_function("random", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(random::<usize>() % len, "This is some text.");
        })
    });

    group.bench_function("start", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            rope.insert(0, "This is some text.");
        })
    });

    group.bench_function("middle", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(len / 2, "This is some text.");
        })
    });

    group.bench_function("end", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(len, "This is some text.");
        })
    });
}

const INSERT_TEXT: &str = include_str!("small.txt");

fn insert_large(c: &mut Criterion) {
    let text = large_string();

    let mut group = c.benchmark_group("insert_large");

    group.bench_function("random", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(random::<usize>() % len, INSERT_TEXT);
        })
    });

    group.bench_function("start", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            rope.insert(0, INSERT_TEXT);
        })
    });

    group.bench_function("middle", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(len / 2, INSERT_TEXT);
        })
    });

    group.bench_function("end", |bench| {
        let mut rope = Rope::from_str(&text);
        bench.iter(|| {
            let len = rope.len();
            rope.insert(len, INSERT_TEXT);
        })
    });
}

//----

fn insert_after_clone(c: &mut Criterion) {
    let text = large_string();

    c.bench_function("insert_after_clone", |bench| {
        let rope = Rope::from_str(&text);
        let mut rope_clone = rope.clone();
        let mut i = 0;
        bench.iter(|| {
            if i > 32 {
                i = 0;
                rope_clone = rope.clone();
            }
            let len = rope_clone.len();
            rope_clone.insert(random::<usize>() % len, "a");
            i += 1;
        })
    });
}

//----

criterion_group!(
    benches,
    insert_char,
    insert_small,
    insert_medium,
    insert_large,
    insert_after_clone
);
criterion_main!(benches);
