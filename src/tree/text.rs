use super::{text_info::TextInfo, MAX_TEXT_SIZE};

#[cfg(feature = "metric_chars")]
use str_indices::chars;

#[cfg(feature = "metric_utf16")]
use str_indices::utf16;

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
use crate::{str_utils::lines, LineType};

/// A leaf node of the Rope, containing text.
#[derive(Copy, Clone)]
pub(crate) struct Text(inner::Buffer);

impl Text {
    //---------------------------------------------------------
    // Create.

    /// Creates a new empty `Text`.
    #[inline(always)]
    pub fn new() -> Self {
        Self(inner::Buffer::new())
    }

    /// Creates a new `Text` with the same contents as the given `&str`.
    #[inline(always)]
    pub fn from_str(string: &str) -> Self {
        Text(inner::Buffer::from_str(string))
    }

    //---------------------------------------------------------
    // Query.

    /// Returns the total length of the contained text in bytes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the amount of free space in this text buffer, in bytes.
    #[inline(always)]
    pub fn free_capacity(&self) -> usize {
        self.0.free_capacity()
    }

    #[inline(always)]
    pub fn text_info(&self) -> TextInfo {
        TextInfo::from_str(self.0.text())
    }

    /// Returns the contained text as a string slice.
    #[inline(always)]
    pub fn text(&self) -> &str {
        self.0.text()
    }

    #[inline(always)]
    pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
        assert!(byte_idx <= self.len());
        self.0.is_char_boundary(byte_idx)
    }

    //---------------------------------------------------------
    // Metric conversions.

    #[cfg(feature = "metric_chars")]
    #[inline(always)]
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        chars::from_byte_idx(self.text(), byte_idx)
    }

    #[cfg(feature = "metric_chars")]
    #[inline(always)]
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        chars::to_byte_idx(self.text(), char_idx)
    }

    #[cfg(feature = "metric_utf16")]
    #[inline(always)]
    pub fn byte_to_utf16(&self, byte_idx: usize) -> usize {
        utf16::from_byte_idx(self.text(), byte_idx)
    }

    #[cfg(feature = "metric_utf16")]
    #[inline(always)]
    pub fn utf16_to_byte(&self, utf16_idx: usize) -> usize {
        utf16::to_byte_idx(self.text(), utf16_idx)
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[inline(always)]
    pub fn byte_to_line(&self, byte_idx: usize, line_type: LineType) -> usize {
        lines::from_byte_idx(self.text(), byte_idx, line_type)
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[inline(always)]
    pub fn line_to_byte(&self, line_idx: usize, line_type: LineType) -> usize {
        lines::to_byte_idx(self.text(), line_idx, line_type)
    }

    //---------------------------------------------------------
    // Modify.

    /// Inserts the given text at the given byte index, and computes an
    /// updated TextInfo for the text at the same time.
    ///
    /// Panics if there isn't enough free space or if the byte index
    /// isn't on a valid char boundary.
    #[must_use]
    pub fn insert_str_and_update_info(
        &mut self,
        byte_idx: usize,
        text: &str,
        current_info: TextInfo,
    ) -> TextInfo {
        if text.is_empty() {
            return current_info;
        }

        // Update text info based on the upcoming insertion.
        let new_info = current_info.str_insert(self.text(), byte_idx, TextInfo::from_str(text));

        self.0.insert(byte_idx, text);

        new_info
    }

    /// Removes the text in the given right-exclusive byte range, and computes
    /// an updated TextInfo for the resulting text at the same time..
    ///
    /// Panics if the range isn't valid or doesn't lie on valid char
    /// boundaries.
    #[must_use]
    pub fn remove_range_and_update_info(
        &mut self,
        byte_idx_range: [usize; 2],
        current_info: TextInfo,
    ) -> TextInfo {
        // Update text info based on the upcoming removal.
        let new_info = current_info.str_remove(self.text(), byte_idx_range);

        self.0.remove(byte_idx_range);

        new_info
    }

    /// Appends `text` to the end.
    ///
    /// Panics if there isn't enough free space.
    #[inline(always)]
    pub fn append_str(&mut self, text: &str) {
        self.0.insert(self.len(), text);
    }

    /// Prepends `text` to the start.
    ///
    /// Panics if there isn't enough free space.
    #[inline(always)]
    pub fn prepend_str(&mut self, text: &str) {
        self.0.insert(0, text);
    }

    /// Splits the leaf into two leaves, at the given byte offset.
    ///
    /// This leaf will contain the left half of the text, and the
    /// returned leaf will contain the right half.
    pub fn split(&mut self, byte_idx: usize) -> Self {
        let right = Self::from_str(&self.0.text()[byte_idx..]);
        self.0.remove([byte_idx, self.len()]);
        right
    }

    /// Equidistributes text data between `self` and `other`.  This behaves
    /// as if the text of `other` is appended to the end of `self`, and the
    /// result is then split between the two, with `other` being the right
    /// half of the text.
    pub fn distribute(&mut self, other: &mut Self) {
        let total_len = self.0.len() + other.0.len();
        let mut split_idx = (total_len + 1) / 2;

        if split_idx < self.len() {
            while !self.0.is_char_boundary(split_idx) {
                split_idx += 1;
            }
            other.0.insert(0, &self.0.text()[split_idx..]);
            self.0.remove([split_idx, self.0.len()]);
        } else if split_idx > self.len() {
            split_idx -= self.len();
            while !other.is_char_boundary(split_idx) {
                // We could subtract 1 here instead, which would avoid
                // needing the special case below.  However, this ensures
                // consistent splitting behavior regardless of whether
                // self or other has more data in it.
                split_idx += 1;
            }
            // There is a slim chance that the chosen split point would
            // overflow the left capacity.  This only happens when both
            // texts are nearly full, and thus essentially equidistributed
            // already.  Thus, if we hit that case, we simply skip doing
            // the equidistribution.
            if (self.len() + split_idx) <= MAX_TEXT_SIZE {
                self.0.insert(self.0.len(), &other.0.text()[0..split_idx]);
                other.0.remove([0, split_idx]);
            }
        } else {
            // Already equidistributed, so do nothing.
        }
    }
}

//-------------------------------------------------------------

impl std::cmp::Eq for Text {}

impl std::cmp::PartialEq<Text> for Text {
    #[inline(always)]
    fn eq(&self, other: &Text) -> bool {
        self.text() == other.text()
    }
}

impl std::cmp::PartialEq<str> for Text {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        self.text() == other
    }
}

