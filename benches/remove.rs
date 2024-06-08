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

fn mul_string_length(text: &str, n: usize) -> String {
    let mut mtext = String::new();
    for _ in 0..n {
        mtext.push_str(text);
    }
    mtext
}

//----

fn remove_small(c: &mut Criterion) {
    let text = large_string();

    let mut group = c.benchmark_group("remove_small");

    group.bench_function("random", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = random::<usize>() % (len + 1);
            let end = (start + 1).min(len);
            rope.remove(start..end);

            if rope.len() == text.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("start", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = 0;
            let end = (start + 1).min(len);
            rope.remove(start..end);

            if rope.len() == text.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("middle", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = len / 2;
            let end = (start + 1).min(len);
            rope.remove(start..end);

            if rope.len() == text.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("end", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let end = len;
            let start = end - (1).min(len);
            rope.remove(start..end);

            if rope.len() == text.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });
}

fn remove_medium(c: &mut Criterion) {
    let text = large_string();

    let mut group = c.benchmark_group("remove_medium");

    group.bench_function("random", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = random::<usize>() % (len + 1);
            let end = (start + 15).min(len);
            rope.remove(start..end);

            if rope.len() == text.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("start", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = 0;
            let end = (start + 15).min(len);
            rope.remove(start..end);

            if rope.len() == text.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("middle", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = len / 2;
            let end = (start + 15).min(len);
            rope.remove(start..end);

            if rope.len() == text.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("end", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let end = len;
            let start = end - (15).min(len);
            rope.remove(start..end);

            if rope.len() == text.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });
}

fn remove_large(c: &mut Criterion) {
    let text = mul_string_length(&large_string(), 16);
    let removal_len = 6000;

    let mut group = c.benchmark_group("remove_large");

    group.bench_function("random", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = random::<usize>() % (len + 1);
            let end = (start + removal_len).min(len);
            rope.remove(start..end);

            if rope.len() == 0 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("start", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = 0;
            let end = (start + removal_len).min(len);
            rope.remove(start..end);

            if rope.len() == 0 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("middle", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let start = len / 2;
            let end = (start + removal_len).min(len);
            rope.remove(start..end);

            if rope.len() == 0 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("end", |bench| {
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len();
            let end = len;
            let start = end - removal_len.min(len);
            rope.remove(start..end);

            if rope.len() == 0 {
                rope = Rope::from_str(&text);
            }
        })
    });
}

fn remove_initial_after_clone(c: &mut Criterion) {
    let text = large_string();

    c.bench_function("remove_initial_after_clone", |bench| {
        let rope = Rope::from_str(&text);
        let mut rope_clone = rope.clone();
        let mut i = 0;
        bench.iter(|| {
            if i > 32 {
                i = 0;
                rope_clone = rope.clone();
            }
            let len = rope_clone.len();
            let start = random::<usize>() % (len + 1);
            let end = (start + 1).min(len);
            rope_clone.remove(start..end);
            i += 1;
        })
    });
}

//----

criterion_group!(
    benches,
    remove_small,
    remove_medium,
    remove_large,
    remove_initial_after_clone
);
criterion_main!(benches);
