//! Randomized tests to try to catch crlf seam errors.

extern crate fastrand;
extern crate ropey;

use ropey::Rope;

#[test]
#[cfg_attr(miri, ignore)]
fn crlf_inserts() {
    let mut rng = fastrand::Rng::new();
    let mut tree = Rope::new();

    // Do a bunch of random incoherent inserts of CRLF
    // pairs.
    for _ in 0..(1 << 12) {
        let len = tree.len_chars().max(1);
        tree.insert(rng.usize(0..len), "\r\n\r\n");
        tree.insert(rng.usize(0..len), "\n\r\n\r");
        tree.insert(rng.usize(0..len), "\r\n\r\n");
        tree.insert(rng.usize(0..len), "\n\r\n\r");
        tree.insert(rng.usize(0..len), "\r\n\r\n");
        tree.insert(rng.usize(0..len), "こんいちは、");
        tree.insert(rng.usize(0..len), "\n\r\n\r");
        tree.insert(rng.usize(0..len), "\r\n\r\n");
        tree.insert(rng.usize(0..len), "\n\r\n\r");
        tree.insert(rng.usize(0..len), "\r\n\r\n");
        tree.insert(rng.usize(0..len), "\n\r\n\r");
        tree.insert(rng.usize(0..len), "みんなさん！");

        // Make sure the tree is sound
        tree.assert_invariants();
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn crlf_removals() {
    let mut rng = fastrand::Rng::new();
    let mut tree = Rope::new();

    // Build tree.
    for _ in 0..(1 << 9) {
        let len = tree.len_chars().max(1);
        tree.insert(rng.usize(0..len), "\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\nこんいちは、\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\nこんいちは、r\n\r\n\r\n\r\nみんなさん！\n\r\n\r\n\r\nこんいちは、\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\nみんなさん！\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\r\n\r\n\r\n\r\n\r\n\r\nみんなさん！\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\rみんなさん！\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r");
    }

    // Do a bunch of random incoherent removals
    for _ in 0..(1 << 11) {
        let start = rng.usize(0..tree.len_chars().max(1));
        let end = (start + 5).min(tree.len_chars());
        tree.remove(start..end);

        let start = rng.usize(0..tree.len_chars().max(1));
        let end = (start + 9).min(tree.len_chars());
        tree.remove(start..end);

        // Make sure the tree is sound
        tree.assert_invariants();
    }
}
