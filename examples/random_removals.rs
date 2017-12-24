extern crate rand;
extern crate ropey;

use ropey::Rope;
use rand::Rng;

fn main() {
    let mut rng = rand::thread_rng();
    let mut tree = Rope::new();

    // Build up a tree
    for _ in 0..(1 << 20) {
        let len = tree.len_chars();
        tree.insert(len, "Hello world! How are you doing? Let's keep inserting more items.\r\nこんいちは、みんなさん！ ");
    }

    println!(
        "Document size: {:.2}MB",
        tree.len_bytes() as f32 / 1000000.0
    );

    let mut remove_count = 0;
    for _ in 0..(1 << 20) {
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);
        let start = rng.gen::<usize>() % tree.len_chars().max(1);
        let end = (start + 6).min(tree.len_chars());
        tree.remove(start, end);

        remove_count += 12;
    }

    println!("Removals: {}", remove_count);
    println!(
        "Final document size: {:.2}MB",
        tree.len_bytes() as f32 / 1000000.0
    );
}
