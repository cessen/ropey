extern crate ropey;
extern crate rand;

use ropey::Rope;
use rand::Rng;

fn main() {
    let mut rng = rand::thread_rng();
    let mut tree = Rope::new();
    let mut insert_count = 0;

    for _ in 0..(1 << 20) {
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

        insert_count += 12;
    }

    println!("Inserts: {}", insert_count);
    println!(
        "Final document size: {:.2}MB",
        tree.len_bytes() as f32 / 1000000.0
    );
}
