use std::borrow::Borrow;
use std::ops::Deref;
use std::str;

use crate::crlf;

/// A custom small string.  The unsafe guts of this are in `NodeSmallString`
/// further down in this file.
#[derive(Clone, Default)]
#[repr(C)]
pub(crate) struct NodeText(inner::NodeSmallString);

impl NodeText {
    /// Creates a new empty `NodeText`
    #[inline(always)]
    pub fn new() -> Self {
        NodeText(inner::NodeSmallString::new())
    }

    /// Creates a new `NodeText` with the same contents as the given `&str`.
    pub fn from_str(string: &str) -> Self {
        NodeText(inner::NodeSmallString::from_str(string))
    }

    /// Inserts a `&str` at byte offset `byte_idx`.
    pub fn insert_str(&mut self, byte_idx: usize, string: &str) {
        self.0.insert_str(byte_idx, string);
    }

    /// Inserts `string` at `byte_idx` and splits the resulting string in half,
    /// returning the right half.
    ///
    /// Only splits on code point boundaries and will never split CRLF pairs,
    /// so if the whole string is a single code point or CRLF pair, the split
    /// will fail and the returned string will be empty.
    pub fn insert_str_split(&mut self, byte_idx: usize, string: &str) -> Self {
        debug_assert!(self.is_char_boundary(byte_idx));

        let tot_len = self.len() + string.len();
        let mid_idx = tot_len / 2;
        let a = byte_idx;
        let b = byte_idx + string.len();

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

        self.0.inline_if_possible();
        right
    }

    /// Appends a `&str` to end the of the `NodeText`.
    pub fn push_str(&mut self, string: &str) {
        let len = self.len();
        self.0.insert_str(len, string);
    }

    /// Appends a `&str` and splits the resulting string in half, returning
    /// the right half.
    ///
    /// Only splits on code point boundaries and will never split CRLF pairs,
    /// so if the whole string is a single code point or CRLF pair, the split
    /// will fail and the returned string will be empty.
    pub fn push_str_split(&mut self, string: &str) -> Self {
        let len = self.len();
        self.insert_str_split(len, string)
    }

    /// Drops the text after byte index `byte_idx`.
    pub fn truncate(&mut self, byte_idx: usize) {
        self.0.truncate(byte_idx);
        self.0.inline_if_possible();
    }

    /// Drops the text before byte index `byte_idx`, shifting the
    /// rest of the text to fill in the space.
    pub fn truncate_front(&mut self, byte_idx: usize) {
        self.0.remove_range(0, byte_idx);
        self.0.inline_if_possible();
    }

    /// Removes the text in the byte index interval `[byte_start, byte_end)`.
    pub fn remove_range(&mut self, byte_start: usize, byte_end: usize) {
        self.0.remove_range(byte_start, byte_end);
        self.0.inline_if_possible();
    }

    /// Splits the `NodeText` at `byte_idx`.
    ///
    /// The left part remains in the original, and the right part is
    /// returned in a new `NodeText`.
    pub fn split_off(&mut self, byte_idx: usize) -> Self {
        let other = NodeText(self.0.split_off(byte_idx));
        self.0.inline_if_possible();
        other
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
        self.0.as_str()
    }
}

