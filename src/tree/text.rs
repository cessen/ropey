use super::{text_info::TextInfo, MAX_TEXT_SIZE};

#[cfg(any(feature = "metric_chars", feature = "metric_utf16"))]
use str_indices::chars;

#[cfg(feature = "metric_utf16")]
use str_indices::utf16;

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
use crate::{
    str_utils::{ends_with_cr, lines, starts_with_lf},
    LineType,
};

/// A leaf node of the Rope, containing text.
///
/// Text nodes store their text as a gap buffer.  However, with the
/// exception of the methods for getting direct access to the left/right
/// text chunks of the gap buffer, all of its APIs behave as if the text
/// is a simple contiguous string.
#[derive(Copy, Clone)]
pub(crate) struct Text(inner::GapBuffer);

impl Text {
    //---------------------------------------------------------
    // Create.

    /// Creates a new empty `Text`.
    #[inline(always)]
    pub fn new() -> Self {
        Self(inner::GapBuffer::new())
    }

    /// Creates a new `Text` with the same contents as the given `&str`.
    pub fn from_str(string: &str) -> Self {
        let mut text = Self::new();
        text.0.append_to_gap(string);
        text
    }

    //---------------------------------------------------------
    // Query.

    /// Returns the total length of the contained text in bytes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the amount of free space in this leaf, in bytes.
    #[inline(always)]
    pub fn free_capacity(&self) -> usize {
        self.0.free_capacity()
    }

    pub fn text_info(&self) -> TextInfo {
        let left_info = TextInfo::from_str(self.0.chunks()[0]);
        let right_info = TextInfo::from_str(self.0.chunks()[1]);
        left_info.concat(right_info)
    }

    /// Returns the two chunk of the gap buffer, in order.
    ///
    /// Note: one or both chunks can be the empty string.
    #[inline(always)]
    pub fn chunks(&self) -> [&str; 2] {
        self.0.chunks()
    }

