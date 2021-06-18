#![no_main]

use libfuzzer_sys::{
    arbitrary::{self, Arbitrary},
    fuzz_target,
};
use ropey::Rope;

const SMALL_TEXT: &str = include_str!("small.txt");
const MEDIUM_TEXT: &str = include_str!("medium.txt");

#[derive(Arbitrary, Copy, Clone, Debug)]
enum Op<'a> {
    Insert(usize, &'a str),
    InsertChar(usize, char),
    Remove(usize, usize),
    SplitOff(usize, bool),
    Append(&'a str),
}

#[derive(Arbitrary, Copy, Clone, Debug)]
enum StartingText<'a> {
    Small,
    Medium,
    Custom(&'a str),
}

fuzz_target!(|data: (StartingText, Vec<Op>)| {
    let mut r = Rope::from_str(match data.0 {
        StartingText::Small => SMALL_TEXT,
        StartingText::Medium => MEDIUM_TEXT,
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
            Op::SplitOff(idx, keep_right) => match r.try_split_off(idx) {
                Ok(right) => {
                    if keep_right {
                        r = right;
                    }
                }
                Err(_) => {}
            },
            Op::Append(s) => {
                r.append(Rope::from_str(s));
            }
        }
    }

    r.assert_integrity();
    r.assert_invariants();
});
