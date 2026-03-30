extern crate fastrand;
extern crate ropey;

use ropey::Rope;

#[test]
#[cfg_attr(miri, ignore)]
fn small_random_inserts() {
    let mut rng = fastrand::Rng::new();
    let mut tree = Rope::new();

    // Do a bunch of random incoherent inserts
    for _ in 0..(1 << 10) {
        let len = tree.len_chars().max(1);
        tree.insert(rng.usize(0..len), "Hello ");
        tree.insert(rng.usize(0..len), "world! ");
        tree.insert(rng.usize(0..len), "How are ");
        tree.insert(rng.usize(0..len), "you ");
        tree.insert(rng.usize(0..len), "doing?\r\n");
        tree.insert(rng.usize(0..len), "Let's ");
        tree.insert(rng.usize(0..len), "keep ");
        tree.insert(rng.usize(0..len), "inserting ");
        tree.insert(rng.usize(0..len), "more ");
        tree.insert(rng.usize(0..len), "items.\r\n");
        tree.insert(rng.usize(0..len), "こんいちは、");
        tree.insert(rng.usize(0..len), "みんなさん！");
    }

    // Make sure the tree is sound
    tree.assert_integrity();
    tree.assert_invariants();
}
