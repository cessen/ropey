//! This is the example from the front page of Ropey's documentation.

extern crate ropey;

use std::io::Result;

use ropey::Rope;
use std::fs::File;
use std::io::{BufReader, BufWriter};

fn main() {
    do_stuff().unwrap();
}

/// Wrapper function, so we can use ? operator.
fn do_stuff() -> Result<()> {
    // Load a text file.
    let mut text = Rope::from_reader(BufReader::new(File::open("my_great_book.txt")?))?;

    // Print the 516th line (zero-indexed) to see the terrible
    // writing.
    println!("{}", text.line(515));

    // Get the start/end char indices of the line.
    let start_idx = text.line_to_char(515);
    let end_idx = text.line_to_char(516);

    // Remove the line...
    text.remove(start_idx..end_idx);

    // ...and replace it with something better.
    text.insert(start_idx, "The flowers are... so... dunno.\n");

    // Print the changes, along with the previous few lines for context.
    let start_idx = text.line_to_char(511);
    let end_idx = text.line_to_char(516);
    println!("{}", text.slice(start_idx..end_idx));

    // Write the file back out to disk.
    text.write_to(BufWriter::new(File::create("my_great_book.txt")?))?;

    Ok(())
}
