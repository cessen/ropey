extern crate rand;
extern crate ropey;

use rand::RngExt;
use ropey::Rope;

#[test]
#[cfg_attr(miri, ignore)]
fn shrink_to_fit() {
    let mut rng = rand::rng();
    let mut rope = Rope::new();

    // Do a bunch of random incoherent inserts
    for _ in 0..(1 << 12) {
        let len = rope.len_chars().max(1);
        rope.insert(rng.random_range(0..len), "Hello ");
        rope.insert(rng.random_range(0..len), "world! ");
        rope.insert(rng.random_range(0..len), "How are ");
        rope.insert(rng.random_range(0..len), "you ");
        rope.insert(rng.random_range(0..len), "doing?\r\n");
        rope.insert(rng.random_range(0..len), "Let's ");
        rope.insert(rng.random_range(0..len), "keep ");
        rope.insert(rng.random_range(0..len), "inserting ");
        rope.insert(rng.random_range(0..len), "more ");
        rope.insert(rng.random_range(0..len), "items.\r\n");
        rope.insert(rng.random_range(0..len), "こんいちは、");
        rope.insert(rng.random_range(0..len), "みんなさん！");
    }

    let rope2 = rope.clone();
    rope.shrink_to_fit();

    assert_eq!(rope, rope2);
    assert!(rope.capacity() < rope2.capacity());

    // Make sure the rope is sound
    rope.assert_integrity();
    rope.assert_invariants();

    rope2.assert_integrity();
    rope2.assert_invariants();
}