impl std::cmp::PartialEq<&str> for Text {
    #[inline(always)]
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl std::cmp::PartialEq<Text> for str {
    #[inline(always)]
    fn eq(&self, other: &Text) -> bool {
        other == self
    }
}

impl std::cmp::PartialEq<Text> for &str {
    #[inline(always)]
    fn eq(&self, other: &Text) -> bool {
        other == self
    }
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_fmt(format_args!("Text {{ \"{}\" }}", self.0.text(),))
    }
}

//=============================================================

/// The unsafe guts of `Text`, exposed through a safe API.
///
/// Try to keep this as small as possible, and implement functionality on
/// `Text` via the safe APIs whenever possible.
mod inner {
    use super::MAX_TEXT_SIZE;
    use std::mem::{self, MaybeUninit};

    #[derive(Copy, Clone)]
    pub(crate) struct Buffer {
        buffer: [MaybeUninit<u8>; MAX_TEXT_SIZE],
        len: u16,
    }

    impl Buffer {
        #[inline(always)]
        pub fn new() -> Self {
            Self {
                buffer: [MaybeUninit::uninit(); MAX_TEXT_SIZE],
                len: 0,
            }
        }

        pub fn from_str(text: &str) -> Self {
            assert!(text.len() <= MAX_TEXT_SIZE);

            let mut buffer = Self {
                buffer: [MaybeUninit::uninit(); MAX_TEXT_SIZE],
                len: text.len() as u16,
            };

            // SAFETY: `&[MaybeUninit<u8>]` and `&[u8]` are layout compatible,
            // with elements that are `Copy`.
            buffer.buffer[..text.len()].copy_from_slice(unsafe { mem::transmute(text.as_bytes()) });

            buffer
        }

