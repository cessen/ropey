//! Ropey is a utf8 text rope for Rust.  It is efficient, robust, and can handle
//! large texts (measured in gigabytes) with ease.
//!
//! Ropey stores text as utf8 and uses byte indices into that utf8 data to
//! specify positions in the text.  Like Rust's built-in `String` and `&str`
//! types, creation of invalid utf8 data through those indices is prevented at
//! runtime.
//!
//! The library is made up of four main components:
//!
//! - [`Rope`]: the main rope type.
//! - [`RopeSlice`]: an immutable view into part of a `Rope`.
//! - [`iter`]: iterators over `Rope`/`RopeSlice` data.
//! - [`RopeBuilder`]: an efficient incremental `Rope` builder.
//!
//!
#![cfg_attr(
    feature = "metric_lines_lf_cr",
    doc = r##"
# A Basic Example

Let's say we want to open up a text file, replace the 516th line (the
writing was terrible!), and save it back to disk.  It's contrived, but will
give a good sampling of the APIs and how they work together.

```no_run
# use std::io::Result;
# fn do_stuff() -> Result<()> {
use std::fs::File;
use std::io::{BufReader, BufWriter};
use ropey::{Rope, LineType::LF_CR};

// Load a text file.
let mut text = Rope::from_reader(
    BufReader::new(File::open("my_great_book.txt")?)
)?;

// Print the 516th line (zero-indexed) to see the terrible
// writing.
println!("{}", text.line(515, LF_CR));

// Get the start/end byte indices of the line.
let start_idx = text.line_to_byte_idx(515, LF_CR);
let end_idx = text.line_to_byte_idx(516, LF_CR);

// Remove the line...
text.remove(start_idx..end_idx);

// ...and replace it with something better.
text.insert(start_idx, "The flowers are... so... dunno.\n");

// Print the changes, along with the previous few lines for context.
let start_idx = text.line_to_byte_idx(511, LF_CR);
let end_idx = text.line_to_byte_idx(516, LF_CR);
println!("{}", text.slice(start_idx..end_idx));

// Write the file back out to disk.
text.write_to(
    BufWriter::new(File::create("my_great_book.txt")?)
)?;
# Ok(())
# }
# do_stuff().unwrap();
```

More examples can be found in the `examples` directory of the git
repository.  Many of those examples demonstrate doing non-trivial things
with Ropey such as grapheme handling, search-and-replace, and streaming
loading of non-utf8 text files.
"##
)]
//!
//!
//! # Low-level APIs
//!
//! Ropey also provides access to some of its low-level APIs, enabling client
//! code to efficiently work with a `Rope`'s data and implement new
//! functionality.  The most important of those API's are:
//!
//! - The [`chunk()`](Rope::chunk) chunk-fetching method of `Rope` and
//!   `RopeSlice`.
//! - The [`Chunks`](iter::Chunks) iterator.
//! - The [`ChunkCursor`](ChunkCursor) type.
//!
//! Internally, each `Rope` stores text as a segmented collection of utf8
//! strings.  The chunk APIs provide direct access to those strings as `&str`
//! slices, allowing client code to work directly with the underlying utf8 data.
//!
//!
//! # A Note About Line Breaks
//!
//! Some of Ropey's APIs use the concept of line breaks or lines of text.
//!
//! Ropey considers the start of the rope and positions immediately _after_ line
//! breaks to be the start of new lines.  It also treats line breaks as being a
//! part of the line they mark the end of.
//!
//! Examples:
//!
//! - `"Hello"` has 1 line: `"Hello"`.
//! - `"Hello\nworld"` has 2 lines: `"Hello\n"` and `"world"`.
//! - `"Hello\nworld\n"` has 3 lines: `"Hello\n"`, `"world\n"`, and `""`.
//!
//! Importantly, **this departs from Rust's standard library**, which follows
//! the Unix convention of treating `\n` as a line *ending* rather than a line
//! *break*.  The reason for Ropey's departure is not to favor one convention
//! over the other, but rather is to avoid being opinionated: the Unix
//! convention is not universal, and it is easier to implement the Unix
//! convention on top of Ropey's behavior than the other way around.  Client
//! code is encouraged to choose and implement whichever convention they prefer
//! on top of Ropey.
//!
//! Another thing you will run into with Ropey's line-based APIs is that they
//! all take a [`LineType`] parameter.  This is because Ropey can track lines
//! according to multiple conventions simultaneously (e.g. whether or not
//! Carriage Return qualifies as a line break).  This additional parameter
//! specifies which of those conventions to use for a call.  See  [`LineType`]'s
//! documentation for more details.
//!
//!
//! # Crate Features
//!
//! In addition to byte indices (Ropey's primary indexing metric), Ropey also
//! has crate features to enable additional secondary indexing metrics:
//!
//! - `metric_chars`: indexing by `char`.
//! - `metric_utf16`: indexing by UTF16 code unit.
//! - `metric_lines_lf`: indexing by line with [`LineType::LF`].
//! - `metric_lines_lf_cr`: indexing by line with [`LineType::LF_CR`].
//! - `metric_lines_unicode`: indexing by line with [`LineType::All`].
//!
//! Of these crate features, only `metric_lines_lf_cr` is enabled by default.
//!
//! The main APIs enabled by these features are conversions to/from byte
//! indices. However, the line-based metrics additionally enable the `Lines`
//! iterator and a convenience method for fetching individual lines as
//! `RopeSlice`s.
//!
//! ## A Note About SIMD Acceleration
//!
//! Ropey has a `simd` feature flag (enabled by default) that enables explicit
//! SIMD on supported platforms to improve performance.
//!
//! There is a bit of a footgun here: if you disable default features (e.g. to
//! configure secondary indexing metrics) then SIMD will also get disabled, and
//! performance will suffer.  So be careful to explicitly re-enable the `simd`
//! feature flag (if desired) when doing that.
//!
//! ## A Warning About Internal-Only Crate Features
//!
//! Please avoid using a blanket `all-features` with Ropey, because there are
//! some internal-only crate features that you probably don't want.  These
//! features will not break any APIs, but they may substantially slow down Ropey
//! and/or make it significantly more memory hungry.  The purpose of those
//! features is for internal testing and debugging during Ropey development.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::inline_always)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::missing_transmute_annotations)]
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::type_complexity)]
#![warn(missing_docs)]

