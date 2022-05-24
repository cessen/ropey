extern crate rand;
extern crate ropey;

use rand::Rng;
use ropey::Rope;

#[test]
#[cfg_attr(miri, ignore)]
fn shrink_to_fit() {
    let mut rng = rand::thread_rng();
    let mut rope = Rope::new();

    // Do a bunch of random incoherent inserts
    for _ in 0..(1 << 12) {
        let len = rope.len_chars().max(1);
        rope.insert(rng.gen::<usize>() % len, "Hello ");
        rope.insert(rng.gen::<usize>() % len, "world! ");
        rope.insert(rng.gen::<usize>() % len, "How are ");
        rope.insert(rng.gen::<usize>() % len, "you ");
        rope.insert(rng.gen::<usize>() % len, "doing?\r\n");
        rope.insert(rng.gen::<usize>() % len, "Let's ");
        rope.insert(rng.gen::<usize>() % len, "keep ");
        rope.insert(rng.gen::<usize>() % len, "inserting ");
        rope.insert(rng.gen::<usize>() % len, "more ");
        rope.insert(rng.gen::<usize>() % len, "items.\r\n");
        rope.insert(rng.gen::<usize>() % len, "こんいちは、");
        rope.insert(rng.gen::<usize>() % len, "みんなさん！");
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
