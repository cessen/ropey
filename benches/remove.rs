extern crate criterion;
extern crate rand;
extern crate ropey;

use criterion::{criterion_group, criterion_main, Criterion};
use rand::random;
use ropey::Rope;

const TEXT: &str = include_str!("large.txt");
const TEXT_SMALL: &str = include_str!("small.txt");

fn mul_string_length(text: &str, n: usize) -> String {
    let mut mtext = String::new();
    for _ in 0..n {
        mtext.push_str(text);
    }
    mtext
}

//----

const LEN_MUL_SMALL: usize = 1;

fn remove_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_small");

    group.bench_function("random", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_SMALL);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = random::<usize>() % (len + 1);
            let end = (start + 1).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == TEXT.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("start", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_SMALL);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = 0;
            let end = (start + 1).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == TEXT.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("middle", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_SMALL);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = len / 2;
            let end = (start + 1).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == TEXT.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("end", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_SMALL);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let end = len;
            let start = end - (1).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == TEXT.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });
}

const LEN_MUL_MEDIUM: usize = 1;

fn remove_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_medium");

    group.bench_function("random", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_MEDIUM);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = random::<usize>() % (len + 1);
            let end = (start + 15).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == TEXT.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("start", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_MEDIUM);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = 0;
            let end = (start + 15).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == TEXT.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("middle", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_MEDIUM);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = len / 2;
            let end = (start + 15).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == TEXT.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("end", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_MEDIUM);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let end = len;
            let start = end - (15).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == TEXT.len() / 2 {
                rope = Rope::from_str(&text);
            }
        })
    });
}

const LEN_MUL_LARGE: usize = 4;

fn remove_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_large");

    group.bench_function("random", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_LARGE);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = random::<usize>() % (len + 1);
            let end = (start + TEXT_SMALL.len()).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == 0 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("start", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_LARGE);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = 0;
            let end = (start + TEXT_SMALL.len()).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == 0 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("middle", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_LARGE);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let start = len / 2;
            let end = (start + TEXT_SMALL.len()).min(len);
            rope.remove(start..end);

            if rope.len_bytes() == 0 {
                rope = Rope::from_str(&text);
            }
        })
    });

    group.bench_function("end", |bench| {
        let text = mul_string_length(TEXT, LEN_MUL_LARGE);
        let mut rope = Rope::from_str(&text);

        bench.iter(|| {
            let len = rope.len_chars();
            let end = len;
            let start = end - TEXT_SMALL.len().min(len);
            rope.remove(start..end);

            if rope.len_bytes() == 0 {
                rope = Rope::from_str(&text);
            }
        })
    });
}

fn remove_initial_after_clone(c: &mut Criterion) {
    c.bench_function("remove_initial_after_clone", |bench| {
        let rope = Rope::from_str(TEXT);
        let mut rope_clone = rope.clone();
        let mut i = 0;
        bench.iter(|| {
            if i > 32 {
                i = 0;
                rope_clone = rope.clone();
            }
            let len = rope_clone.len_chars();
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
