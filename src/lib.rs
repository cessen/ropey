//! Ropey is a utf8 text rope for Rust.  It is fast, robust, and can handle
//! huge texts and memory-incoherent edits with ease.
//!
//! Ropey's atomic unit of text is Unicode scalar values (or `char`s in Rust)
//! encoded as utf8.  All of Ropey's editing and slicing operations are done
//! in terms of char indices which prevents accidental creation of invalid
//! utf8 data.
//!
//! The library is made up of four main components:
//!
//! - [`Rope`](struct.Rope.html): the main rope type.
//! - [`RopeSlice`](struct.RopeSlice.html): an immutable view into part of a
//!   `Rope`.
//! - [`iter`](iter/index.html): iterators over `Rope`/`RopeSlice` data.
//! - [`RopeBuilder`](struct.RopeBuilder.html): an efficient incremental
//!   `Rope` builder.
//!
//!
//! # A Basic Example
//!
//! Let's say we want to open up a text file, replace the 516th line (the
//! writing was terrible!), and save it back to disk.  It's contrived, but will
//! give a good sampling of the APIs and how they work together.
//!
//! ```no_run
//! # use std::io::Result;
//! use std::fs::File;
//! use std::io::{BufReader, BufWriter};
//! use ropey::Rope;
//!
//! # fn do_stuff() -> Result<()> {
//! // Load a text file.
//! let mut text = Rope::from_reader(
//!     BufReader::new(File::open("my_great_book.txt")?)
//! )?;
//!
//! // Print the 516th line (zero-indexed) to see the terrible
//! // writing.
//! println!("{}", text.line(515));
//!
//! // Get the start/end char indices of the line.
//! let start_idx = text.line_to_char(515);
//! let end_idx = text.line_to_char(516);
//!
//! // Remove the line...
//! text.remove(start_idx..end_idx);
//!
//! // ...and replace it with something better.
//! text.insert(start_idx, "The flowers are... so... dunno.\n");
//!
//! // Print the changes, along with the previous few lines for context.
//! let start_idx = text.line_to_char(511);
//! let end_idx = text.line_to_char(516);
//! println!("{}", text.slice(start_idx..end_idx));
//!
//! // Write the file back out to disk.
//! text.write_to(
//!     BufWriter::new(File::create("my_great_book.txt")?)
//! )?;
//! # Ok(())
//! # }
//! # do_stuff().unwrap();
//! ```
//!
//! More examples can be found in the `examples` directory of the git
//! repository.  Many of those examples demonstrate doing non-trivial things
//! with Ropey such as grapheme handling, search-and-replace, and streaming
//! loading of non-utf8 text files.
//!
//!
//! # Low-level APIs
//!
//! Ropey also provides access to some of its low-level APIs, enabling client
//! code to efficiently work with a `Rope`'s data and implement new
//! functionality.  The most important of those API's are:
//!
//! - The [`chunk_at_*()`](struct.Rope.html#method.chunk_at_byte)
//!   chunk-fetching methods of `Rope` and `RopeSlice`.
//! - The [`Chunks`](iter/struct.Chunks.html) iterator.
//! - The functions in [`str_utils`](str_utils/index.html) for operating on
//!   `&str` slices.
//!
//! Internally, each `Rope` stores text as a segemented collection of utf8
//! strings.  The chunk-fetching methods and `Chunks` iterator provide direct
//! access to those strings (or "chunks") as `&str` slices, allowing client
//! code to work directly with the underlying utf8 data.
//!
//! The chunk-fetching methods and `str_utils` functions are the basic
//! building blocks that Ropey itself uses to build much of its functionality.
//! For example, the [`Rope::byte_to_char()`](struct.Rope.html#method.byte_to_char)
//! method can be reimplemented as a free function like this:
//!
//! ```no_run
//! use ropey::{
//!     Rope,
//!     str_utils::byte_to_char_idx
//! };
//!
//! fn byte_to_char(rope: &Rope, byte_idx: usize) -> usize {
//!     let (chunk, b, c, _) = rope.chunk_at_byte(byte_idx);
//!     c + byte_to_char_idx(chunk, byte_idx - b)
//! }
//! ```
//!
//! And this will be just as efficient as Ropey's implementation.
//!
//! The chunk-fetching methods in particular are among the fastest functions
//! that Ropey provides, generally operating in the sub-hundred nanosecond
//! range for medium-sized (~200kB) documents on recent-ish computer systems.
//!
//!
//! # A Note About Line Endings
//!
//! Some of Ropey's APIs use the concept of line breaks or lines of text.  In
//! all such APIs, Ropey treats the following unicode sequences as line
//! breaks:
//!
//! - `U+000A`          &mdash; LF (Line Feed)
//! - `U+000B`          &mdash; VT (Vertical Tab)
//! - `U+000C`          &mdash; FF (Form Feed)
//! - `U+000D`          &mdash; CR (Carriage Return)
//! - `U+0085`          &mdash; NEL (Next Line)
//! - `U+2028`          &mdash; Line Separator
//! - `U+2029`          &mdash; Paragraph Separator
//! - `U+000D` `U+000A` &mdash; CRLF (Carriage Return + Line Feed)
//!
//! Additionally, Ropey treats line breaks as being a part of the line that
//! they mark the end of.  That is to say, lines begin immediately _after_ a
//! line break.
//!
//! CRLF pairs are always treated as a single line break, and are never split
//! across chunks.  Note, however, that slicing can still split them.

#![allow(clippy::collapsible_if)]
#![allow(clippy::inline_always)]
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::type_complexity)]

extern crate smallvec;

mod crlf;
mod rope;
mod rope_builder;
mod slice;
mod tree;

pub mod iter;
pub mod str_utils;

pub use crate::rope::Rope;
pub use crate::rope_builder::RopeBuilder;
pub use crate::slice::RopeSlice;
