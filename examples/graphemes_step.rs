//! This example shows how to implement functions that make queries about
//! grapheme boundaries on a `Rope` or `RopeSlice`.  This also serves as a
//! good starting point for similar functions for other kinds of segementation,
//! such as word boundaries.

#![allow(dead_code)]

extern crate ropey;
extern crate unicode_segmentation;

use ropey::{str_utils::byte_to_char_idx, RopeSlice};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

fn main() {}

/// Finds the previous grapheme boundary before the given char position.
fn prev_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> usize {
    // Bounds check
    debug_assert!(char_idx <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

    // Find the previous grapheme cluster boundary.
    loop {
        match gc.prev_boundary(chunk, chunk_byte_idx) {
            Ok(None) => return 0,
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                return chunk_char_idx + tmp;
            }
            Err(GraphemeIncomplete::PrevChunk) => {
                let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_idx - 1);
                chunk = a;
                chunk_byte_idx = b;
                chunk_char_idx = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                gc.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

/// Finds the next grapheme boundary after the given char position.
fn next_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> usize {
    // Bounds check
    debug_assert!(char_idx <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

    // Find the next grapheme cluster boundary.
    loop {
        match gc.next_boundary(chunk, chunk_byte_idx) {
            Ok(None) => return slice.len_chars(),
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                return chunk_char_idx + tmp;
            }
            Err(GraphemeIncomplete::NextChunk) => {
                chunk_byte_idx += chunk.len();
                let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_idx);
                chunk = a;
                chunk_char_idx = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                gc.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

/// Returns whether the given char position is a grapheme boundary.
fn is_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> bool {
    // Bounds check
    debug_assert!(char_idx <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);

    // Get the chunk with our byte index in it.
    let (chunk, chunk_byte_idx, _, _) = slice.chunk_at_byte(byte_idx);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

    // Determine if the given position is a grapheme cluster boundary.
    loop {
        match gc.is_boundary(chunk, chunk_byte_idx) {
            Ok(n) => return n,
            Err(GraphemeIncomplete::PreContext(n)) => {
                let (ctx_chunk, ctx_byte_start, _, _) = slice.chunk_at_byte(n - 1);
                gc.provide_context(ctx_chunk, ctx_byte_start);
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
#[rustfmt::skip] // Because of the crazy long graphemes
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn prev_grapheme() {
        let r = Rope::from_str("Hẽ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃llõ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃ wõ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃rld!");
        let mut idx = r.len_chars();

        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2705);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2704);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2703);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2702);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 984);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 983);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 982);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 446);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 445);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 444);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 1);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 0);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 0);
    }

    #[test]
    fn next_grapheme() {
        let r = Rope::from_str("Hẽ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃llõ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃ wõ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃rld!");
        let mut idx = 0;

        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 1);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 444);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 445);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 446);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 982);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 983);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 984);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2702);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2703);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2704);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2705);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2706);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2706);
    }

    #[test]
    fn is_grapheme_boundary_01() {
        let r = Rope::from_str("Hẽ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃llõ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃ wõ̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃̃rld!");

        assert_eq!(is_grapheme_boundary(&r.slice(..), 0), true);

        assert_eq!(is_grapheme_boundary(&r.slice(..), 1), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 2), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 200), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 443), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 444), true);

        assert_eq!(is_grapheme_boundary(&r.slice(..), 445), true);

        assert_eq!(is_grapheme_boundary(&r.slice(..), 446), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 447), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 600), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 981), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 982), true);

        assert_eq!(is_grapheme_boundary(&r.slice(..), 983), true);

        assert_eq!(is_grapheme_boundary(&r.slice(..), 984), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 985), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 1400), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 2701), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 2702), true);

        assert_eq!(is_grapheme_boundary(&r.slice(..), 2703), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 2704), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 2705), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 2706), true);
    }

    #[test]
    fn prev_grapheme_regional_symbols() {
        let r = Rope::from_str("🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪🇸🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪🇸🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪");
        let mut idx = r.len_chars();

        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 46);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 44);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 42);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 40);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 38);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 36);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 34);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 32);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 30);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 28);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 26);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 24);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 22);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 20);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 18);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 16);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 14);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 12);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 10);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 8);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 6);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 4);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 0);
        idx = prev_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 0);
    }

    #[test]
    fn next_grapheme_regional_symbols() {
        let r = Rope::from_str("🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪🇸🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪🇸🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪");
        let mut idx = 0;

        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 2);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 4);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 6);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 8);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 10);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 12);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 14);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 16);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 18);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 20);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 22);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 24);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 26);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 28);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 30);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 32);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 34);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 36);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 38);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 40);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 42);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 44);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 46);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 47);
        idx = next_grapheme_boundary(&r.slice(..), idx);
        assert_eq!(idx, 47);
    }

    #[test]
    fn is_grapheme_boundary_regional_symbols() {
        let r = Rope::from_str("🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪🇸🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪🇸🇬🇧🇯🇵🇺🇸🇫🇷🇷🇺🇨🇳🇩🇪🇪");

        assert_eq!(is_grapheme_boundary(&r.slice(..), 0), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 1), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 2), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 3), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 4), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 5), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 6), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 7), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 8), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 9), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 10), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 11), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 12), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 13), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 14), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 15), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 16), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 17), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 18), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 19), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 20), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 21), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 22), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 23), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 24), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 25), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 26), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 27), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 28), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 29), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 30), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 31), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 32), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 33), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 34), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 35), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 36), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 37), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 38), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 39), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 40), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 41), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 42), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 43), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 44), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 45), false);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 46), true);
        assert_eq!(is_grapheme_boundary(&r.slice(..), 47), true);
    }
}
