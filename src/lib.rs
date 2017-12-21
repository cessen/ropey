//! Ropey is a utf8 text rope library, designed to be the backing text
//! buffer for applications such as text editors.  Ropey is fast,
//! Unicode-safe, has low memory overhead, and can handle huge texts
//! and memory-incoherent edits without breaking a sweat.
//!
//! The library is made up of four main components:
//!
//! - [`Rope`](struct.Rope.html): the main editable text buffer type.
//! - [`RopeSlice`](struct.RopeSlice.html): an immutable view into part
//!   of a `Rope`.
//! - [`RopeBuilder`](struct.RopeBuilder.html): a type for efficiently
//!   creating `Rope`s from streaming data.
//! - [`iter`](iter/index.html): iterators over a `Rope`/`RopeSlice`'s
//!   data.
//!
//! # Basic examples
//!
//! ## Insertion and deletion
//! ```
//! use ropey::Rope;
//! 
//! let mut rope = Rope::from_str("Hello individual!");
//! rope.remove(6, 16);
//! rope.insert(6, "world");
//! 
//! assert_eq!(rope, "Hello world!");
//! ```
//!
//! ## Slicing
//! ```
//! # use ropey::Rope;
//! #
//! let mut rope = Rope::from_str("Hello individual!");
//! let slice = rope.slice(6, 16);
//!
//! assert_eq!(slice, "individual");
//! ```
//!
//! ## Iterating over lines
//! ```
//! # use ropey::Rope;
//! #
//! let mut rope = Rope::from_str("Hello individual!\nHow are you?");
//! let mut itr = rope.lines();
//!
//! assert_eq!(itr.next().unwrap(), "Hello individual!\n");
//! assert_eq!(itr.next().unwrap(), "How are you?");
//! assert_eq!(itr.next(), None);
//! ```

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