    #[inline(always)]
    pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
        assert!(byte_idx <= self.len());
        self.0.is_char_boundary(byte_idx)
    }

    #[inline(always)]
    pub fn find_char_boundary_l(&self, mut byte_idx: usize) -> usize {
        let [c1, c2] = self.chunks();

        if byte_idx < c1.len() {
            while (c1.as_bytes()[byte_idx] >> 6) == 0b10 && byte_idx > 0 {
                byte_idx -= 1;
            }
            return byte_idx;
        } else {
            byte_idx -= c1.len();
            if byte_idx >= c2.len() {
                return self.len();
            }

            while (c2.as_bytes()[byte_idx] >> 6) == 0b10 && byte_idx > 0 {
                byte_idx -= 1;
            }
            return byte_idx + c1.len();
        }
    }

    //---------------------------------------------------------
    // Metric conversions.

    #[cfg(feature = "metric_chars")]
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        let [c0, c1] = self.chunks();

        if byte_idx < c0.len() {
            chars::from_byte_idx(c0, byte_idx)
        } else {
            let start_char_idx = chars::count(c0);
            let char_idx = chars::from_byte_idx(c1, byte_idx - c0.len());
            start_char_idx + char_idx
        }
    }

    #[cfg(feature = "metric_chars")]
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        let [c0, c1] = self.chunks();

        // TODO: if `chars::to_byte_idx()` also returned the char count if it
        // goes off the end, that would allow us to skip this count and be more
        // efficient.
        let c0_char_len = chars::count(c0);
        if char_idx < c0_char_len {
            chars::to_byte_idx(c0, char_idx)
        } else {
            let byte_idx = chars::to_byte_idx(c1, char_idx - c0_char_len);
            c0.len() + byte_idx
        }
    }

    #[cfg(feature = "metric_utf16")]
    pub fn byte_to_utf16(&self, byte_idx: usize) -> usize {
        let [c0, c1] = self.chunks();

        if byte_idx < c0.len() {
            utf16::from_byte_idx(c0, byte_idx)
        } else {
            let start_char_idx = utf16::count(c0);
            let char_idx = utf16::from_byte_idx(c1, byte_idx - c0.len());
            start_char_idx + char_idx
        }
    }

    #[cfg(feature = "metric_utf16")]
    pub fn utf16_to_byte(&self, utf16_idx: usize) -> usize {
        let [c0, c1] = self.chunks();

        // TODO: if `utf16::to_byte_idx()` also returned the utf16 count if it
        // goes off the end, that would allow us to skip this count and be more
        // efficient.
        let c0_utf16_len = utf16::count(c0);
        if utf16_idx < c0_utf16_len {
            utf16::to_byte_idx(c0, utf16_idx)
        } else {
            let byte_idx = utf16::to_byte_idx(c1, utf16_idx - c0_utf16_len);
            c0.len() + byte_idx
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    pub fn byte_to_line(&self, byte_idx: usize, line_type: LineType) -> usize {
        let [c0, c1] = self.chunks();

        if byte_idx < c0.len() {
            lines::from_byte_idx(c0, byte_idx, line_type)
        } else {
            let start_line_idx = lines::count_breaks(c0, line_type)
                - lines::has_crlf_split(c0, c1, line_type) as usize;
            let line_idx = lines::from_byte_idx(c1, byte_idx - c0.len(), line_type);
            start_line_idx + line_idx
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    pub fn line_to_byte(&self, line_idx: usize, line_type: LineType) -> usize {
        let [c0, c1] = self.chunks();

        // TODO: if `to_byte_idx()` also returned the line break count if it
        // goes off the end, that would allow us to skip this count and be
        // more efficient.
        let c0_line_len =
            lines::count_breaks(c0, line_type) - lines::has_crlf_split(c0, c1, line_type) as usize;
        if line_idx <= c0_line_len {
            lines::to_byte_idx(c0, line_idx, line_type)
        } else {
            let byte_idx = lines::to_byte_idx(c1, line_idx - c0_line_len, line_type);
            c0.len() + byte_idx
        }
    }

    //---------------------------------------------------------
    // Modify.

    /// Inserts the given text at the given byte index.
    ///
    /// Panics if there isn't enough free space or if the byte index
    /// isn't on a valid char boundary.
    pub fn insert_str(&mut self, byte_idx: usize, text: &str) {
        self.0.move_gap(byte_idx);
        self.0.append_to_gap(text);
    }

    /// Inserts the given text at the given byte index, and computes an
    /// updated TextInfo for the text at the same time.
    ///
    /// This is preferable when TextInfo is already available, because
    /// the update can be done much more efficiently than doing a full
    /// recompute of the info.
    ///
    /// Panics if there isn't enough free space or if the byte index
    /// isn't on a valid char boundary.
    pub fn insert_str_and_update_info(
        &mut self,
        byte_idx: usize,
        text: &str,
        mut current_info: TextInfo,
    ) -> TextInfo {
        if text.is_empty() {
            return current_info;
        }

        self.0.move_gap(byte_idx);

        // Update text info based on the upcoming insertion.
        let text_info = TextInfo::from_str(text);
        current_info += text_info;
        #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
        {
            let crlf_split_compensation_1 =
                (ends_with_cr(self.0.chunks()[0]) && starts_with_lf(self.0.chunks()[1])) as usize;
            let crlf_split_compensation_2 = (ends_with_cr(self.0.chunks()[0])
                && text_info.starts_with_lf) as usize
                + (text_info.ends_with_cr && starts_with_lf(self.0.chunks()[1])) as usize;
            #[cfg(feature = "metric_lines_cr_lf")]
            {
                current_info.line_breaks_cr_lf += crlf_split_compensation_1;
                current_info.line_breaks_cr_lf -= crlf_split_compensation_2;
            }
            #[cfg(feature = "metric_lines_unicode")]
            {
                current_info.line_breaks_unicode += crlf_split_compensation_1;
                current_info.line_breaks_unicode -= crlf_split_compensation_2;
            }

            if byte_idx == 0 {
                current_info.starts_with_lf = text_info.starts_with_lf;
            }
            if byte_idx == self.0.len() {
                current_info.ends_with_cr = text_info.ends_with_cr;
            }
        }

        // Do the actual insert at the start of the gap.
        self.0.append_to_gap(text);

        current_info
    }

    /// Appends `text` to the end.
    ///
    /// Panics if there isn't enough free space.
    #[inline(always)]
    pub fn append_str(&mut self, text: &str) {
        self.0.move_gap(self.0.len());
        self.0.append_to_gap(text);
    }

    /// Removes the text in the given right-exclusive byte range.
    ///
    /// Panics if the range isn't valid or doesn't lie on valid char
    /// boundaries.
    pub fn remove(&mut self, byte_idx_range: [usize; 2]) {
        assert!(byte_idx_range[0] <= byte_idx_range[1]);
        self.0.move_gap(byte_idx_range[0]);
        self.0.grow_gap_r(byte_idx_range[1] - byte_idx_range[0]);
    }

    /// Splits the leaf into two leaves, at the given byte offset.
    ///
    /// This leaf will contain the left half of the text, and the
    /// returned leaf will contain the right half.
    pub fn split(&mut self, byte_idx: usize) -> Self {
        self.0.move_gap(byte_idx);
        let right = Self::from_str(self.0.chunks()[1]);
        self.0.grow_gap_r(self.0.len() - byte_idx);
        right
    }

    /// Appends the contents of another `Text` to the end of this one.
    ///
    /// Panics if there isn't enough free space to accommodate the append.
    pub fn append_text(&mut self, other: &Self) {
        self.0.move_gap(self.0.len());
        let [left_chunk, right_chunk] = other.0.chunks();
        self.0.append_to_gap(left_chunk);
        self.0.append_to_gap(right_chunk);
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
            self.0.move_gap(split_idx);
            other.0.move_gap(0);
            other.0.append_to_gap(self.0.chunks()[1]);
            self.0.grow_gap_r(self.len() - split_idx);
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
                other.0.move_gap(split_idx);
                self.0.move_gap(self.len());
                self.0.append_to_gap(other.0.chunks()[0]);
                other.0.grow_gap_l(split_idx);
            }
        } else {
            // Already equidistributed, so do nothing.
        }
    }
}

//-------------------------------------------------------------

impl std::cmp::Eq for Text {}

impl std::cmp::PartialEq<Text> for Text {
    fn eq(&self, other: &Text) -> bool {
        if self.len() != other.len() {
            return false;
        }

        let mut a = self.0.chunks().into_iter().map(|c| c.as_bytes());
        let mut b = other.0.chunks().into_iter().map(|c| c.as_bytes());

        let mut buf_a: &[u8] = &[];
        let mut buf_b: &[u8] = &[];
        loop {
            if buf_a.is_empty() {
                if let Some(a) = a.next() {
                    buf_a = a;
                } else {
                    break;
                }
            }
            if buf_b.is_empty() {
                if let Some(b) = b.next() {
                    buf_b = b;
                } else {
                    break;
                }
            }

            if buf_a.len() >= buf_b.len() {
                if &buf_a[..buf_b.len()] != buf_b {
                    return false;
                }
                buf_a = &buf_a[buf_b.len()..];
                buf_b = &[];
            } else if buf_a.len() < buf_b.len() {
                if &buf_b[..buf_a.len()] != buf_a {
                    return false;
                }
                buf_b = &buf_b[buf_a.len()..];
                buf_a = &[];
            }
        }

        return true;
    }
}

impl std::cmp::PartialEq<str> for Text {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let [left, right] = self.0.chunks();

        (left.as_bytes() == &other.as_bytes()[..left.len()])
            && (right.as_bytes() == &other.as_bytes()[left.len()..])
    }
}

