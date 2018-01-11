extern crate ropey;

use std::io::Result;

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use ropey::Rope;

/// This is the example from the front page of Ropey's documentation.
fn main() {
    do_stuff().unwrap();
}

/// Wrapper function, so we can use ? operator.
fn do_stuff() -> Result<()> {
    // Load the file into a Rope.
    let mut text = Rope::from_reader(BufReader::new(File::open("my_great_book.txt")?))?;

    // Make sure there are at least 516 lines.
    if text.len_lines() >= 516 {
        // Let's print the line first, to embarrass ourselves with our
        // terrible writing!  Note that lines are zero-indexed, so the
        // 516th line is at index 515.
        println!("{}", text.line(515));

        // Get the char indices of the start/end of the line.
        let start_idx = text.line_to_char(515);
        let end_idx = text.line_to_char(516);

        // Remove that terrible writing!
        text.remove(start_idx..end_idx);

        // ...and replace it with something better.
        text.insert(start_idx, "The flowers are... so... dunno.\n");

        // Let's print our changes, along with the previous few lines
        // for context.  Gotta make sure the writing works!
        let start_idx = text.line_to_char(511);
        let end_idx = text.line_to_char(516);
        println!("{}", text.slice(start_idx..end_idx));
    }

    // Write the file back out to disk.  We use the `Chunks` iterator
    // here to be maximally efficient.
    let mut file = BufWriter::new(File::create("my_great_book.txt")?);
    for chunk in text.chunks() {
        file.write(chunk.as_bytes())?;
    }

    Ok(())
}
