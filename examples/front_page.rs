extern crate ropey;

use std::io::Result;

use ropey::Rope;
use std::fs::File;
use std::io::{BufReader, BufWriter};

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
        // Print the 516th line (zero-indexed) to see the terrible
        // writing.
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

    // Write the file back out to disk.
    text.write_to(BufWriter::new(File::create("my_great_book.txt")?))?;

    Ok(())
}
