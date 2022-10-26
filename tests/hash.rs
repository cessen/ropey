extern crate ropey;

use std::hash::{Hash, Hasher};

use ropey::Rope;

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

#[test]
#[cfg_attr(miri, ignore)]
fn hash_1() {
    // Build rope from file contents
    let mut r = Rope::from_str(SMALL_TEXT);

    r.insert(0, "\0");
    // Verify rope integrity
    r.assert_integrity();
    r.assert_invariants();

    check_line_hashes(r)
}

fn check_line_hashes(r: Rope) {
    let r2 = Rope::from_str(&r.to_string());
    for (line1, line2) in r.lines().zip(r2.lines()) {
        let mut hasher1 = TestHasher::default();
        let mut hasher2 = TestHasher::default();
        line1.hash(&mut hasher1);
        line2.hash(&mut hasher2);
        if hasher1.finish() != hasher2.finish() {
            assert_ne!(line1, line2)
        }
    }
}
