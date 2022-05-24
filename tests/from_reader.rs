extern crate rand;
extern crate ropey;

use std::io::Cursor;

use ropey::Rope;

const TEXT: &str = include_str!("test_text.txt");

#[test]
#[cfg_attr(miri, ignore)]
fn from_reader_01() {
    // Make a reader from our in-memory text
    let text_reader = Cursor::new(TEXT);

    let rope = Rope::from_reader(text_reader).unwrap();

    assert_eq!(rope, TEXT);

    // Make sure the tree is sound
    rope.assert_integrity();
    rope.assert_invariants();
}

#[test]
#[cfg_attr(miri, ignore)]
fn from_reader_02() {
    // Make a reader from blank text
    let text_reader = Cursor::new("");

    let rope = Rope::from_reader(text_reader).unwrap();

    assert_eq!(rope, "");

    // Make sure the tree is sound
    rope.assert_integrity();
    rope.assert_invariants();
}

#[test]
#[cfg_attr(miri, ignore)]
fn from_reader_03() {
    // Make text with a utf8-invalid byte sequence in it.
    let mut text = Vec::new();
    text.extend(TEXT.as_bytes());
    text[6132] = 0b1100_0000;
    text[6133] = 0b0100_0000;

    // Make a reader from the invalid data
    let text_reader = Cursor::new(text);

    // Try to read the data, and verify that we get the right error.
    if let Err(e) = Rope::from_reader(text_reader) {
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
    } else {
        panic!("Should have returned an invalid data error.")
    }
}