        #[inline(always)]
        pub fn len(&self) -> usize {
            self.len as usize
        }

        #[inline(always)]
        pub fn free_capacity(&self) -> usize {
            self.buffer.len() - self.len()
        }

        /// Returns whether the given byte index is a valid char
        /// boundary or not.
        ///
        /// Note: always returns true for out-of-bounds indices.  This is
        /// because it results in better code gen, and bounds checking will
        /// happen elsewhere anyway.
        #[inline(always)]
        pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
            if byte_idx >= self.len() {
                return true;
            }

            // SAFETY: We know the index is within the initialized part of
            // the buffer because of the guard clause above.
            let byte = unsafe { self.buffer[byte_idx].assume_init() };

            // Trick from rust stdlib.  Equivalent to:
            // `byte < 128 || byte >= 192`
            (byte as i8) >= -0x40
        }

        /// Returns the text of the buffer as a string slice.
        #[inline(always)]
        pub fn text(&self) -> &str {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the bytes before `len()` are guaranteed to be initialized.
            let bytes = unsafe { mem::transmute(&self.buffer[..self.len()]) };

            // SAFETY: we know that the chunks must be valid utf8, because the
            // API doesn't allow the creation of not-utf8 data or incorrectly
            // split utf8 data.
            debug_assert!(std::str::from_utf8(bytes).is_ok());
            unsafe { std::str::from_utf8_unchecked(bytes) }
        }

        pub fn insert(&mut self, byte_idx: usize, text: &str) {
            assert!(self.is_char_boundary(byte_idx));
            assert!(byte_idx <= self.len());
            assert!(self.len() + text.len() <= self.buffer.len());

            // SAFETY: the unsafe code below should be equivalent to the
            // following, except without bounds and range validity checks.
            //
            // ```
            // self.buffer.copy_within(
            //     byte_idx..self.len,
            //     byte_idx + text.len(),
            // );
            //```
            // In practice, the safe version produced very bloated branchy
            // code, being unable to elide undeeded bounds checks etc.
            //
            // The needed bounds checks for safety are taken care of by the
            // asserts at the top of this function.
            unsafe {
                let ptr = self.buffer.as_mut_ptr();
                std::ptr::copy(
                    ptr.add(byte_idx),
                    ptr.add(byte_idx + text.len()),
                    self.len() - byte_idx,
                );
            }

            let gap_slice = &mut self.buffer[byte_idx..(byte_idx + text.len())];

            // SAFETY: `&[MaybeUninit<u8>]` and `&[u8]` are layout compatible,
            // with elements that are `Copy`.
            gap_slice.copy_from_slice(unsafe { mem::transmute(text.as_bytes()) });

            self.len += text.len() as u16;
        }

