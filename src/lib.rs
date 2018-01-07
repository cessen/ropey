//! Ropey is a utf8 text rope library, designed to be the backing text
//! buffer for applications such as text editors.  Ropey is fast,
//! Unicode-safe, has low memory overhead, and can handle huge texts
//! and memory-incoherent edits without trouble.
//!
//! The library is made up of four main components:
//!
//! - [`Rope`](struct.Rope.html): the main editable text buffer type.
//! - [`RopeSlice`](struct.RopeSlice.html): an immutable view into part
//!   of a `Rope`.
//! - [`RopeBuilder`](struct.RopeBuilder.html): an efficient incremental `Rope` builder.
//! - [`iter`](iter/index.html): iterators over a `Rope`'s/`RopeSlice`'s
//!   data.
//!
//! # A Contrived Example
//!
//! Let's say we want to open up a file, replace the 516th line (the writing
//! was terrible!), and save it back to disk.  It's contrived, but will give
//! a good sampling of the APIs and how they work together.
//!
//! ```no_run
//! # use std::io::Result;
//! use std::fs::File;
//! use std::io::{Write, BufReader, BufWriter};
//! use ropey::Rope;
//!
//! # fn do_stuff() -> Result<()> {
//! // Load the file into a Rope.
//! let mut text = Rope::from_reader(
//!     BufReader::new(File::open("my_great_book.txt")?)
//! )?;
//!
//! // Make sure there are at least 516 lines.
//! if text.len_lines() >= 516 {
//!     // Let's print the line first, to embarrass ourselves with our
//!     // terrible writing!  Note that lines are zero-indexed, so the
//!     // 516th line is at index 515.
//!     println!("{}", text.line(515));
//!
//!     // Get the char indices of the start/end of the line.
//!     let start_idx = text.line_to_char(515);
//!     let end_idx = text.line_to_char(516);
//!
//!     // Remove that terrible writing!
//!     text.remove(start_idx, end_idx);
//!
//!     // ...and replace it with something better.
//!     text.insert(start_idx, "The flowers are... so... dunno.\n");
//!
//!     // Let's print our changes, along with the previous few lines
//!     // for context.  Gotta make sure the writing works!
//!     let start_idx = text.line_to_char(511);
//!     let end_idx = text.line_to_char(516);
//!     println!("{}", text.slice(start_idx, end_idx));
//! }
//!
//! // Write the file back out to disk.  We use the `Chunks` iterator
//! // here to be maximally efficient.
//! let mut file = BufWriter::new(File::create("my_great_book.txt")?);
//! for chunk in text.chunks() {
//!     file.write(chunk.as_bytes())?;
//! }
//! # Ok(())
//! # }
//! # do_stuff().unwrap();
//! ```

#![cfg_attr(feature = "cargo-clippy", allow(inline_always))]
#![cfg_attr(feature = "cargo-clippy", allow(needless_return))]

extern crate smallvec;
extern crate unicode_segmentation;

mod tree;
mod rope;
mod rope_builder;
mod slice;
mod str_utils;

pub mod iter;
pub mod segmenter;

pub use rope::Rope;
pub use rope_builder::RopeBuilder;
pub use slice::RopeSlice;
