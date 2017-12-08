//! A ut8 text rope for efficient editing of large texts.
//!
//! Ropey provides a unicode-aware implementation of text ropes for Rust.
//! It does double-duty as both the mutate-in-place and copy-on-write
//! variants of text ropes (it just depends on how you use the APIs).
//!
//! Ropey is designed with the following in mind:
//!
//! - **Strong Unicode support.**  Ropey treats `char`'s (unicode code points)
//!   as the base unit of text.  You can index into, slice by, and iterate over
//!   a Ropey-rope by `char` index.
//! - **Line-aware.**  Ropey maintains meta-data about line breaks, allowing
//!   you to index into and iterate over lines of text.
//! - **Grapheme-friendly.**  Ropey ensures that graphemes are never split in its
//!   internal representation of text, and provides API's for iterating over
//!   graphemes and querying about grapheme boundaries.
//!   grapheme bounda.  This means you can always get a complete
//!   zero-copy `&str` slice of any grapheme in the text.
//! - **Frequent edits of large texts.**  Ropey is intended to be used for text
//!   editing and manipulation, including when the text is hundreds of megabytes
//!   large and the edits are all over the place.
//! - **Low-level access when needed.**  Although Ropey doesn't expose anything that
//!   could lead to invalid text states, it does provide APIs for working with
//!   the text as bytes, and for reading the rope's text as larger contiguous
//!   chunks.
//! - **Thread safety.** Data is shared between clones of Ropey-ropes, making
//!   clones extremley cheap. This is entirely thread safe, and clones can be
//!   freely sent between threads.  More memory is only taken up incrementally
//!   as edits cause the clones to diverge.
//! - **Efficiency.**  All of the above is fast and minimizes memory usage.


extern crate arrayvec;
extern crate smallvec;
extern crate unicode_segmentation;

mod node;
mod rope;
mod small_string;
mod small_string_utils;
mod text_info;

pub mod iter;
pub mod slice;

pub use rope::Rope;


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