        pub fn remove(&mut self, byte_idx_range: [usize; 2]) {
            assert!(self.is_char_boundary(byte_idx_range[0]));
            assert!(self.is_char_boundary(byte_idx_range[1]));
            assert!(byte_idx_range[0] <= byte_idx_range[1]);
            assert!(byte_idx_range[1] <= self.len());

            // SAFETY: the unsafe code below should be equivalent to the
            // following, except without bounds and range validity checks.
            //
            // ```
            // self.buffer.copy_within(
            //     byte_idx_range[1]..self.len(),
            //     byte_idx_range[0],
            // );
            //```
            // In practice, the safe version produced very bloated branchy
            // code, being unable to elide undeeded bounds checks etc.
            //
            // The needed bounds checks for safety are taken care of by the
            // asserts at the top of this function.
            unsafe {
                let ptr = self.buffer.as_mut_ptr();
                std::ptr::copy(
                    ptr.add(byte_idx_range[1]),
                    ptr.add(byte_idx_range[0]),
                    self.len() - byte_idx_range[1],
                );
            }

            self.len -= (byte_idx_range[1] - byte_idx_range[0]) as u16;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn buffer_from_str(text: &str) -> Buffer {
            let mut buffer = Buffer::new();
            buffer.insert(0, text);
            buffer
        }

        #[test]
        fn new_01() {
            let leaf = Buffer::new();
            assert_eq!(leaf.text(), "");
        }

        #[test]
        fn is_char_boundary_01() {
            let text = "Hello world!";
            let buf = buffer_from_str(&text);
            for i in 0..(text.len() + 1) {
                assert_eq!(text.is_char_boundary(i), buf.is_char_boundary(i));
            }
        }

        #[test]
        fn is_char_boundary_02() {
            let text = "こんにちは";
            let buf = buffer_from_str(&text);
            for i in 0..(text.len() + 1) {
                assert_eq!(text.is_char_boundary(i), buf.is_char_boundary(i));
            }
        }

        #[test]
        fn insert_01() {
            let mut buf = buffer_from_str("Hello!");
            buf.insert(0, "foo");
            assert_eq!(buf.text(), "fooHello!");
        }

        #[test]
        fn insert_02() {
            let mut buf = buffer_from_str("Hello!");
            buf.insert(3, "foo");
            assert_eq!(buf.text(), "Helfoolo!");
        }

        #[test]
        fn insert_03() {
            let mut buf = buffer_from_str("Hello!");
            buf.insert(6, "foo");
            assert_eq!(buf.text(), "Hello!foo");
        }

        #[test]
        #[should_panic]
        fn insert_04() {
            let mut buf = buffer_from_str("Hello!");
            buf.insert(7, "foo");
        }

        #[test]
        #[should_panic]
        fn insert_05() {
            let mut buf = buffer_from_str("こんに");
            buf.insert(1, "foo");
        }

        #[test]
        fn remove_01() {
            let mut buf = buffer_from_str("Hello!");
            buf.remove([0, 2]);
            assert_eq!(buf.text(), "llo!");
        }

        #[test]
        fn remove_02() {
            let mut buf = buffer_from_str("Hello!");
            buf.remove([2, 4]);
            assert_eq!(buf.text(), "Heo!");
        }

        #[test]
        fn remove_03() {
            let mut buf = buffer_from_str("Hello!");
            buf.remove([4, 6]);
            assert_eq!(buf.text(), "Hell");
        }

        #[test]
        fn remove_04() {
            let mut buf = buffer_from_str("Hello!");
            buf.remove([0, 6]);
            assert_eq!(buf.text(), "");
        }

        #[test]
        #[should_panic]
        fn remove_05() {
            let mut buf = buffer_from_str("Hello!");
            buf.remove([0, 7]);
        }

        #[test]
        #[should_panic]
        fn remove_06() {
            let mut buf = buffer_from_str("Hello!");
            buf.remove([2, 1]);
        }

        #[test]
        #[should_panic]
        fn remove_07() {
            let mut buf = buffer_from_str("こんに");
            buf.remove([1, 2]);
        }
    }
}

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_01() {
        let leaf = Text::new();
        assert_eq!(leaf.text(), "");
    }

    #[test]
    fn from_str_01() {
        let leaf = Text::from_str("");
        assert_eq!(leaf.text(), "");
    }

    #[test]
    fn from_str_02() {
        let text = "Hello world!";
        let text_info = TextInfo::from_str(text);
        let leaf = Text::from_str(text);
        assert_eq!(leaf.text(), text);
        assert_eq!(leaf.text_info(), text_info);
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn byte_to_char_01() {
        let text = Text::from_str("こんmちは");

        assert_eq!(0, text.byte_to_char(0));
        assert_eq!(0, text.byte_to_char(1));
        assert_eq!(0, text.byte_to_char(2));
        assert_eq!(1, text.byte_to_char(3));
        assert_eq!(1, text.byte_to_char(4));
        assert_eq!(1, text.byte_to_char(5));
        assert_eq!(2, text.byte_to_char(6));
        assert_eq!(3, text.byte_to_char(7));
        assert_eq!(3, text.byte_to_char(8));
        assert_eq!(3, text.byte_to_char(9));
        assert_eq!(4, text.byte_to_char(10));
        assert_eq!(4, text.byte_to_char(11));
        assert_eq!(4, text.byte_to_char(12));
        assert_eq!(5, text.byte_to_char(13));
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn char_to_byte_01() {
        let text = Text::from_str("こんmちは");

        assert_eq!(0, text.char_to_byte(0));
        assert_eq!(3, text.char_to_byte(1));
        assert_eq!(6, text.char_to_byte(2));
        assert_eq!(7, text.char_to_byte(3));
        assert_eq!(10, text.char_to_byte(4));
        assert_eq!(13, text.char_to_byte(5));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_01() {
        let text = Text::from_str("ん🐸mち");
        assert_eq!(0, text.byte_to_utf16(0));
        assert_eq!(0, text.byte_to_utf16(1));
        assert_eq!(0, text.byte_to_utf16(2));
        assert_eq!(1, text.byte_to_utf16(3));
        assert_eq!(1, text.byte_to_utf16(4));
        assert_eq!(1, text.byte_to_utf16(5));
        assert_eq!(1, text.byte_to_utf16(6));
        assert_eq!(3, text.byte_to_utf16(7));
        assert_eq!(4, text.byte_to_utf16(8));
        assert_eq!(4, text.byte_to_utf16(9));
        assert_eq!(4, text.byte_to_utf16(10));
        assert_eq!(5, text.byte_to_utf16(11));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_01() {
        let text = Text::from_str("ん🐸mち");

        assert_eq!(0, text.utf16_to_byte(0));
        assert_eq!(3, text.utf16_to_byte(1));
        assert_eq!(3, text.utf16_to_byte(2));
        assert_eq!(7, text.utf16_to_byte(3));
        assert_eq!(8, text.utf16_to_byte(4));
        assert_eq!(11, text.utf16_to_byte(5));
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn byte_to_line_01() {
        let text = Text::from_str("\r\n\r\n\n\r\r\n");
        let line_idxs = [
            [0, 0],
            [0, 0],
            [1, 1],
            [1, 1],
            [2, 2],
            [3, 3],
            [3, 4],
            [3, 4],
            [4, 5],
        ];

        #[allow(unused_variables)]
        for (j, [lf, crlf]) in line_idxs.iter().copied().enumerate() {
            #[cfg(feature = "metric_lines_lf")]
            assert_eq!(lf, text.byte_to_line(j, LineType::LF));
            #[cfg(feature = "metric_lines_lf_cr")]
            assert_eq!(crlf, text.byte_to_line(j, LineType::LF_CR));
            #[cfg(feature = "metric_lines_unicode")]
            assert_eq!(crlf, text.byte_to_line(j, LineType::All));
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn line_to_byte_01() {
        let text = Text::from_str("\r\n\r\n\n\r\r\n");

        #[cfg(feature = "metric_lines_lf")]
        let line_lf_byte_idxs = [0, 2, 4, 5, 8];
        #[cfg(any(feature = "metric_lines_lf_cr", feature = "metric_lines_unicode"))]
        let line_crlf_byte_idxs = [0, 2, 4, 5, 6, 8];

        #[cfg(feature = "metric_lines_lf")]
        for (l, byte_idx) in line_lf_byte_idxs.iter().copied().enumerate() {
            assert_eq!(byte_idx, text.line_to_byte(l, LineType::LF));
        }

        #[cfg(any(feature = "metric_lines_lf_cr", feature = "metric_lines_unicode"))]
        for (l, byte_idx) in line_crlf_byte_idxs.iter().copied().enumerate() {
            #[cfg(feature = "metric_lines_lf_cr")]
            assert_eq!(byte_idx, text.line_to_byte(l, LineType::LF_CR));
            #[cfg(feature = "metric_lines_unicode")]
            assert_eq!(byte_idx, text.line_to_byte(l, LineType::All));
        }
    }

    #[test]
    fn insert_str_and_update_info_01() {
        let mut leaf = Text::new();

        let info = leaf.insert_str_and_update_info(0, "o ", leaf.text_info());
        assert_eq!(leaf, "o ");
        assert_eq!(info, TextInfo::from_str("o "));

        let info = leaf.insert_str_and_update_info(0, "He", leaf.text_info());
        assert_eq!(leaf, "Heo ");
        assert_eq!(info, TextInfo::from_str("Heo "));

        let info = leaf.insert_str_and_update_info(2, "ll", leaf.text_info());
        assert_eq!(leaf, "Hello ");
        assert_eq!(info, TextInfo::from_str("Hello "));

        let info = leaf.insert_str_and_update_info(6, "world!", leaf.text_info());
        assert_eq!(leaf, "Hello world!");
        assert_eq!(info, TextInfo::from_str("Hello world!"));
    }

    #[test]
    fn remove_range_and_update_info_01() {
        let mut leaf = Text::from_str("Hello world!");

        let info = leaf.remove_range_and_update_info([4, 6], leaf.text_info());
        assert_eq!(leaf, "Hellworld!");
        assert_eq!(info, TextInfo::from_str("Hellworld!"));

        let info = leaf.remove_range_and_update_info([0, 3], leaf.text_info());
        assert_eq!(leaf, "lworld!");
        assert_eq!(info, TextInfo::from_str("lworld!"));

        let info = leaf.remove_range_and_update_info([4, 7], leaf.text_info());
        assert_eq!(leaf, "lwor");
        assert_eq!(info, TextInfo::from_str("lwor"));

        let info = leaf.remove_range_and_update_info([0, 4], leaf.text_info());
        assert_eq!(leaf, "");
        assert_eq!(info, TextInfo::from_str(""));
    }

    #[test]
    fn split_01() {
        let text = "Hello world!";
        let leaf = Text::from_str(text);
        for i in 0..(text.len() + 1) {
            let mut left = leaf.clone();
            let right = left.split(i);
            assert_eq!(left, &text[..i]);
            assert_eq!(right, &text[i..]);
        }
    }

    #[test]
    fn split_02() {
        let mut leaf = Text::new();
        let right = leaf.split(0);
        assert_eq!(leaf, "");
        assert_eq!(right, "");
    }

    #[test]
    #[should_panic]
    fn split_03() {
        let mut leaf = Text::from_str("人");
        let _ = leaf.split(1);
    }

    #[test]
    fn distribute_01() {
        let text = "Hello world!!";
        let expected_left = "Hello w";
        let expected_right = "orld!!";
        for split_i in 0..=text.len() {
            let mut leaf_1 = Text::from_str(&text[..split_i]);
            let mut leaf_2 = Text::from_str(&text[split_i..]);
            leaf_1.distribute(&mut leaf_2);
            assert_eq!(leaf_1, expected_left);
            assert_eq!(leaf_2, expected_right);
        }
    }

    #[test]
    fn distribute_02() {
        let text = "こんにちは";
        let expected_left = "こんに";
        let expected_right = "ちは";
        for split_i in 0..=(text.len() / 3) {
            let split_i = split_i * 3;
            let mut leaf_1 = Text::from_str(&text[..split_i]);
            let mut leaf_2 = Text::from_str(&text[split_i..]);
            leaf_1.distribute(&mut leaf_2);
            assert_eq!(leaf_1, expected_left);
            assert_eq!(leaf_2, expected_right);
        }
    }

    #[test]
    fn distribute_03() {
        // This tests a corner case where we can't split at the exact
        // desired split point because the left side is just shy of
        // being full and the right side is full and starts with a
        // multi-byte character.  In a naive implementation of
        // `distribute()` this case will panic as it tries to move
        // more data into the left side than can fit.
        let mut text_l = String::new();
        let mut text_r = String::new();
        while (text_l.len() + "a".len()) <= (MAX_TEXT_SIZE - 1) {
            text_l.push_str("a");
        }
        while (text_r.len() + "🐸".len()) <= MAX_TEXT_SIZE {
            text_r.push_str("🐸");
        }
        while (text_r.len() + "a".len()) <= MAX_TEXT_SIZE {
            text_r.push_str("a");
        }

        let mut leaf_1 = Text::from_str(&text_l);
        let mut leaf_2 = Text::from_str(&text_r);
        leaf_1.distribute(&mut leaf_2);
    }
}
