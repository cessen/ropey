// Code originally derived from the smallstring crate:
// https://crates.io/crates/smallstring
// Which is under the MIT license.
//
// Many methods have been added that are needed by Ropey, and the
// stuff Ropey doesn't use has been removed.

use std;

use std::borrow::Borrow;
use std::iter::IntoIterator;
use std::ops::Deref;
use std::ptr;
use std::str;

use smallvec::{Array, SmallVec};


#[derive(Clone, Default)]
pub(crate) struct SmallString<B: Array<Item = u8>> {
    buffer: SmallVec<B>,
}

impl<B: Array<Item = u8>> SmallString<B> {
    /// Creates a new empty `SmallString`
    #[inline]
    pub fn new() -> Self {
        SmallString { buffer: SmallVec::new() }
    }

    /// Creates a new empty `SmallString` with at least `capacity` bytes
    /// of capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        SmallString { buffer: SmallVec::with_capacity(capacity) }
    }

    /// Creates a new `SmallString` with the same contents as the given `&str`.
    #[inline]
    pub fn from_str(string: &str) -> Self {
        SmallString { buffer: string.as_bytes().into_iter().cloned().collect() }
    }

    /// Inserts a `&str` at byte offset `idx`.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    #[inline]
    pub fn insert_str(&mut self, idx: usize, string: &str) {
        assert!(self.is_char_boundary(idx));
        assert!(idx <= self.len());

        unsafe {
            self.insert_bytes(idx, string.as_bytes());
        }
    }

    /// Appends a `&str` to end the of the `SmallString`.
    #[inline]
    pub fn push_str(&mut self, string: &str) {
        let len = self.len();
        unsafe {
            self.insert_bytes(len, string.as_bytes());
        }
    }

    /// Drops the text after byte index `idx`.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    #[inline]
    pub fn truncate(&mut self, idx: usize) {
        assert!(self.is_char_boundary(idx));
        assert!(idx <= self.len());
        self.buffer.truncate(idx);
    }

    /// Drops the text before byte index `idx`, shifting the
    /// rest of the text to fill in the space.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    #[inline]
    pub fn truncate_front(&mut self, idx: usize) {
        assert!(self.is_char_boundary(idx));
        assert!(idx <= self.len());
        unsafe {
            self.remove_bytes(0, idx);
        }
    }

    /// Removes the text in the byte index interval `[start, end)`.
    ///
    /// Panics if either `start` or `end` are not char boundaries, as that
    /// would result in an invalid utf8 string.
    #[inline]
    pub fn remove_range(&mut self, start: usize, end: usize) {
        assert!(self.is_char_boundary(start));
        assert!(self.is_char_boundary(end));
        assert!(end < self.len());
        assert!(start <= end);
        unsafe {
            self.remove_bytes(start, end);
        }
    }

    /// Splits the `SmallString` at `idx`.
    ///
    /// The left part remains in the original, and the right part is
    /// returned in a new `SmallString`.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    #[inline]
    pub fn split_off(&mut self, idx: usize) -> SmallString<B> {
        assert!(self.is_char_boundary(idx));
        assert!(idx <= self.len());
        let len = self.len();
        let mut other = SmallString::with_capacity(len - idx);
        unsafe {
            ptr::copy_nonoverlapping(
                self.buffer.as_ptr().offset(idx as isize),
                other.buffer.as_mut_ptr().offset(0),
                len - idx,
            );
            self.buffer.set_len(idx);
            other.buffer.set_len(len - idx);
        }
        other
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.buffer.shrink_to_fit();
    }

    #[inline]
    pub unsafe fn as_mut_smallvec(&mut self) -> &mut SmallVec<B> {
        &mut self.buffer
    }

    unsafe fn insert_bytes(&mut self, idx: usize, bytes: &[u8]) {
        debug_assert!(idx <= self.len());
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

    unsafe fn remove_bytes(&mut self, start: usize, end: usize) {
        debug_assert!(end >= start);
        debug_assert!(end <= self.len());
        let len = self.len();
        let amt = end - start;
        ptr::copy(
            self.buffer.as_ptr().offset(end as isize),
            self.buffer.as_mut_ptr().offset(start as isize),
            len - end,
        );
        self.buffer.set_len(len - amt);
    }
}


impl<B: Array<Item = u8>> std::cmp::PartialEq for SmallString<B> {
    fn eq(&self, other: &Self) -> bool {
        let (s1, s2): (&str, &str) = (self, other);
        s1 == s2
    }
}

impl<'a, B: Array<Item = u8>> PartialEq<SmallString<B>> for &'a str {
    fn eq(&self, other: &SmallString<B>) -> bool {
        *self == (other as &str)
    }
}

impl<B: Array<Item = u8>> std::fmt::Display for SmallString<B> {
    fn fmt(&self, fm: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let s: &str = SmallString::deref(self);
        s.fmt(fm)
    }
}

impl<B: Array<Item = u8>> std::fmt::Debug for SmallString<B> {
    fn fmt(&self, fm: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s: &str = SmallString::deref(self);
        s.fmt(fm)
    }
}

impl<'a, B: Array<Item = u8>> From<&'a str> for SmallString<B> {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl<B: Array<Item = u8>> Deref for SmallString<B> {
    type Target = str;

    fn deref(&self) -> &str {
        // SmallString's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

impl<B: Array<Item = u8>> AsRef<str> for SmallString<B> {
    fn as_ref(&self) -> &str {
        // SmallString's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

impl<B: Array<Item = u8>> Borrow<str> for SmallString<B> {
    fn borrow(&self) -> &str {
        // SmallString's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use node::BackingArray;
    type SS = SmallString<BackingArray>;

    #[test]
    fn remove_bytes_01() {
        let mut s = SS::new();
        s.push_str("Hello there, everyone!  How's it going?");
        unsafe {
            s.remove_bytes(11, 21);
        }
        assert_eq!("Hello there!  How's it going?", s);
    }
}
