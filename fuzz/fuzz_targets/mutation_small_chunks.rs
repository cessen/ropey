#![no_main]

use libfuzzer_sys::{
    arbitrary::{self, Arbitrary},
    fuzz_target,
};
use ropey::Rope;

const SMALL_TEXT: &str = include_str!("small.txt");

#[derive(Arbitrary, Copy, Clone, Debug)]
enum Op<'a> {
    Insert(usize, &'a str),
    InsertChar(usize, char),
    Remove(usize, usize),
}

#[derive(Arbitrary, Copy, Clone, Debug)]
enum StartingText<'a> {
    Small,
    Custom(&'a str),
}

fuzz_target!(|data: (StartingText, Vec<Op>)| {
    let mut r = Rope::from_str(match data.0 {
        StartingText::Small => SMALL_TEXT,
        StartingText::Custom(s) => s,
    });

    for op in data.1 {
        match op {
            Op::Insert(idx, s) => {
                let _ = r.try_insert(idx, s);
            }
            Op::InsertChar(idx, c) => {
                let _ = r.try_insert_char(idx, c);
            }
            Op::Remove(idx_1, idx_2) => {
                let _ = r.try_remove(idx_1..idx_2);
            }
        }
    }

    r.assert_invariants();
});
