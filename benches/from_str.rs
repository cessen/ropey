#[macro_use]
extern crate bencher;
extern crate ropey;

use bencher::Bencher;
use ropey::Rope;

const TEXT_SMALL: &str = include_str!("small.txt");
const TEXT_MEDIUM: &str = include_str!("medium.txt");
const TEXT_LARGE: &str = include_str!("large.txt");

//----

fn from_str_small(bench: &mut Bencher) {
    bench.iter(|| {
        Rope::from_str(TEXT_SMALL);
    });

    bench.bytes = TEXT_SMALL.len() as u64;
}

fn from_str_medium(bench: &mut Bencher) {
    bench.iter(|| {
        Rope::from_str(TEXT_MEDIUM);
    });

    bench.bytes = TEXT_MEDIUM.len() as u64;
}

fn from_str_large(bench: &mut Bencher) {
    bench.iter(|| {
        Rope::from_str(TEXT_LARGE);
    });

    bench.bytes = TEXT_LARGE.len() as u64;
}

//----

benchmark_group!(benches, from_str_small, from_str_medium, from_str_large);
benchmark_main!(benches);
