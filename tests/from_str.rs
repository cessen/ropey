extern crate ropey;

use ropey::Rope;

const TEXT: &str = include_str!("test_text.txt");

#[test]
#[cfg_attr(miri, ignore)]
fn from_str() {
    // Build rope from file contents
    let rope = Rope::from_str(TEXT);

    // Verify rope integrity
    rope.assert_integrity();
    rope.assert_invariants();

    // Verify that they match
    assert_eq!(rope, TEXT);
}
