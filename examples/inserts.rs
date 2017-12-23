extern crate ropey;

use ropey::Rope;

fn main() {
    let mut tree = Rope::new();

    for _ in 0..16 {
        let len = tree.len_chars().max(1);
        tree.insert(1_298_809 % len, "Hello world! How are you doing?\r\n");
        let len = tree.len_chars().max(1);
        tree.insert(1_298_809 % len, "Let's keep inserting more items.\r\n");
        let len = tree.len_chars().max(1);
        tree.insert(1_298_809 % len, "こんいちは、みんなさん！");
    }

    println!("{}", tree.to_string());
}
