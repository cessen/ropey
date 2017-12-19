//! Ropey provides a unicode-aware implementation of utf8 text ropes for Rust.
//! It is designed with the following in mind:
//!
//! - **Text editing.**  Ropey is specifically designed with text editing
//!   in mind, including editing of very large texts (e.g. hundreds of
//!   megabytes).
//! - **Strong Unicode support.**  Ropey treats Unicode code points as the
//!   base unit of text.  It also provides facilities for working with grapheme
//!   clusters.
//! - **Line-aware.**  Ropey knows about line breaks, so you can index into
//!   and iterate over lines of text.
//! - **Efficiency.**  Ropey aims to be fast and to minimize memory usage.


extern crate smallvec;
extern crate unicode_segmentation;

mod tree;
mod rope;
mod rope_builder;
mod slice;
mod str_utils;

pub mod iter;

pub use rope::Rope;
pub use rope_builder::RopeBuilder;
pub use slice::RopeSlice;
