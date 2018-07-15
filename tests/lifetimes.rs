//! This test file ensures that all of the lifetimes work the way we
//! want, and that there are no regressions. It's a "does this compile?"
//! test.
#![allow(unused)]

extern crate ropey;

use ropey::{Rope, RopeSlice};

const TEXT: &str = include_str!("test_text.txt");

fn main() {
    let rope = Rope::from_str(TEXT);

    // The lifetimes of the intermediate slices shouldn't matter,
    // and all of this should compile.  The lifetimes of the things
    // produced by these calls should be tied to the lifetime of the
    // original rope, not the lifetime of the slice they were created
    // from.
    let a = rope.slice(4..500).slice(4..400).slice(4..300);
    let b = rope.slice(4..500).slice(4..400).as_str();
    let c = rope.slice(4..500).slice(4..400).line(1);
    let d = rope.slice(4..500).slice(4..400).chunk_at_byte(50);
    let e = rope.slice(4..500).slice(4..400).chunk_at_char(50);
    let f = rope.slice(4..500).slice(4..400).chunk_at_line_break(3);

    // Same for iterators.  In addition, the items _yielded_ by the
    // iterators should also be tied to the lifetime of the original
    // rope, not to the iterator or slice they came from.

    let mut count = 0;
    for _ in rope.slice(4..500).slice(4..400).bytes() {
        count += 1;
    }
    for _ in rope.slice(4..500).slice(4..400).chars() {
        count += 1;
    }

    let mut line = RopeSlice::from_str("");
    for l in rope.slice(4..500).slice(4..400).lines() {
        line = l;
    }
    line = line.slice(..).slice(..);

    let mut string = "";
    for c in rope.slice(4..500).slice(4..400).chunks() {
        string = c;
    }
}