impl AsRef<str> for NodeText {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Borrow<str> for NodeText {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

//=======================================================================

/// Takes two `NodeText`s and mends the CRLF break between them, if any.
///
/// Note: this will leave one of the strings empty if the entire composite string
/// is a single CRLF pair.
pub(crate) fn fix_segment_seam(l: &mut NodeText, r: &mut NodeText) {
    // Early out, if there's nothing to do.
    if crlf::seam_is_break(l.as_bytes(), r.as_bytes()) {
        return;
    }

    let tot_len = l.len() + r.len();

    // Find the new split position, if any.
    let new_split_pos = {
        let l_split = crlf::prev_break(l.len(), l.as_bytes());
        let r_split = l.len() + crlf::next_break(0, r.as_bytes());
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

/// The unsafe guts of NodeText, exposed through a safe API.
///
/// Try to keep this as small as possible, and implement functionality on
/// NodeText via the safe APIs whenever possible.
mod inner {
    use crate::tree::MAX_BYTES;
    use smallvec::{Array, SmallVec};
    use std::str;

    /// The backing internal buffer type for `NodeText`.
    #[derive(Copy, Clone)]
    struct BackingArray([u8; MAX_BYTES]);

    /// We need a very specific size of array, which is not necessarily
    /// supported directly by the impls in the smallvec crate.  We therefore
    /// have to implement this unsafe trait for our specific array size.
    /// TODO: once integer const generics land, and smallvec updates its APIs
    /// to use them, switch over and get rid of this unsafe impl.
    unsafe impl Array for BackingArray {
        type Item = u8;
        fn size() -> usize {
            MAX_BYTES
        }
    }

    /// Internal small string for `NodeText`.
    #[derive(Clone, Default)]
    #[repr(C)]
    pub struct NodeSmallString {
        buffer: SmallVec<BackingArray>,
    }

    impl NodeSmallString {
        #[inline(always)]
        pub fn new() -> Self {
            NodeSmallString {
                buffer: SmallVec::new(),
            }
        }

        #[inline(always)]
        pub fn with_capacity(capacity: usize) -> Self {
            NodeSmallString {
                buffer: SmallVec::with_capacity(capacity),
            }
        }

        #[inline(always)]
        pub fn from_str(string: &str) -> Self {
            let mut nodetext = NodeSmallString::with_capacity(string.len());
            nodetext.insert_str(0, string);
            nodetext
        }

        #[inline(always)]
        pub fn len(&self) -> usize {
            self.buffer.len()
        }

        #[inline(always)]
        pub fn as_str(&self) -> &str {
            // NodeSmallString's methods don't allow `buffer` to become invalid
            // utf8, so this is safe.
            unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
        }

        /// Inserts `string` at `byte_idx`.
        ///
        /// Panics on out-of-bounds or of `byte_idx` isn't a char boundary.
        #[inline(always)]
        pub fn insert_str(&mut self, byte_idx: usize, string: &str) {
            assert!(self.as_str().is_char_boundary(byte_idx));

            // Copy bytes from `string` into the appropriate space in the
            // buffer.
            self.buffer.insert_from_slice(byte_idx, string.as_bytes());
        }

        /// Removes text in range `[start_byte_idx, end_byte_idx)`
        ///
        /// Panics on out-of-bounds or non-char-boundary indices.
        #[inline(always)]
        pub fn remove_range(&mut self, start_byte_idx: usize, end_byte_idx: usize) {
            assert!(start_byte_idx <= end_byte_idx);
            // Already checked by copy_within/is_char_boundary.
            debug_assert!(end_byte_idx <= self.len());
            assert!(self.as_str().is_char_boundary(start_byte_idx));
            assert!(self.as_str().is_char_boundary(end_byte_idx));
            let len = self.len();
            let amt = end_byte_idx - start_byte_idx;

            self.buffer.copy_within(end_byte_idx..len, start_byte_idx);

            self.buffer.truncate(len - amt);
        }

        /// Removes text after `byte_idx`.
        #[inline(always)]
        pub fn truncate(&mut self, byte_idx: usize) {
            // Already checked by is_char_boundary.
            debug_assert!(byte_idx <= self.len());
            assert!(self.as_str().is_char_boundary(byte_idx));
            self.buffer.truncate(byte_idx);
        }

        /// Splits at `byte_idx`, returning the right part and leaving the
        /// left part in the original.
        ///
        /// Panics on out-of-bounds or of `byte_idx` isn't a char boundary.
        #[inline(always)]
        pub fn split_off(&mut self, byte_idx: usize) -> Self {
            // Already checked by is_char_boundary.
            debug_assert!(byte_idx <= self.len());
            assert!(self.as_str().is_char_boundary(byte_idx));
            let len = self.len();
            let mut other = NodeSmallString::with_capacity(len - byte_idx);
            other.buffer.extend_from_slice(&self.buffer[byte_idx..]);
            self.buffer.truncate(byte_idx);
            other
        }

        /// Re-inlines the data if it's been heap allocated but can
        /// fit inline.
        #[inline(always)]
        pub fn inline_if_possible(&mut self) {
            if self.buffer.spilled() && (self.buffer.len() <= self.buffer.inline_size()) {
                self.buffer.shrink_to_fit();
            }
        }
    }

    //-----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn small_string_basics() {
            let s = NodeSmallString::from_str("Hello!");
            assert_eq!("Hello!", s.as_str());
            assert_eq!(6, s.len());
        }

        #[test]
        fn insert_str_01() {
            let mut s = NodeSmallString::from_str("Hello!");
            s.insert_str(3, "oz");
            assert_eq!("Helozlo!", s.as_str());
        }

        #[test]
        #[should_panic]
        fn insert_str_02() {
            let mut s = NodeSmallString::from_str("Hello!");
            s.insert_str(7, "oz");
        }

        #[test]
        #[should_panic]
        fn insert_str_03() {
            let mut s = NodeSmallString::from_str("こんにちは");
            s.insert_str(4, "oz");
        }

        #[test]
        fn remove_range_01() {
            let mut s = NodeSmallString::from_str("Hello!");
            s.remove_range(2, 4);
            assert_eq!("Heo!", s.as_str());
        }

        #[test]
        #[should_panic]
        fn remove_range_02() {
            let mut s = NodeSmallString::from_str("Hello!");
            s.remove_range(4, 2);
        }

        #[test]
        #[should_panic]
        fn remove_range_03() {
            let mut s = NodeSmallString::from_str("Hello!");
            s.remove_range(2, 7);
        }

        #[test]
        #[should_panic]
        fn remove_range_04() {
            let mut s = NodeSmallString::from_str("こんにちは");
            s.remove_range(2, 4);
        }

        #[test]
        fn truncate_01() {
            let mut s = NodeSmallString::from_str("Hello!");
            s.truncate(4);
            assert_eq!("Hell", s.as_str());
        }

        #[test]
        #[should_panic]
        fn truncate_02() {
            let mut s = NodeSmallString::from_str("Hello!");
            s.truncate(7);
        }

        #[test]
        #[should_panic]
        fn truncate_03() {
            let mut s = NodeSmallString::from_str("こんにちは");
            s.truncate(4);
        }

        #[test]
        fn split_off_01() {
            let mut s1 = NodeSmallString::from_str("Hello!");
            let s2 = s1.split_off(4);
            assert_eq!("Hell", s1.as_str());
            assert_eq!("o!", s2.as_str());
        }

        #[test]
        #[should_panic]
        fn split_off_02() {
            let mut s1 = NodeSmallString::from_str("Hello!");
            s1.split_off(7);
        }

        #[test]
        #[should_panic]
        fn split_off_03() {
            let mut s1 = NodeSmallString::from_str("こんにちは");
            s1.split_off(4);
        }
    }
}
