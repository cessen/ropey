//! Ropey is a utf8 text rope for Rust.  It is fast, robust, and can handle
//! huge texts and memory-incoherent edits with ease.
//!
//! Ropey's atomic unit of text is Unicode scalar values (or `char`s in Rust)
//! encoded as utf8.  All of Ropey's editing and slicing operations are done
//! in terms of char indices, which prevents accidental creation of invalid
//! utf8 data.
//!
//! The library is made up of four main components:
//!
//! - [`Rope`]: the main rope type.
//! - [`RopeSlice`]: an immutable view into part of a
//!   `Rope`.
//! - [`iter`]: iterators over `Rope`/`RopeSlice` data.
//! - [`RopeBuilder`]: an efficient incremental
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
//! - The [`chunk_at_*()`](Rope::chunk_at_byte)
//!   chunk-fetching methods of `Rope` and `RopeSlice`.
//! - The [`Chunks`](iter::Chunks) iterator.
//! - The functions in [`str_utils`] for operating on
//!   `&str` slices.
//!
//! Internally, each `Rope` stores text as a segemented collection of utf8
//! strings.  The chunk-fetching methods and `Chunks` iterator provide direct
//! access to those strings (or "chunks") as `&str` slices, allowing client
//! code to work directly with the underlying utf8 data.
//!
//! The chunk-fetching methods and `str_utils` functions are the basic
//! building blocks that Ropey itself uses to build much of its functionality.
//! For example, the [`Rope::byte_to_char()`]
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
//! # A Note About Line Breaks
//!
//! Some of Ropey's APIs use the concept of line breaks or lines of text.
//!
//! Ropey considers the start of the rope and positions immediately
//! _after_ line breaks to be the start of new lines.  And it treats
//! line breaks as being a part of the lines they mark the end of.
//!
//! For example, the rope `"Hello"` has a single line: `"Hello"`.  The
//! rope `"Hello\nworld"` has two lines: `"Hello\n"` and `"world"`.  And
//! the rope `"Hello\nworld\n"` has three lines: `"Hello\n"`,
//! `"world\n"`, and `""`.
//!
//! Ropey can be configured at build time via feature flags to recognize
//! different line breaks.  Ropey always recognizes:
//!
//! - `U+000A`          &mdash; LF (Line Feed)
//! - `U+000D` `U+000A` &mdash; CRLF (Carriage Return + Line Feed)
//!
//! With the `cr_lines` feature, the following are also recognized:
//!
//! - `U+000D`          &mdash; CR (Carriage Return)
//!
//! With the `unicode_lines` feature, in addition to all of the
//! above, the following are also recognized (bringing Ropey into
//! conformance with
//! [Unicode Annex #14](https://www.unicode.org/reports/tr14/#BK)):
//!
//! - `U+000B`          &mdash; VT (Vertical Tab)
//! - `U+000C`          &mdash; FF (Form Feed)
//! - `U+0085`          &mdash; NEL (Next Line)
//! - `U+2028`          &mdash; Line Separator
//! - `U+2029`          &mdash; Paragraph Separator
//!
//! (Note: `unicode_lines` is enabled by default, and always implies
//! `cr_lines`.)
//!
//! CRLF pairs are always treated as a single line break, and are never split
//! across chunks.  Note, however, that slicing can still split them.
//!
//!
//! # A Note About SIMD Acceleration
//!
//! Ropey has a `simd` feature flag (enabled by default) that enables
//! explicit SIMD on supported platforms to improve performance.
//!
//! There is a bit of a footgun here: if you disable default features to
//! configure line break behavior (as per the section above) then SIMD
//! will also get disabled, and performance will suffer.  So be careful
//! to explicitly re-enable the `simd` feature flag (if desired) when
//! doing that.

#![allow(clippy::collapsible_if)]
#![allow(clippy::inline_always)]
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::type_complexity)]

extern crate smallvec;
extern crate str_indices;

mod crlf;
mod rope;
mod rope_builder;
mod slice;
mod tree;

pub mod iter;
pub mod str_utils;

use std::ops::Bound;

pub use crate::rope::Rope;
pub use crate::rope_builder::RopeBuilder;
pub use crate::slice::RopeSlice;

/// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
/// These are only exposed for tests that live in the `tests` directory.
#[doc(hidden)]
pub use crate::tree::{MAX_BYTES, MAX_CHILDREN, MIN_BYTES, MIN_CHILDREN};

//==============================================================
// Error reporting types.

