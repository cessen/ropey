extern crate btree_rope;
extern crate rand;

use btree_rope::Rope;
use rand::Rng;

fn main() {
    let mut rng = rand::thread_rng();
    let mut tree = Rope::new();

    for _ in 0..(1 << 17) {
        let len = tree.len_chars().max(1);
        tree.insert(rng.gen::<usize>() % len, "Hello ");
        tree.insert(rng.gen::<usize>() % len, "world! ");
        tree.insert(rng.gen::<usize>() % len, "How are ");
        tree.insert(rng.gen::<usize>() % len, "you ");
        tree.insert(rng.gen::<usize>() % len, "doing?\r\n");
        tree.insert(rng.gen::<usize>() % len, "Let's ");
        tree.insert(rng.gen::<usize>() % len, "keep ");
        tree.insert(rng.gen::<usize>() % len, "inserting ");
        tree.insert(rng.gen::<usize>() % len, "more ");
        tree.insert(rng.gen::<usize>() % len, "items.\r\n");
        tree.insert(rng.gen::<usize>() % len, "こんいちは、");
        tree.insert(rng.gen::<usize>() % len, "みんなさん！");
    }
}
