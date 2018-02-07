use std;

use std::borrow::Borrow;
use std::ops::Deref;
use std::ptr;
use std::str;

use crlf;
use smallvec::{Array, SmallVec};
use str_utils::{char_idx_to_byte_idx, count_chars};
use tree::MAX_BYTES;

/// A custom small string, with an internal buffer of `tree::MAX_BYTES`
/// length.  Has a bunch of methods on it that are useful for the rope
/// tree.
#[derive(Clone, Default)]
pub(crate) struct NodeText {
    buffer: SmallVec<BackingArray>,
}

impl NodeText {
    /// Creates a new empty `NodeText`
    #[inline(always)]
    pub fn new() -> Self {
        NodeText {
            buffer: SmallVec::new(),
        }
    }

    /// Creates a new empty `NodeText` with at least `capacity` bytes
    /// of capacity.
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        NodeText {
            buffer: SmallVec::with_capacity(capacity),
        }
    }

    /// Creates a new `NodeText` with the same contents as the given `&str`.
    pub fn from_str(string: &str) -> Self {
        let mut nodetext = NodeText::with_capacity(string.len());
        unsafe { nodetext.insert_bytes(0, string.as_bytes()) };
        nodetext
    }

    /// Inserts a `&str` at byte offset `idx`.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    pub fn insert_str(&mut self, byte_idx: usize, string: &str) {
        debug_assert!(self.is_char_boundary(byte_idx));
        debug_assert!(byte_idx <= self.len());

        unsafe {
            self.insert_bytes(byte_idx, string.as_bytes());
        }
    }

    /// Inserts the given text into the given string at the given char index.
    pub fn insert_str_at_char(&mut self, text: &str, char_idx: usize) {
        let byte_idx = char_idx_to_byte_idx(self, char_idx);
        self.insert_str(byte_idx, text);
    }

    /// Inserts a `&str` and splits the resulting string in half, returning
    /// the right half.
    ///
    /// Only splits on code point boundaries and will never split CRLF pairs,
    /// grapheme boundaries, so if the whole string is a single code point or
    /// CRLF pair, the split will fail and the returned string will be empty.
    pub fn insert_str_split(&mut self, idx: usize, string: &str) -> Self {
        debug_assert!(self.is_char_boundary(idx));
        debug_assert!((self.len() + string.len()) <= (MAX_BYTES * 2 - 4));

        let tot_len = self.len() + string.len();
        let mid_idx = tot_len / 2;
        let a = idx;
        let b = idx + string.len();

        // Figure out the split index, accounting for code point
        // boundaries and CRLF pairs.
        // We first copy the bytes in the area of the proposed split point into
        // a small 8-byte buffer.  We then use that buffer to look for the
        // real split point.
        let split_idx = {
            let mut buf = [0u8; 8];
            let start = mid_idx - 4.min(mid_idx);
            let end = (mid_idx + 4).min(tot_len);
            for i in start..end {
                buf[i - start] = if i < a {
                    self.as_bytes()[i]
                } else if i < b {
                    string.as_bytes()[i - a]
                } else {
                    self.as_bytes()[i - string.len()]
                };
            }

            crlf::nearest_internal_break(mid_idx - start, &buf[..(end - start)]) + start
        };

        debug_assert!(split_idx <= MAX_BYTES);

        let mut right = NodeText::new();
        if split_idx <= a {
            right.push_str(&self[split_idx..a]);
            right.push_str(string);
            right.push_str(&self[a..]);
            self.truncate(split_idx);
        } else if split_idx <= b {
            right.push_str(&string[(split_idx - a)..]);
            right.push_str(&self[a..]);
            self.truncate(a);
            self.push_str(&string[..(split_idx - a)]);
        } else {
            right.push_str(&self[(split_idx - string.len())..]);
            self.truncate(split_idx - string.len());
            self.insert_str(a, string);
        }

        right
    }

    /// Appends a `&str` to end the of the `NodeText`.
    pub fn push_str(&mut self, string: &str) {
        let len = self.len();
        unsafe {
            self.insert_bytes(len, string.as_bytes());
        }
    }

    /// Appends a `&str` and splits the resulting string in half, returning
    /// the right half.
    ///
    /// Only splits on code point boundaries and will never split CRLF pairs,
    /// grapheme boundaries, so if the whole string is a single code point or
    /// CRLF pair, the split will fail and the returned string will be empty.
    ///
    /// TODO: make this work without allocations when possible.
    pub fn push_str_split(&mut self, string: &str) -> Self {
        self.push_str(string);

        let split_pos = {
            let pos = self.len() - (self.len() / 2);
            crlf::find_good_split(pos, self.as_bytes(), false)
        };

        self.split_off(split_pos)
    }

    /// Drops the text after byte index `idx`.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    pub fn truncate(&mut self, idx: usize) {
        assert!(self.is_char_boundary(idx));
        debug_assert!(idx <= self.len());
        self.buffer.truncate(idx);
        self.inline_if_possible();
    }

    /// Drops the text before byte index `idx`, shifting the
    /// rest of the text to fill in the space.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    pub fn truncate_front(&mut self, idx: usize) {
        assert!(self.is_char_boundary(idx));
        debug_assert!(idx <= self.len());
        unsafe {
            self.remove_bytes(0, idx);
        }
    }

    /// Removes the text in the byte index interval `[start, end)`.
    ///
    /// Panics if either `start` or `end` are not char boundaries, as that
    /// would result in an invalid utf8 string.
    pub fn remove_range(&mut self, start: usize, end: usize) {
        assert!(self.is_char_boundary(start));
        assert!(self.is_char_boundary(end));
        debug_assert!(end <= self.len());
        debug_assert!(start <= end);
        unsafe {
            self.remove_bytes(start, end);
        }
        self.inline_if_possible();
    }

    pub fn remove_char_range(&mut self, start: usize, end: usize) {
        debug_assert!(start <= end);
        debug_assert!(end <= count_chars(self));

        let byte_start = char_idx_to_byte_idx(self, start);
        let byte_end = char_idx_to_byte_idx(&self[start..], end - start);

        unsafe { self.remove_bytes(byte_start, byte_end) }
        self.inline_if_possible();
    }

    /// Splits the `NodeText` at `idx`.
    ///
    /// The left part remains in the original, and the right part is
    /// returned in a new `NodeText`.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    pub fn split_off(&mut self, idx: usize) -> Self {
        assert!(self.is_char_boundary(idx));
        assert!(idx <= self.len());
        let len = self.len();
        let mut other = NodeText::with_capacity(len - idx);
        unsafe {
            ptr::copy_nonoverlapping(
                self.buffer.as_ptr().offset(idx as isize),
                other.buffer.as_mut_ptr().offset(0),
                len - idx,
            );
            self.buffer.set_len(idx);
            other.buffer.set_len(len - idx);
        }
        self.inline_if_possible();
        other
    }

    /// Splits a string into two strings at the char index given.
    /// The first section of the split is stored in the original string,
    /// while the second section of the split is returned as a new string.
    pub fn split_off_at_char(&mut self, char_idx: usize) -> Self {
        debug_assert!(char_idx <= count_chars(self));
        let byte_idx = char_idx_to_byte_idx(self, char_idx);
        self.split_off(byte_idx)
    }

    #[inline(always)]
    pub fn shrink_to_fit(&mut self) {
        self.buffer.shrink_to_fit();
    }

    /// Returns whether the string has spilled out into a heap-allocated
    /// memory area.
    pub fn spilled(&self) -> bool {
        self.buffer.spilled()
    }

    #[inline(always)]
    pub unsafe fn as_mut_smallvec(&mut self) -> &mut SmallVec<BackingArray> {
        &mut self.buffer
    }

    #[inline(always)]
    unsafe fn insert_bytes(&mut self, idx: usize, bytes: &[u8]) {
        assert!(idx <= self.len());
        let len = self.len();
        let amt = bytes.len();
        self.buffer.reserve(amt);

        ptr::copy(
            self.buffer.as_ptr().offset(idx as isize),
            self.buffer.as_mut_ptr().offset((idx + amt) as isize),
            len - idx,
        );
        ptr::copy(
            bytes.as_ptr(),
            self.buffer.as_mut_ptr().offset(idx as isize),
            amt,
        );
        self.buffer.set_len(len + amt);
    }

    #[inline(always)]
    unsafe fn remove_bytes(&mut self, start: usize, end: usize) {
        assert!(start <= end);
        assert!(end <= self.len());
        let len = self.len();
        let amt = end - start;
        ptr::copy(
            self.buffer.as_ptr().offset(end as isize),
            self.buffer.as_mut_ptr().offset(start as isize),
            len - end,
        );
        self.buffer.set_len(len - amt);

        self.inline_if_possible();
    }

    /// Re-inlines the data if it's been heap allocated but can
    /// fit inline.
    #[inline(always)]
    fn inline_if_possible(&mut self) {
        if self.buffer.spilled() && (self.buffer.len() <= self.buffer.inline_size()) {
            self.buffer.shrink_to_fit();
        }
    }
}