impl std::cmp::PartialEq<&str> for Text {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl std::cmp::PartialEq<Text> for str {
    #[inline]
    fn eq(&self, other: &Text) -> bool {
        other == self
    }
}

impl std::cmp::PartialEq<Text> for &str {
    #[inline]
    fn eq(&self, other: &Text) -> bool {
        other == self
    }
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_fmt(format_args!(
            "Text {{ \"{}\", \"{}\" }}",
            self.0.chunks()[0],
            self.0.chunks()[1],
        ))
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
    pub(crate) struct GapBuffer {
        buffer: [MaybeUninit<u8>; MAX_TEXT_SIZE],

        /// Gap tracking data.
        gap_start: u16,
        gap_len: u16,
    }

    impl GapBuffer {
        #[inline(always)]
        pub fn new() -> Self {
            Self {
                buffer: [MaybeUninit::uninit(); MAX_TEXT_SIZE],
                gap_start: 0,
                gap_len: MAX_TEXT_SIZE as u16,
            }
        }

        #[inline(always)]
        pub fn len(&self) -> usize {
            MAX_TEXT_SIZE - self.gap_len as usize
        }

        #[inline(always)]
        pub fn free_capacity(&self) -> usize {
            self.gap_len as usize
        }

        #[inline(always)]
        pub fn gap_idx(&self) -> usize {
            self.gap_start as usize
        }

