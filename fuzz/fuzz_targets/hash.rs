#![no_main]

use libfuzzer_sys::{
    arbitrary::{self, Arbitrary},
    fuzz_target,
};
use ropey::Rope;
use std::hash::{Hasher, Hash};

const SMALL_TEXT: &str = include_str!("small.txt");

/// This is an example `Hasher` to demonstrate a property guaranteed by
/// the documentation that is not exploited by the default `Hasher` (SipHash)
/// Relevant excerpt from the `Hasher` documentation:
/// > Nor can you assume that adjacent
/// > `write` calls are merged, so it's possible, for example, that
/// > ```
/// > # fn foo(hasher: &mut impl std::hash::Hasher) {
/// > hasher.write(&[1, 2]);
/// > hasher.write(&[3, 4, 5, 6]);
/// > # }
/// > ```
/// > and
/// > ```
/// > # fn foo(hasher: &mut impl std::hash::Hasher) {
/// > hasher.write(&[1, 2, 3, 4]);
/// > hasher.write(&[5, 6]);
/// > # }
/// > ```
/// > end up producing different hashes.
///
/// This dummy hasher simply collects all bytes and inserts a separator byte (0xFF) at the end of `write`.
/// While this hasher might seem a little silly, it is perfectly inline with the std documentation.
/// Many other commonly used high performance `Hasher`s (fxhash, ahash, fnvhash) exploit the same property
/// to improve the performance of `write`, so violating this property will cause issues in practice.
#[derive(Default)]
struct TestHasher(std::collections::hash_map::DefaultHasher);
impl Hasher for TestHasher {
    fn finish(&self) -> u64 {
        self.0.finish() 
   }

    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
        self.0.write_u8(0xFF);
    }
}

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
    
    

    // shift chunk bounderies

    let r2 = Rope::from_str(&r.to_string()); 
    for (line1, line2) in r.lines().zip(r2.lines()) {
        let mut hasher1 = TestHasher::default();
        let mut hasher2 = TestHasher::default();
        line1.hash(&mut hasher1);    
        line2.hash(&mut hasher2);
        if hasher1.finish() != hasher2.finish(){
            assert_ne!(line1, line2)
        }
    }
});