impl std::cmp::PartialEq for NodeText {
    fn eq(&self, other: &Self) -> bool {
        let (s1, s2): (&str, &str) = (self, other);
        s1 == s2
    }
}

impl<'a> PartialEq<NodeText> for &'a str {
    fn eq(&self, other: &NodeText) -> bool {
        *self == (other as &str)
    }
}

impl<'a> PartialEq<&'a str> for NodeText {
    fn eq(&self, other: &&'a str) -> bool {
        (self as &str) == *other
    }
}

impl std::fmt::Display for NodeText {
    fn fmt(&self, fm: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        NodeText::deref(self).fmt(fm)
    }
}

impl std::fmt::Debug for NodeText {
    fn fmt(&self, fm: &mut std::fmt::Formatter) -> std::fmt::Result {
        NodeText::deref(self).fmt(fm)
    }
}

impl<'a> From<&'a str> for NodeText {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl Deref for NodeText {
    type Target = str;

    fn deref(&self) -> &str {
        // NodeText's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

impl AsRef<str> for NodeText {
    fn as_ref(&self) -> &str {
        // NodeText's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

impl Borrow<str> for NodeText {
    fn borrow(&self) -> &str {
        // NodeText's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

//=======================================================================

/// Takes two `NodeText`s and mends the segment break between them, if any.
///
/// Note: this will leave one of the strings empty if the entire composite string
/// is one big grapheme.
pub(crate) fn fix_segment_seam(l: &mut NodeText, r: &mut NodeText) {
    // Early out, if there's nothing to do.
    if crlf::seam_is_break(l.as_bytes(), r.as_bytes()) {
        return;
    }

    // Find the nearest breaks around the seam.
    let l_split = crlf::prev_break(l.len(), l.as_bytes());
    let r_split = l.len() + crlf::next_break(0, r.as_bytes());

    // Find the new split position, if any.
    let new_split_pos = {
        let tot_len = l.len() + r.len();
        if l_split != 0 && (r_split == tot_len || l.len() > r.len()) {
            l_split
        } else {
            r_split
        }
    };

    // Move the bytes to create the new split
    if new_split_pos < l.len() {
        r.insert_str(0, &l[new_split_pos..]);
        l.truncate(new_split_pos);
    } else {
        let pos = new_split_pos - l.len();
        l.push_str(&r[..pos]);
        r.truncate_front(pos);
    }
}

//=======================================================================

/// The backing internal buffer for `NodeText`.
#[derive(Copy, Clone)]
pub(crate) struct BackingArray([u8; MAX_BYTES]);
unsafe impl Array for BackingArray {
    type Item = u8;
    fn size() -> usize {
        MAX_BYTES
    }
    fn ptr(&self) -> *const u8 {
        &self.0[0]
    }
    fn ptr_mut(&mut self) -> *mut u8 {
        &mut self.0[0]
    }
}

//=======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_bytes_01() {
        let mut s = NodeText::new();
        s.push_str("Hello there, everyone!  How's it going?");
        unsafe {
            s.remove_bytes(11, 21);
        }
        assert_eq!("Hello there!  How's it going?", s);
    }
}