/// Ropey's result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Ropey's error type.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum Error {
    /// Indicates that the passed byte index was out of bounds.
    ///
    /// Contains the index attempted and the actual length of the
    /// `Rope`/`RopeSlice` in bytes, in that order.
    ByteIndexOutOfBounds(usize, usize),

    /// Indicates that the passed char index was out of bounds.
    ///
    /// Contains the index attempted and the actual length of the
    /// `Rope`/`RopeSlice` in chars, in that order.
    CharIndexOutOfBounds(usize, usize),

    /// Indicates that the passed line index was out of bounds.
    ///
    /// Contains the index attempted and the actual length of the
    /// `Rope`/`RopeSlice` in lines, in that order.
    LineIndexOutOfBounds(usize, usize),

    /// Indicates that the passed utf16 code-unit index was out of
    /// bounds.
    ///
    /// Contains the index attempted and the actual length of the
    /// `Rope`/`RopeSlice` in utf16 code units, in that order.
    Utf16IndexOutOfBounds(usize, usize),

    /// Indicates that the passed byte index was not a char boundary.
    ///
    /// Contains the passed byte index.
    ByteIndexNotCharBoundary(usize),

    /// Indicates that the passed byte range didn't line up with char
    /// boundaries.
    ///
    /// Contains the [start, end) byte indices of the range, in that order.
    /// When either the start or end are `None`, that indicates a half-open
    /// range.
    ByteRangeNotCharBoundary(
        Option<usize>, // Start.
        Option<usize>, // End.
    ),

    /// Indicates that a reversed byte-index range (end < start) was
    /// encountered.
    ///
    /// Contains the [start, end) byte indices of the range, in that order.
    ByteRangeInvalid(
        usize, // Start.
        usize, // End.
    ),

    /// Indicates that a reversed char-index range (end < start) was
    /// encountered.
    ///
    /// Contains the [start, end) char indices of the range, in that order.
    CharRangeInvalid(
        usize, // Start.
        usize, // End.
    ),

    /// Indicates that the passed byte-index range was partially or fully
    /// out of bounds.
    ///
    /// Contains the [start, end) byte indices of the range and the actual
    /// length of the `Rope`/`RopeSlice` in bytes, in that order.  When
    /// either the start or end are `None`, that indicates a half-open range.
    ByteRangeOutOfBounds(
        Option<usize>, // Start.
        Option<usize>, // End.
        usize,         // Rope byte length.
    ),

    /// Indicates that the passed char-index range was partially or fully
    /// out of bounds.
    ///
    /// Contains the [start, end) char indices of the range and the actual
    /// length of the `Rope`/`RopeSlice` in chars, in that order.  When
    /// either the start or end are `None`, that indicates a half-open range.
    CharRangeOutOfBounds(
        Option<usize>, // Start.
        Option<usize>, // End.
        usize,         // Rope char length.
    ),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    // Deprecated in std.
    fn description(&self) -> &str {
        ""
    }

    // Deprecated in std.
    fn cause(&self) -> Option<&dyn std::error::Error> {
        None
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Error::ByteIndexOutOfBounds(index, len) => {
                write!(
                    f,
                    "Byte index out of bounds: byte index {}, Rope/RopeSlice byte length {}",
                    index, len
                )
            }
            Error::CharIndexOutOfBounds(index, len) => {
                write!(
                    f,
                    "Char index out of bounds: char index {}, Rope/RopeSlice char length {}",
                    index, len
                )
            }
            Error::LineIndexOutOfBounds(index, len) => {
                write!(
                    f,
                    "Line index out of bounds: line index {}, Rope/RopeSlice line count {}",
                    index, len
                )
            }
            Error::Utf16IndexOutOfBounds(index, len) => {
                write!(f, "Utf16 code-unit index out of bounds: utf16 index {}, Rope/RopeSlice utf16 length {}", index, len)
            }
            Error::ByteIndexNotCharBoundary(index) => {
                write!(
                    f,
                    "Byte index is not a valid char boundary: byte index {}",
                    index
                )
            }
            Error::ByteRangeNotCharBoundary(start_idx_opt, end_idx_opt) => {
                write!(f, "Byte range does not align with char boundaries: range ")?;
                write_range(f, start_idx_opt, end_idx_opt)
            }
            Error::ByteRangeInvalid(start_idx, end_idx) => {
                write!(
                    f,
                    "Invalid byte range {}..{}: start must be <= end",
                    start_idx, end_idx
                )
            }
            Error::CharRangeInvalid(start_idx, end_idx) => {
                write!(
                    f,
                    "Invalid char range {}..{}: start must be <= end",
                    start_idx, end_idx
                )
            }
            Error::ByteRangeOutOfBounds(start_idx_opt, end_idx_opt, len) => {
                write!(f, "Byte range out of bounds: byte range ")?;
                write_range(f, start_idx_opt, end_idx_opt)?;
                write!(f, ", Rope/RopeSlice byte length {}", len)
            }
            Error::CharRangeOutOfBounds(start_idx_opt, end_idx_opt, len) => {
                write!(f, "Char range out of bounds: char range ")?;
                write_range(f, start_idx_opt, end_idx_opt)?;
                write!(f, ", Rope/RopeSlice char length {}", len)
            }
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Just re-use the debug impl.
        std::fmt::Debug::fmt(self, f)
    }
}

fn write_range(
    f: &mut std::fmt::Formatter<'_>,
    start_idx: Option<usize>,
    end_idx: Option<usize>,
) -> std::fmt::Result {
    match (start_idx, end_idx) {
        (None, None) => {
            write!(f, "..")
        }

        (Some(start), None) => {
            write!(f, "{}..", start)
        }

        (None, Some(end)) => {
            write!(f, "..{}", end)
        }

        (Some(start), Some(end)) => {
            write!(f, "{}..{}", start, end)
        }
    }
}

//==============================================================
// Range handling utilities.

#[inline(always)]
pub(crate) fn start_bound_to_num(b: Bound<&usize>) -> Option<usize> {
    match b {
        Bound::Included(n) => Some(*n),
        Bound::Excluded(n) => Some(*n + 1),
        Bound::Unbounded => None,
    }
}

#[inline(always)]
pub(crate) fn end_bound_to_num(b: Bound<&usize>) -> Option<usize> {
    match b {
        Bound::Included(n) => Some(*n + 1),
        Bound::Excluded(n) => Some(*n),
        Bound::Unbounded => None,
    }
}
