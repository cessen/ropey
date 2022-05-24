extern crate ropey;

use std::iter::Iterator;

use ropey::Rope;

const TEXT: &str = include_str!("test_text.txt");

#[test]
#[cfg_attr(miri, ignore)]
fn clone_rope() {
    let mut rope1 = Rope::from_str(TEXT);
    let mut rope2 = rope1.clone();

    // Do identical insertions into both ropes
    rope1.insert(432, "Hello ");
    rope1.insert(2345, "world! ");
    rope1.insert(5256, "How are ");
    rope1.insert(53, "you ");
    rope1.insert(768, "doing?\r\n");

    rope2.insert(432, "Hello ");
    rope2.insert(2345, "world! ");
    rope2.insert(5256, "How are ");
    rope2.insert(53, "you ");
    rope2.insert(768, "doing?\r\n");

    // Make sure they match
    let matches = Iterator::zip(rope1.chars(), rope2.chars())
        .map(|(a, b)| a == b)
        .all(|n| n);
    assert!(matches);

    // Insert something into the clone, and make sure they don't match
    // afterwards.
    rope2.insert(3891, "I'm doing fine, thanks!");
    let matches = Iterator::zip(rope1.chars(), rope2.chars())
        .map(|(a, b)| a == b)
        .all(|n| n);
    assert!(!matches);
}
