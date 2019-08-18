#[macro_use]
extern crate bencher;
extern crate ropey;

use bencher::Bencher;
use ropey::Rope;

const TEXT: &str = include_str!("large.txt");
const TEXT_TINY: &str = include_str!("tiny.txt");

//----

fn create_bytes_iter(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    bench.iter(|| {
        r.bytes();
    });
}

fn create_bytes_iter_at(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let len = r.len_bytes();
    let mut i = 0;
    bench.iter(|| {
        r.bytes_at(i % (len + 1));
        i += 1;
    });
}

fn create_bytes_iter_at_end(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let len = r.len_bytes();
    bench.iter(|| {
        r.bytes_at(len);
    });
}

fn create_chars_iter(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    bench.iter(|| {
        r.chars();
    });
}

fn create_chars_iter_at(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let len = r.len_chars();
    let mut i = 0;
    bench.iter(|| {
        r.chars_at(i % (len + 1));
        i += 1;
    });
}

fn create_chars_iter_at_end(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let len = r.len_chars();
    bench.iter(|| {
        r.chars_at(len);
    });
}

fn create_lines_iter(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    bench.iter(|| {
        r.lines();
    });
}

fn create_lines_iter_at(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let len = r.len_lines();
    let mut i = 0;
    bench.iter(|| {
        r.lines_at(i % (len + 1));
        i += 1;
    });
}

fn create_chunks_iter(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    bench.iter(|| {
        r.chunks();
    });
}

fn create_chunks_iter_at_byte(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let len = r.len_bytes();
    let mut i = 0;
    bench.iter(|| {
        r.chunks_at_byte(i % (len + 1));
        i += 1;
    });
}

fn create_chunks_iter_at_char(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let len = r.len_chars();
    let mut i = 0;
    bench.iter(|| {
        r.chunks_at_char(i % (len + 1));
        i += 1;
    });
}

fn create_chunks_iter_at_line_break(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let len = r.len_lines();
    let mut i = 0;
    bench.iter(|| {
        r.chunks_at_line_break(i % (len + 1));
        i += 1;
    });
}

//----

fn bytes_iter_next(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let mut itr = r.bytes().cycle();
    bench.iter(|| {
        itr.next();
    });
}

fn bytes_iter_prev(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let itr_src = r.bytes_at(r.len_bytes());
    let mut itr = itr_src.clone();
    bench.iter(|| {
        if let None = itr.prev() {
            itr = itr_src.clone();
        }
    });
}

fn chars_iter_next(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let mut itr = r.chars().cycle();
    bench.iter(|| {
        itr.next();
    });
}

fn chars_iter_prev(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let itr_src = r.chars_at(r.len_chars());
    let mut itr = itr_src.clone();
    bench.iter(|| {
        if let None = itr.prev() {
            itr = itr_src.clone();
        }
    });
}

fn lines_iter_next(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let mut itr = r.lines().cycle();
    bench.iter(|| {
        itr.next();
    });
}

fn lines_iter_next_tiny(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT_TINY);
    let mut itr = r.lines().cycle();
    bench.iter(|| {
        itr.next();
    });
}

fn lines_iter_prev(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let itr_src = r.lines_at(r.len_lines());
    let mut itr = itr_src.clone();
    bench.iter(|| {
        if let None = itr.prev() {
            itr = itr_src.clone();
        }
    });
}

fn lines_iter_prev_tiny(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT_TINY);
    let itr_src = r.lines_at(r.len_lines());
    let mut itr = itr_src.clone();
    bench.iter(|| {
        if let None = itr.prev() {
            itr = itr_src.clone();
        }
    });
}

fn chunks_iter_next(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let mut itr = r.chunks().cycle();
    bench.iter(|| {
        itr.next();
    });
}

fn chunks_iter_prev(bench: &mut Bencher) {
    let r = Rope::from_str(TEXT);
    let itr_src = r.chunks_at_char(r.len_chars()).0;
    let mut itr = itr_src.clone();
    bench.iter(|| {
        if let None = itr.prev() {
            itr = itr_src.clone();
        }
    });
}

//----

benchmark_group!(
    benches,
    create_bytes_iter,
    create_bytes_iter_at,
    create_bytes_iter_at_end,
    create_chars_iter,
    create_chars_iter_at,
    create_chars_iter_at_end,
    create_lines_iter,
    create_lines_iter_at,
    create_chunks_iter,
    create_chunks_iter_at_byte,
    create_chunks_iter_at_char,
    create_chunks_iter_at_line_break,
    bytes_iter_next,
    bytes_iter_prev,
    chars_iter_next,
    chars_iter_prev,
    lines_iter_next,
    lines_iter_next_tiny,
    lines_iter_prev,
    lines_iter_prev_tiny,
    chunks_iter_next,
    chunks_iter_prev,
);
benchmark_main!(benches);
