extern crate ropey;

use std::hash::{Hash, Hasher};

use ropey::RopeBuilder;

const SMALL_TEXT: &str = include_str!("small_ascii.txt");

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

#[test]
#[cfg_attr(miri, ignore)]
fn hash_1() {
    // Build two ropes with the same contents but different chunk boundaries.
    let r1 = {
        let mut b = RopeBuilder::new();
        b._append_chunk("Hello w");
        b._append_chunk("orld");
        b._finish_no_fix()
    };
    let r2 = {
        let mut b = RopeBuilder::new();
        b._append_chunk("Hell");
        b._append_chunk("o world");
        b._finish_no_fix()
    };

    let mut hasher1 = TestHasher::default();
    let mut hasher2 = TestHasher::default();
    r1.hash(&mut hasher1);
    r2.hash(&mut hasher2);

    assert_eq!(hasher1.finish(), hasher2.finish());
}

#[test]
#[cfg_attr(miri, ignore)]
fn hash_2() {
    // Build two ropes with the same contents but different chunk boundaries.
    let r1 = {
        let mut b = RopeBuilder::new();
        for chunk in SMALL_TEXT.as_bytes().chunks(5) {
            b._append_chunk(std::str::from_utf8(chunk).unwrap());
        }
        b._finish_no_fix()
    };
    let r2 = {
        let mut b = RopeBuilder::new();
        for chunk in SMALL_TEXT.as_bytes().chunks(7) {
            b._append_chunk(std::str::from_utf8(chunk).unwrap());
        }
        b._finish_no_fix()
    };

    for (l1, l2) in r1.lines().zip(r2.lines()) {
        let mut hasher1 = TestHasher::default();
        let mut hasher2 = TestHasher::default();
        l1.hash(&mut hasher1);
        l2.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish());
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn hash_3() {
    // Build two ropes with the same contents but different chunk boundaries.
    let r1 = {
        let mut b = RopeBuilder::new();
        for chunk in SMALL_TEXT.as_bytes().chunks(521) {
            b._append_chunk(std::str::from_utf8(chunk).unwrap());
        }
        b._finish_no_fix()
    };
    let r2 = {
        let mut b = RopeBuilder::new();
        for chunk in SMALL_TEXT.as_bytes().chunks(547) {
            b._append_chunk(std::str::from_utf8(chunk).unwrap());
        }
        b._finish_no_fix()
    };

    let mut hasher1 = TestHasher::default();
    let mut hasher2 = TestHasher::default();
    r1.hash(&mut hasher1);
    r2.hash(&mut hasher2);

    assert_eq!(hasher1.finish(), hasher2.finish());
}