        /// Returns whether the given byte index is a valid char
        /// boundary or not.
        ///
        /// Note: always returns true for out-of-bounds indices.  This is
        /// because it results in better code gen, and bounds checking will
        /// happen elsewhere anyway.
        #[inline(always)]
        pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
            let byte_idx = self.real_idx(byte_idx);
            if byte_idx >= self.buffer.len() {
                true
            } else {
                // SAFETY: `real_idx()` only returns indices to bytes outside
                // of the gap, and thus this is guaranteed to be initialized
                // memory.
                let byte = unsafe { self.buffer[byte_idx].assume_init() };
                (byte & 0xC0) != 0x80
            }
        }

        /// Returns the two chunk of the gap buffer, in order.
        ///
        /// Note: one or both chunks can be the empty string.
        #[inline(always)]
        pub fn chunks(&self) -> [&str; 2] {
            // SAFETY: `MaybeUninit<T>` is layout compatible with `T`, and
            // the bytes outside of the gap are guaranteed to be initialized.
            let (chunk_l, chunk_r) = unsafe {
                (
                    mem::transmute(&self.buffer[..self.gap_start as usize]),
                    mem::transmute(&self.buffer[(self.gap_start + self.gap_len) as usize..]),
                )
            };

            // SAFETY: we know that the chunks must be valid utf8, because the
            // API doesn't allow the creation of not-utf8 data or incorrectly
            // split utf8 data.
            debug_assert!(std::str::from_utf8(chunk_l).is_ok());
            debug_assert!(std::str::from_utf8(chunk_r).is_ok());
            [unsafe { std::str::from_utf8_unchecked(chunk_l) }, unsafe {
                std::str::from_utf8_unchecked(chunk_r)
            }]
        }

        pub fn move_gap(&mut self, byte_idx: usize) {
            assert!(self.real_idx(byte_idx) <= self.buffer.len());
            assert!(self.is_char_boundary(byte_idx));

            if byte_idx < self.gap_start as usize {
                // SAFETY: the unsafe code below should be equivalent to
                // the following, except without bounds and range validity
                // checks.
                // ```
                // self.buffer.copy_within(
                //     byte_idx..self.gap_start as usize,
                //     byte_idx + self.gap_len as usize,
                // );
                //```
                // In practice, the safe version produced very bloated branchy
                // code, being unable to elide undeeded bounds checks etc.
                unsafe {
                    let ptr = self.buffer.as_mut_ptr();
                    std::ptr::copy(
                        ptr.offset(byte_idx as isize),
                        ptr.offset(byte_idx as isize + self.gap_len as isize),
                        self.gap_start as usize - byte_idx,
                    );
                }
            } else if byte_idx > self.gap_start as usize {
                // SAFETY: the unsafe code below should be equivalent to
                // the following, except without bounds and range validity
                // checks.
                // ```
                // self.buffer.copy_within(
                //     (self.gap_start + self.gap_len) as usize..(byte_idx + self.gap_len as usize),
                //     self.gap_start as usize,
                // );
                //```
                // In practice, the safe version produced very bloated branchy
                // code, being unable to elide undeeded bounds checks etc.
                unsafe {
                    let ptr = self.buffer.as_mut_ptr();
                    std::ptr::copy(
                        ptr.offset((self.gap_start + self.gap_len) as isize),
                        ptr.offset(self.gap_start as isize),
                        byte_idx - self.gap_start as usize,
                    );
                }
            }

            self.gap_start = byte_idx as u16;
        }

