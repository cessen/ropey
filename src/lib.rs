//! A ut8 text rope for efficient editing of large texts.
//!
//! Ropey provides a unicode-aware implementation of text ropes for Rust.
//! It does double-duty as both the mutate-in-place and copy-on-write
//! variants of text ropes (it just depends on how you use the APIs).
//!
//! Ropey is designed with the following in mind:
//!
//! - **Strong Unicode support.**  Ropey treats `char`'s (unicode code points)
//!   as the base unit of text.  You can index into, slice by, and iterate over
//!   a Ropey-rope by `char` index.
//! - **Line-aware.**  Ropey maintains meta-data about line breaks, allowing
//!   you to index into and iterate over lines of text.
//! - **Grapheme-friendly.**  Ropey ensures that graphemes are never split in its
//!   internal representation of text, and provides APIs for iterating over
//!   graphemes and querying about grapheme boundaries.
//! - **Frequent edits of large texts.**  Ropey is intended to be used for text
//!   editing and manipulation, including when the text is hundreds of megabytes
//!   large and the edits are all over the place.
//! - **Thread safety.** Data is shared between clones of Ropey-ropes, making
//!   clones extremley cheap. This is entirely thread safe, and clones can be
//!   freely sent between threads.  More memory is only taken up incrementally
//!   as edits cause clones to diverge.
//! - **Efficiency.**  All of the above is fast and minimizes memory usage.


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
