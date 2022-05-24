//! This test file ensures that all of the lifetimes work the way we
//! want, and that there are no regressions. It's a "does this compile?"
//! test.

extern crate ropey;

use ropey::{Rope, RopeSlice};

const TEXT: &str = include_str!("test_text.txt");

fn main() {
    if cfg!(miri) {
        return;
    }

    let rope = Rope::from_str(TEXT);

    let (a, b, c, d, e, f, g, count, line, string) = {
        // The lifetimes of intermediate slices shouldn't matter.  The
        // lifetimes of the things produced by the calls below should be
        // tied to the lifetime of the original rope, not the lifetimes of
        // the slices they were created from.  Therefore, this should all
        // compile.

        let a = rope.slice(4..500).slice(4..400).slice(4..300);
        let b = rope.slice(4..500).slice(4..400).as_str();
        let c = rope.slice(4..500).slice(4..400).line(1);
        let d = rope.line(1).slice(4..20).slice(4..10);
        let e = rope.slice(4..500).slice(4..400).chunk_at_byte(50);
        let f = rope.slice(4..500).slice(4..400).chunk_at_char(50);
        let g = rope.slice(4..500).slice(4..400).chunk_at_line_break(3);

        // Same for iterators.  In addition, the items _yielded_ by the
        // iterators should also be tied to the lifetime of the original
        // rope, not to the iterators or slices they came from.

        let mut count = 0;
        for _ in rope.slice(4..500).slice(4..400).bytes() {
            count += 1;
        }
        for _ in rope.slice(4..500).slice(4..400).chars() {
            count += 1;
        }

        let mut line: RopeSlice = "".into();
        for l in rope.slice(4..500).slice(4..400).lines() {
            line = l;
        }
        line = line.slice(..).slice(..);

        let mut string = "";
        for c in rope.slice(4..500).slice(4..400).chunks() {
            string = c;
        }

        (a, b, c, d, e, f, g, count, line, string)
    };

    println!(
        "{} {:?} {} {} {:?} {:?} {:?} {} {} {}",
        a, b, c, d, e, f, g, count, line, string
    );
}