        #[inline(always)]
        pub fn append_to_gap(&mut self, text: &str) {
            assert!(text.len() <= self.free_capacity());
            let gap_slice =
                &mut self.buffer[self.gap_start as usize..(self.gap_start as usize + text.len())];

            // SAFETY: `&[MaybeUninit<u8>]` and `&[u8]` are layout compatible,
            // with elements that are `Copy`.
            gap_slice.copy_from_slice(unsafe { mem::transmute(text.as_bytes()) });

            self.gap_start += text.len() as u16;
            self.gap_len -= text.len() as u16;
        }

        #[inline(always)]
        pub fn grow_gap_l(&mut self, amount: usize) {
            assert!(amount <= self.gap_start as usize);
            assert!(self.is_char_boundary(self.gap_start as usize - amount));
            self.gap_start -= amount as u16;
            self.gap_len += amount as u16;
        }

        #[inline(always)]
        pub fn grow_gap_r(&mut self, amount: usize) {
            assert!(self.gap_start as usize + self.gap_len as usize + amount <= MAX_TEXT_SIZE);
            assert!(self.is_char_boundary(self.gap_start as usize + amount));
            self.gap_len += amount as u16;
        }

        //-----------------------------------------------------

        /// Converts a byte index relative to the total data to the actual
        /// buffer index, accounting for the gap.
        #[inline(always)]
        fn real_idx(&self, byte_idx: usize) -> usize {
            let offset = if byte_idx >= self.gap_start as usize {
                self.gap_len as usize
            } else {
                0
            };
            offset + byte_idx
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::tree::text_info::TextInfo;

        fn buffer_from_str(text: &str) -> GapBuffer {
            let mut buffer = GapBuffer::new();
            buffer.append_to_gap(text);
            buffer
        }

        #[test]
        fn new_01() {
            let leaf = GapBuffer::new();
            assert_eq!(leaf.chunks(), ["", ""]);
        }

        #[test]
        fn is_char_boundary_01() {
            let text = "Hello world!";
            let mut buf = buffer_from_str(&text);
            for gap_i in 0..=text.len() {
                buf.move_gap(gap_i);
                for i in 0..(text.len() + 1) {
                    assert_eq!(text.is_char_boundary(i), buf.is_char_boundary(i));
                }
            }
        }

        #[test]
        fn is_char_boundary_02() {
            let text = "こんにちは";
            let mut buf = buffer_from_str(&text);
            for gap_i in 0..=(text.len() / 3) {
                buf.move_gap(gap_i * 3);
                for i in 0..(text.len() + 1) {
                    assert_eq!(text.is_char_boundary(i), buf.is_char_boundary(i));
                }
            }
        }

        #[test]
        fn move_gap_01() {
            let text = "Hello world!";
            let mut buf = buffer_from_str(text);
            for i in 0..(text.len() + 1) {
                buf.move_gap(i);
                assert_eq!(buf.chunks(), [&text[..i], &text[i..]]);
            }
        }

        #[test]
        fn move_gap_02() {
            let text = "Hello world!";
            let mut buf = buffer_from_str(text);
            for i in 0..(text.len() + 1) {
                let ii = text.len() - i;
                buf.move_gap(ii);
                assert_eq!(buf.chunks(), [&text[..ii], &text[ii..]]);
            }
        }

        #[test]
        fn move_gap_03() {
            let text = "こんにちは";
            let mut buf = buffer_from_str(&text);
            for i in 0..=(text.len() / 3) {
                let ii = text.len() - (i * 3);
                buf.move_gap(ii);
                assert_eq!(buf.chunks(), [&text[..ii], &text[ii..]]);
            }
        }

        #[test]
        #[should_panic]
        fn move_gap_04() {
            let text = "こんにちは！";
            let mut buf = buffer_from_str(&text);
            for i in 0..(text.len() + 1) {
                let ii = text.len() - i;
                buf.move_gap(ii);
                assert_eq!(buf.chunks(), [&text[..ii], &text[ii..]]);
            }
        }

        #[test]
        fn append_to_gap_01() {
            let text = "Hello!";
            for i in 0..(text.len() + 1) {
                let mut buf = buffer_from_str(&text);
                buf.move_gap(i);
                buf.append_to_gap("foo");
                assert_eq!(&buf.chunks()[0][..i], &text[..i]);
                assert_eq!(&buf.chunks()[0][i..], "foo");
                assert_eq!(buf.chunks()[1], &text[i..]);
            }
        }

        #[test]
        #[should_panic]
        fn append_to_gap_02() {
            // Should panic because of trying to grow the text beyond capacity.
            let text: String = ['a'].iter().cycle().take(MAX_TEXT_SIZE - 2).collect();
            let mut buf = buffer_from_str(&text);
            buf.move_gap(4);
            buf.append_to_gap("foo");
        }

        #[test]
        fn grow_gap_l_01() {
            let text = "Hello!";
            for i in 0..text.len() {
                for g in 0..i {
                    let mut buf = buffer_from_str(&text);
                    buf.move_gap(i);
                    buf.grow_gap_l(g);
                    assert_eq!(buf.chunks()[0], &text[..(i - g)]);
                    assert_eq!(buf.chunks()[1], &text[i..]);
                }
            }
        }

        #[test]
        #[should_panic]
        fn grow_gap_l_02() {
            // Should panic because of char boundary violation.
            let text = "こんにちは";
            let mut buf = buffer_from_str(&text);
            buf.move_gap(6);
            buf.grow_gap_l(2);
        }

        #[test]
        #[should_panic]
        fn grow_gap_l_03() {
            // Should panic because of trying to grow the gap beyond the end.
            let mut buf = buffer_from_str("Hello!");
            buf.move_gap(3);
            buf.grow_gap_l(4);
        }

        #[test]
        fn grow_gap_r_01() {
            let text = "Hello!";
            for i in 0..text.len() {
                for g in 0..(text.len() - i) {
                    let mut buf = buffer_from_str(&text);
                    buf.move_gap(i);
                    buf.grow_gap_r(g);
                    assert_eq!(buf.chunks()[0], &text[..i]);
                    assert_eq!(buf.chunks()[1], &text[(i + g)..]);
                }
            }
        }

        #[test]
        #[should_panic]
        fn grow_gap_r_02() {
            // Should panic because of char boundary violation.
            let text = "こんにちは";
            let mut buf = buffer_from_str(&text);
            buf.move_gap(3);
            buf.grow_gap_r(2);
        }

        #[test]
        #[should_panic]
        fn grow_gap_r_03() {
            // Should panic because of trying to grow the gap beyond the end.
            let mut buf = buffer_from_str("Hello!");
            buf.move_gap(3);
            buf.grow_gap_r(4);
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
        assert_eq!(leaf.chunks(), ["", ""]);
    }

    #[test]
    fn from_str_01() {
        let leaf = Text::from_str("");
        assert_eq!(leaf.chunks(), ["", ""]);
    }

    #[test]
    fn from_str_02() {
        let text = "Hello world!";
        let text_info = TextInfo::from_str(text);
        let leaf = Text::from_str(text);
        assert_eq!(leaf.chunks(), [text, ""]);
        assert_eq!(leaf.text_info(), text_info);
    }

    #[test]
    fn comparison_true() {
        let text = "Hello world!";
        let mut leaf_1 = Text::from_str(text);
        let mut leaf_2 = Text::from_str(text);
        let len = text.len();

        let gap_idx_list = [
            [0, 0],
            [len, len],
            [0, len],
            [len, 0],
            [3, 3],
            [3, 6],
            [6, 3],
            [0, 3],
            [3, 0],
            [len, 3],
            [3, len],
        ];

        for [i1, i2] in gap_idx_list {
            leaf_1.0.move_gap(i1);
            leaf_2.0.move_gap(i2);
            assert_eq!(leaf_1, leaf_2);
        }
    }

    #[test]
    fn comparison_str_true() {
        let text = "Hello world!";
        let mut leaf = Text::from_str(text);

        let gap_idx_list = [0, 6, leaf.len()];

        for i in gap_idx_list {
            leaf.0.move_gap(i);
            assert_eq!(leaf, text);
            assert_eq!(&leaf, text);
        }
    }

    #[test]
    fn comparison_false() {
        let mut leaf_1 = Text::from_str("Hello world!");
        let mut leaf_2 = Text::from_str("Hella world!");
        let len = leaf_1.len();

        let gap_idx_list = [
            [0, 0],
            [len, len],
            [0, len],
            [len, 0],
            [3, 3],
            [3, 6],
            [6, 3],
            [0, 3],
            [3, 0],
            [len, 3],
            [3, len],
        ];

        for [i1, i2] in gap_idx_list {
            leaf_1.0.move_gap(i1);
            leaf_2.0.move_gap(i2);
            assert!(leaf_1 != leaf_2);
        }
    }

    #[test]
    fn comparison_str_false() {
        let text = "Hello world!";
        let mut leaf = Text::from_str("Hella world!");

        let gap_idx_list = [0, 6, leaf.len()];

        for i in gap_idx_list {
            leaf.0.move_gap(i);
            assert!(leaf != text);
            assert!(&leaf != text);
        }
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn byte_to_char_01() {
        let mut text = Text::from_str("こんmちは");
        let gap_idx_list = [0, 3, 6, 7, 10, 13];

        for i in gap_idx_list {
            text.0.move_gap(i);
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
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn char_to_byte_01() {
        let mut text = Text::from_str("こんmちは");
        let gap_idx_list = [0, 3, 6, 7, 10, 13];

        for i in gap_idx_list {
            text.0.move_gap(i);
            assert_eq!(0, text.char_to_byte(0));
            assert_eq!(3, text.char_to_byte(1));
            assert_eq!(6, text.char_to_byte(2));
            assert_eq!(7, text.char_to_byte(3));
            assert_eq!(10, text.char_to_byte(4));
            assert_eq!(13, text.char_to_byte(5));
        }
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_01() {
        let mut text = Text::from_str("ん🐸mち");
        let gap_idx_list = [0, 3, 7, 8, 11];

        for i in gap_idx_list {
            text.0.move_gap(i);
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
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_01() {
        let mut text = Text::from_str("ん🐸mち");
        let gap_idx_list = [0, 3, 7, 8, 11];

        for i in gap_idx_list {
            text.0.move_gap(i);
            assert_eq!(0, text.utf16_to_byte(0));
            assert_eq!(3, text.utf16_to_byte(1));
            assert_eq!(3, text.utf16_to_byte(2));
            assert_eq!(7, text.utf16_to_byte(3));
            assert_eq!(8, text.utf16_to_byte(4));
            assert_eq!(11, text.utf16_to_byte(5));
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn byte_to_line_01() {
        let mut text = Text::from_str("\r\n\r\n\n\r\r\n");
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

        for i in 0..=text.len() {
            text.0.move_gap(i);
            #[allow(unused_variables)]
            for (j, [lf, crlf]) in line_idxs.iter().copied().enumerate() {
                #[cfg(feature = "metric_lines_lf")]
                assert_eq!(lf, text.byte_to_line(j, LineType::LF));
                #[cfg(feature = "metric_lines_cr_lf")]
                assert_eq!(crlf, text.byte_to_line(j, LineType::CRLF));
                #[cfg(feature = "metric_lines_unicode")]
                assert_eq!(crlf, text.byte_to_line(j, LineType::All));
            }
        }
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn line_to_byte_01() {
        let mut text = Text::from_str("\r\n\r\n\n\r\r\n");
        let line_lf_byte_idxs = [0, 2, 4, 5, 8];
        let line_crlf_byte_idxs = [0, 2, 4, 5, 6, 8];

        for i in 0..=text.len() {
            text.0.move_gap(i);
            #[cfg(feature = "metric_lines_lf")]
            for (l, byte_idx) in line_lf_byte_idxs.iter().copied().enumerate() {
                assert_eq!(byte_idx, text.line_to_byte(l, LineType::LF));
            }

            #[cfg(any(feature = "metric_lines_cr_lf", feature = "metric_lines_unicode"))]
            for (l, byte_idx) in line_crlf_byte_idxs.iter().copied().enumerate() {
                #[cfg(feature = "metric_lines_cr_lf")]
                assert_eq!(byte_idx, text.line_to_byte(l, LineType::CRLF));
                #[cfg(feature = "metric_lines_unicode")]
                assert_eq!(byte_idx, text.line_to_byte(l, LineType::All));
            }
        }
    }

    #[test]
    fn insert_str_01() {
        let mut leaf = Text::new();
        leaf.insert_str(0, "o ");
        assert_eq!(leaf, "o ");
        assert_eq!(leaf.text_info(), TextInfo::from_str("o "));
        leaf.insert_str(0, "He");
        assert_eq!(leaf, "Heo ");
        assert_eq!(leaf.text_info(), TextInfo::from_str("Heo "));
        leaf.insert_str(2, "ll");
        assert_eq!(leaf, "Hello ");
        assert_eq!(leaf.text_info(), TextInfo::from_str("Hello "));
        leaf.insert_str(6, "world!");
        assert_eq!(leaf, "Hello world!");
        assert_eq!(leaf.text_info(), TextInfo::from_str("Hello world!"));
    }

    #[test]
    fn remove_01() {
        let mut leaf = Text::from_str("Hello world!");
        leaf.remove([4, 6]);
        assert_eq!(leaf, "Hellworld!");
        assert_eq!(leaf.text_info(), TextInfo::from_str("Hellworld!"));
        leaf.remove([0, 3]);
        assert_eq!(leaf, "lworld!");
        assert_eq!(leaf.text_info(), TextInfo::from_str("lworld!"));
        leaf.remove([4, 7]);
        assert_eq!(leaf, "lwor");
        assert_eq!(leaf.text_info(), TextInfo::from_str("lwor"));
        leaf.remove([0, 4]);
        assert_eq!(leaf, "");
        assert_eq!(leaf.text_info(), TextInfo::from_str(""));
    }

    #[test]
    fn split_01() {
        let text = "Hello world!";
        let mut leaf = Text::from_str(text);
        for j in 0..(text.len() + 1) {
            leaf.0.move_gap(j);
            for i in 0..(text.len() + 1) {
                let mut left = leaf.clone();
                let right = left.split(i);
                assert_eq!(left, &text[..i]);
                assert_eq!(right, &text[i..]);
            }
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
    fn append_text_01() {
        for i in 0..7 {
            let mut leaf_1 = Text::from_str("Hello ");
            let mut leaf_2 = Text::from_str("world!");
            leaf_1.0.move_gap(i);
            leaf_2.0.move_gap(i);

            leaf_1.append_text(&leaf_2);

            assert_eq!("Hello world!", leaf_1);
        }
    }

    #[test]
    fn distribute_01() {
        let text = "Hello world!!";
        let expected_left = "Hello w";
        let expected_right = "orld!!";
        for split_i in 0..=text.len() {
            for gap_l_i in 0..=split_i {
                for gap_r_i in 0..=(text.len() - split_i) {
                    let mut leaf_1 = Text::from_str(&text[..split_i]);
                    let mut leaf_2 = Text::from_str(&text[split_i..]);
                    leaf_1.0.move_gap(gap_l_i);
                    leaf_2.0.move_gap(gap_r_i);
                    leaf_1.distribute(&mut leaf_2);
                    assert_eq!(leaf_1, expected_left);
                    assert_eq!(leaf_2, expected_right);
                }
            }
        }
    }

    #[test]
    fn distribute_02() {
        let text = "こんにちは";
        let expected_left = "こんに";
        let expected_right = "ちは";
        for split_i in 0..=(text.len() / 3) {
            for gap_l_i in 0..=split_i {
                for gap_r_i in 0..=((text.len() / 3) - split_i) {
                    let split_i = split_i * 3;
                    let gap_l_i = gap_l_i * 3;
                    let gap_r_i = gap_r_i * 3;
                    let mut leaf_1 = Text::from_str(&text[..split_i]);
                    let mut leaf_2 = Text::from_str(&text[split_i..]);
                    leaf_1.0.move_gap(gap_l_i);
                    leaf_2.0.move_gap(gap_r_i);
                    leaf_1.distribute(&mut leaf_2);
                    assert_eq!(leaf_1, expected_left);
                    assert_eq!(leaf_2, expected_right);
                }
            }
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
