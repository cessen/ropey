extern crate arrayvec;
extern crate smallvec;
extern crate rand;

mod rope;
mod small_string;
mod small_string_utils;

use rope::Rope;
use rand::Rng;

fn main() {
    let mut rng = rand::thread_rng();

    let mut tree = Rope::new();

    for _ in 0..32 {
        let len = tree.char_count().max(1);
        tree.insert(rng.gen::<u32>() % len, "Hello ");
        tree.insert(rng.gen::<u32>() % len, "world! ");
        tree.insert(rng.gen::<u32>() % len, "How are ");
        tree.insert(rng.gen::<u32>() % len, "you ");
        tree.insert(rng.gen::<u32>() % len, "doing? ");
        tree.insert(rng.gen::<u32>() % len, "Let's ");
        tree.insert(rng.gen::<u32>() % len, "keep ");
        tree.insert(rng.gen::<u32>() % len, "inserting ");
        tree.insert(rng.gen::<u32>() % len, "more ");
        tree.insert(rng.gen::<u32>() % len, "items. ");
        tree.insert(rng.gen::<u32>() % len, "こんいちは、");
        tree.insert(rng.gen::<u32>() % len, "みんなさん！");
    }

    println!("{:#?}", tree);
}
