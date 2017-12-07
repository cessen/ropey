extern crate arrayvec;
extern crate rand;
extern crate smallvec;
extern crate unicode_segmentation;

mod rope;
mod iter;
mod small_string;
mod small_string_utils;

use rope::Rope;
use rand::Rng;

const DO_RAND: bool = false;

fn main() {
    let mut rng = rand::thread_rng();

    let mut tree = Rope::new();

    for _ in 0..16 {
        use rope::Count;
        if DO_RAND {
            let len = tree.char_count().max(1);
            tree.insert(rng.gen::<Count>() % len, "Hello ");
            tree.insert(rng.gen::<Count>() % len, "world! ");
            tree.insert(rng.gen::<Count>() % len, "How are ");
            tree.insert(rng.gen::<Count>() % len, "you ");
            tree.insert(rng.gen::<Count>() % len, "doing?\r\n");
            tree.insert(rng.gen::<Count>() % len, "Let's ");
            tree.insert(rng.gen::<Count>() % len, "keep ");
            tree.insert(rng.gen::<Count>() % len, "inserting ");
            tree.insert(rng.gen::<Count>() % len, "more ");
            tree.insert(rng.gen::<Count>() % len, "items.\r\n");
            tree.insert(rng.gen::<Count>() % len, "こんいちは、");
            tree.insert(rng.gen::<Count>() % len, "みんなさん！");
        } else {
            let len = tree.char_count().max(1);
            tree.insert(1298809 % len, "Hello world! How are you doing?\r\n");
            let len = tree.char_count().max(1);
            tree.insert(1298809 % len, "Let's keep inserting more items.\r\n");
            let len = tree.char_count().max(1);
            tree.insert(1298809 % len, "こんいちは、みんなさん！");
        }
    }

    // println!("{:#?}", tree);
    println!("{}", tree.to_string());
}


#[cfg(test)]
mod tests {
    use rope::Rope;

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

    #[test]
    fn insert_05() {
        let mut r = Rope::new();
        r.insert(0, "こんいちは、みんなさん！");
        r.insert(7, "zopter");
        assert_eq!("こんいちは、みzopterんなさん！", &r.to_string());
    }

    #[test]
    fn insert_06() {
        let mut r = Rope::new();
        r.insert(0, "こ");
        r.insert(1, "ん");
        r.insert(2, "い");
        r.insert(3, "ち");
        r.insert(4, "は");
        r.insert(5, "、");
        r.insert(6, "み");
        r.insert(7, "ん");
        r.insert(8, "な");
        r.insert(9, "さ");
        r.insert(10, "ん");
        r.insert(11, "！");
        r.insert(7, "zopter");
        assert_eq!("こんいちは、みzopterんなさん！", &r.to_string());
    }
}
