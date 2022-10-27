extern crate ropey;

use ropey::Rope;

const MEDIUM_TEXT: &str = include_str!("medium.txt");

#[test]
#[cfg_attr(miri, ignore)]
fn remove_at_chunk_boundery() {
    let mut r = Rope::from_str(MEDIUM_TEXT);
    // remove exactly at a chunk boundry
    // to trigger an edgecase in fix_tree_seam
    r.remove(31354..58881);

    // Verify rope integrity
    r.assert_integrity();
    r.assert_invariants();
}