use std::ops::Bound;

mod shared_impl;

mod chunk_cursor;
mod rope;
mod rope_builder;
mod slice;
mod str_utils;
mod tree;

pub mod extra;
pub mod iter;

pub use chunk_cursor::ChunkCursor;
pub use rope::Rope;
pub use rope_builder::RopeBuilder;
pub use slice::RopeSlice;

/// Specifies a set of line breaks to be recognized in Ropey's line-based APIs.
///
/// Ropey can track more than one line break convention simultaneously, and
/// `LineType` is used to specify which convention to use for a given function
/// call.  For example:
///
/// ```
/// # use ropey::Rope;
/// # #[cfg(any(feature = "metric_lines_lf", feature = "metric_lines_lf_cr"))]
/// # use ropey::LineType;
/// // Text with both an LF and CR line break.
/// let text = Rope::from_str("Line 1\nLine 2\rLine 3");
///
/// # #[cfg(feature = "metric_lines_lf")]
/// # {
/// // A call with only LF as a line break, so it sees just two lines.
/// assert_eq!("Line 1\n", text.line(0, LineType::LF));
/// assert_eq!("Line 2\rLine 3", text.line(1, LineType::LF));
/// # }
///
/// # #[cfg(feature = "metric_lines_lf_cr")]
/// # {
/// // A call with LF and CR as line breaks, so it sees three lines.
/// assert_eq!("Line 1\n", text.line(0, LineType::LF_CR));
/// assert_eq!("Line 2\r", text.line(1, LineType::LF_CR));
/// assert_eq!("Line 3", text.line(2, LineType::LF_CR));
/// # }
///```
///
/// This is admittedly a little awkward if you only ever use one line break
/// convention.  However, this approach provides a lot of flexibility that would
/// otherwise be impossible.  A few examples:
///
/// - If you want your application to recognize all Unicode-specified line
///   breaks, but you also need it to communicate with Language Server Protocol
///   which only recognizes a small subset of that, you can track both
///   conventions and use the latter only for LSP communication.
/// - If you want your application to recognize LF and CR, but you also want to
///   jump to compilation errors from a compiler that only recognizes LF, you
///   can track both conventions and use Ropey's metric conversion functions to
///   translate line numbers.
/// - If desired, you can let your users switch between line break conventions
///   for different documents.
#[cfg_attr(
    docsrs,
    doc(cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    )))
)]
#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum LineType {
    /// - LF (Line Feed)
    /// - CRLF (Carriage Return + Line Feed) &mdash; implicitly recognized, due to
    ///   CR being ignored.
    #[cfg_attr(docsrs, doc(cfg(feature = "metric_lines_lf")))]
    #[cfg(feature = "metric_lines_lf")]
    LF,

    /// - LF (Line Feed)
    /// - CR (Carriage Return)
    /// - CRLF (Carriage Return + Line Feed)
    #[cfg_attr(docsrs, doc(cfg(feature = "metric_lines_lf_cr")))]
    #[cfg(feature = "metric_lines_lf_cr")]
    LF_CR,

    /// All Unicode line breaks (as specified in [Unicode Annex
    /// #14](https://www.unicode.org/reports/tr14/#BK)):
    ///
    /// - LF (Line Feed)
    /// - CR (Carriage Return)
    /// - CRLF (Carriage Return + Line Feed)
    /// - VT (Vertical Tab)
    /// - FF (Form Feed)
    /// - NEL (Next Line)
    /// - Line Separator
    /// - Paragraph Separator
    #[cfg_attr(docsrs, doc(cfg(feature = "metric_lines_unicode")))]
    #[cfg(feature = "metric_lines_unicode")]
    All,
}

