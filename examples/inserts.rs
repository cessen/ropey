extern crate btree_rope;

use btree_rope::Rope;

fn main() {
    let mut tree = Rope::new();

    for _ in 0..16 {
        let len = tree.char_count().max(1);
        tree.insert(1298809 % len, "Hello world! How are you doing?\r\n");
        let len = tree.char_count().max(1);
        tree.insert(1298809 % len, "Let's keep inserting more items.\r\n");
        let len = tree.char_count().max(1);
        tree.insert(1298809 % len, "こんいちは、みんなさん！");
    }

    println!("{}", tree.to_string());
}
