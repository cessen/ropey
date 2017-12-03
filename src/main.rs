extern crate arrayvec;
extern crate smallvec;
extern crate rand;

mod rope;
mod iter;
mod small_string;
mod small_string_utils;

use rope::Rope;
use rand::Rng;

fn main() {
    let mut rng = rand::thread_rng();

    let mut tree = Rope::new();

    for _ in 0..32 {
        use rope::Count;
        let len = tree.char_count().max(1);
        tree.insert(rng.gen::<Count>() % len, "Hello ");
        tree.insert(rng.gen::<Count>() % len, "world! ");
        tree.insert(rng.gen::<Count>() % len, "How are ");
        tree.insert(rng.gen::<Count>() % len, "you ");
        tree.insert(rng.gen::<Count>() % len, "doing? ");
        tree.insert(rng.gen::<Count>() % len, "Let's ");
        tree.insert(rng.gen::<Count>() % len, "keep ");
        tree.insert(rng.gen::<Count>() % len, "inserting ");
        tree.insert(rng.gen::<Count>() % len, "more ");
        tree.insert(rng.gen::<Count>() % len, "items. ");
        tree.insert(rng.gen::<Count>() % len, "こんいちは、");
        tree.insert(rng.gen::<Count>() % len, "みんなさん！");
    }

    println!("{:#?}", tree);
}


#[cfg(test)]
mod tests {
    use rope::Rope;
    use iter::RopeChunkIter;

    #[test]
    fn insert_01() {
        let mut r = Rope::new();
        r.insert(0, "Hello world!");
        r.insert(3, "zopter");

        assert_eq!("Helzopterlo world!", &r.to_string());
    }

    #[test]
    fn insert_02() {
        let mut r = Rope::new();
        r.insert(0, "Hello world!");
        r.insert(0, "zopter");

        assert_eq!("zopterHello world!", &r.to_string());
    }

    #[test]
    fn insert_03() {
        let mut r = Rope::new();
        r.insert(0, "Hello world!");
        r.insert(12, "zopter");

        assert_eq!("Hello world!zopter", &r.to_string());
    }

    #[test]
    fn insert_04() {
        let mut r = Rope::new();
        r.insert(0, "He");
        r.insert(2, "l");
        r.insert(3, "l");
        r.insert(4, "o w");
        r.insert(7, "o");
        r.insert(8, "rl");
        r.insert(10, "d!");
        r.insert(3, "zopter");

        assert_eq!("Helzopterlo world!", &r.to_string());
    }
}