/// Ropey's result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Ropey's error type.
///
/// Indicates the cause of a failed operation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The byte index(s) given were not on a char boundary, but needed to be
    /// for the given operation.
    NonCharBoundary,

    /// The index(s) given exceeded the size of the text.
    OutOfBounds,

    /// The range given was intrinsically invalid (e.g. inverted).
    InvalidRange,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Error::NonCharBoundary => write!(f, "byte index is not on a char boundary"),
            Error::OutOfBounds => write!(f, "index is out of bounds"),
            Error::InvalidRange => write!(f, "index range is invalid: end < start"),
        }
    }
}

//=============================================================
// Utilities.

#[inline(always)]
pub(crate) fn is_char_boundary(byte_idx: usize, text: &[u8]) -> bool {
    if byte_idx == text.len() {
        return true;
    }

    // Trick from rust stdlib.  Equivalent to:
    // `text[byte_idx] < 128 || text[byte_idx] >= 192`
    (text[byte_idx] as i8) >= -0x40
}

/// Returns the closest byte index <= `byte_idx` that is a char boundary.
///
/// Precondition: `text` must be a well-formed utf8 string.
///
/// Note for convenience, if `byte_idx > text.len()`, this simply returns
/// `text.len()`.
pub(crate) fn floor_char_boundary(mut byte_idx: usize, text: &[u8]) -> usize {
    if byte_idx >= text.len() {
        return text.len();
    }

    // The redundant `< text.len()` bounds check is for code gen.  For some
    // reason the compiler (at time of writing) can't infer that `>= text.len()`
    // is impossible from the if clause above without this, and that in turn
    // results in needlessly bloated code gen.
    while byte_idx > 0 && byte_idx < text.len() && !is_char_boundary(byte_idx, text) {
        byte_idx -= 1;
    }

    byte_idx
}

/// Returns the closest byte index >= `byte_idx` that is a char boundary.
///
/// Precondition: `text` must be a well-formed utf8 string.
///
/// Note for convenience, if `byte_idx > text.len()`, this simply returns
/// `text.len()`.
pub(crate) fn ceil_char_boundary(mut byte_idx: usize, text: &[u8]) -> usize {
    if byte_idx >= text.len() {
        return text.len();
    }

    while byte_idx < text.len() && !is_char_boundary(byte_idx, text) {
        byte_idx += 1;
    }

    byte_idx
}

/// Returns the closest byte index <= `byte_idx` that is a char boundary and
/// doesn't split a CRLF pair.
///
/// Mainly used for finding an appropriate place to split text.
///
/// Note for convenience, if `byte_idx > text.len()`, this simply returns
/// `text.len()`.
pub(crate) fn find_appropriate_split_floor(mut byte_idx: usize, text: &str) -> usize {
    byte_idx = floor_char_boundary(byte_idx, text.as_bytes());

    if byte_idx > 0
        && byte_idx < text.len()
        && str_utils::byte_is_lf(text, byte_idx)
        && str_utils::byte_is_cr(text, byte_idx - 1)
    {
        byte_idx -= 1;
    }

    byte_idx
}

/// Returns the closest byte index >= `byte_idx` that is a char boundary and
/// doesn't split a CRLF pair.
///
/// Mainly used for finding an appropriate place to split text.
///
/// Note for convenience, if `byte_idx > text.len()`, this simply returns
/// `text.len()`.
pub(crate) fn find_appropriate_split_ceil(mut byte_idx: usize, text: &str) -> usize {
    byte_idx = ceil_char_boundary(byte_idx, text.as_bytes());

    if byte_idx > 0
        && byte_idx < text.len()
        && str_utils::byte_is_lf(text, byte_idx)
        && str_utils::byte_is_cr(text, byte_idx - 1)
    {
        byte_idx += 1;
    }

    byte_idx
}

//-------------------------------------------------------------
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
