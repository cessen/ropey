use super::{text_info::TextInfo, MAX_TEXT_SIZE};

/// A leaf node of the Rope, containing text.
///
/// Text nodes store their text as a gap buffer.  However, with the
/// exception of the methods for getting direct access to the left/right
/// text chunks of the gap buffer, all of its APIs behave as if the text
/// is a simple contiguous string.
#[derive(Copy, Clone)]
pub(crate) struct Text {
    buffer: [u8; MAX_TEXT_SIZE],
    gap_start: u16,
    gap_size: u16,
}

impl Text {
    /// Creates a new `Text` with the same contents as the given `&str`.
    pub fn from_str(string: &str) -> Self {
        assert!(string.len() <= MAX_TEXT_SIZE);

        let mut buffer = [0; MAX_TEXT_SIZE];
        buffer[..string.len()].copy_from_slice(string.as_bytes());

        Self {
            buffer: buffer,
            gap_start: string.len() as u16,
            gap_size: (MAX_TEXT_SIZE - string.len()) as u16,
        }
    }

    /// Returns the total length of the contained text in bytes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        MAX_TEXT_SIZE - self.free_capacity()
    }

    /// Returns the amount of free space in this leaf, in bytes.
    #[inline(always)]
    pub fn free_capacity(&self) -> usize {
        self.gap_size as usize
    }

    pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
        assert!(byte_idx <= self.len());
        if byte_idx == self.len() {
            true
        } else {
            let idx = self.real_idx(byte_idx);
            (self.buffer[idx] & 0xC0) != 0x80
        }
    }

    pub fn text_info(&self) -> TextInfo {
        let [left, right] = self.chunks();
        let left_info = TextInfo::from_str(left);
        let right_info = TextInfo::from_str(right);
        left_info.combine(right_info)
    }

    /// Inserts the given text at the given byte index.
    ///
    /// Panics if there isn't enough free space or if the byte index
    /// isn't on a valid char boundary.
    pub fn insert(&mut self, byte_idx: usize, text: &str) {
        assert!(text.len() <= self.free_capacity());
        assert!(self.is_char_boundary(byte_idx));
        assert!(byte_idx <= self.len());

        self.move_gap_start(byte_idx);
        self.buffer[byte_idx..(byte_idx + text.len())].copy_from_slice(text.as_bytes());
        self.gap_start += text.len() as u16;
        self.gap_size -= text.len() as u16;
    }

    /// Appends `text` to the end.
    ///
    /// Panics if there isn't enough free space.
    #[inline(always)]
    pub fn append_str(&mut self, text: &str) {
        self.insert(self.len(), text);
    }

    /// Removes the text in the given right-exclusive byte range.
    ///
    /// Panics if the range isn't valid or doesn't lie on valid char
    /// boundaries.
    pub fn remove(&mut self, byte_idx_range: [usize; 2]) {
        assert!(byte_idx_range[0] <= byte_idx_range[1]);
        assert!(byte_idx_range[1] <= self.len());
        assert!(self.is_char_boundary(byte_idx_range[0]));
        assert!(self.is_char_boundary(byte_idx_range[1]));

        self.move_gap_start(byte_idx_range[0]);
        self.gap_size += (byte_idx_range[1] - byte_idx_range[0]) as u16;
    }

    /// Returns the two chunk of the gap buffer, in order.
    ///
    /// Note: one or both chunks can be the empty string.
    #[inline(always)]
    pub fn chunks(&self) -> [&str; 2] {
        let chunk_l = &self.buffer[..self.gap_start as usize];
        let chunk_r = &self.buffer[(self.gap_start + self.gap_size) as usize..];
        debug_assert!(std::str::from_utf8(chunk_l).is_ok());
        debug_assert!(std::str::from_utf8(chunk_r).is_ok());
        [unsafe { std::str::from_utf8_unchecked(chunk_l) }, unsafe {
            std::str::from_utf8_unchecked(chunk_r)
        }]
    }

    /// Splits the leaf into two leaves, at the given byte offset.
    ///
    /// This leaf will contain the left half of the text, and the
    /// returned leaf will contain the right half.
    pub fn split(&mut self, byte_idx: usize) -> Self {
        assert!(self.is_char_boundary(byte_idx));

        self.move_gap_start(byte_idx);
        let right = Self::from_str(self.chunks()[1]);
        self.gap_size = MAX_TEXT_SIZE as u16 - self.gap_start;

        right
    }

    /// Appends the contents of another leaf to the end of this one.
    ///
    /// Panics if there isn't enough free space to accommodate the append.
    pub fn append(&mut self, other: &Self) {
        assert!((self.len() + other.len()) <= MAX_TEXT_SIZE);

        self.move_gap_start(self.len());
        let [left_chunk, right_chunk] = other.chunks();
        self.insert(self.len(), left_chunk);
        self.insert(self.len(), right_chunk);
    }

    /// Equidistributes text data between `self` and `other`.  This behaves
    /// as if the text of `other` is appended to the end of `self`, and the
    /// result is then split between the two, with `other` being the right
    /// half of the text.
    pub fn distribute(&mut self, other: &mut Self) {
        let total_len = self.len() + other.len();
        let mut split_idx = (total_len + 1) / 2;

        if split_idx < self.len() {
            while !self.is_char_boundary(split_idx) {
                split_idx += 1;
            }
            self.move_gap_start(split_idx);
            other.insert(0, self.chunks()[1]);
            self.remove([split_idx, self.len()]);
        } else if split_idx > self.len() {
            split_idx -= self.len();
            while !other.is_char_boundary(split_idx) {
                split_idx += 1;
            }
            other.move_gap_start(split_idx);
            self.insert(self.len(), other.chunks()[0]);
            other.remove([0, split_idx]);
        } else {
            // Already equidistributed, so do nothing.
        }
    }

    //---------------------------------------------------------

    fn move_gap_start(&mut self, byte_idx: usize) {
        assert!(byte_idx <= self.len());
        assert!(self.is_char_boundary(byte_idx));

        if byte_idx < self.gap_start as usize {
            self.buffer.copy_within(
                byte_idx..self.gap_start as usize,
                byte_idx + self.gap_size as usize,
            );
        } else if byte_idx > self.gap_start as usize {
            self.buffer.copy_within(
                (self.gap_start + self.gap_size) as usize..(byte_idx + self.gap_size as usize),
                self.gap_start as usize,
            );
        } else {
            // Gap is already there, so do nothing.
        }

        self.gap_start = byte_idx as u16;
    }

    /// Converts the string byte index to the actual buffer index,
    /// accounting for the gap.
    #[inline(always)]
    fn real_idx(&self, byte_idx: usize) -> usize {
        let offset = if byte_idx >= self.gap_start as usize {
            self.gap_size as usize
        } else {
            0
        };
        offset + byte_idx
    }
}

//-------------------------------------------------------------

impl std::cmp::Eq for Text {}

impl std::cmp::PartialEq<Text> for Text {
    fn eq(&self, other: &Text) -> bool {
        if self.len() != other.len() {
            return false;
        }

        let mut a = self.chunks().into_iter().map(|c| c.as_bytes());
        let mut b = other.chunks().into_iter().map(|c| c.as_bytes());

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
        let [left, right] = self.chunks();

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
            self.chunks()[0],
            self.chunks()[1],
        ))
    }
}

//-------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_01() {
        let leaf = Text::from_str("");
        assert_eq!(leaf.chunks(), ["", ""]);
    }

    #[test]
    fn from_str_02() {
        let text = "Hello world!";
        let leaf = Text::from_str(text);
        assert_eq!(leaf.chunks(), [text, ""]);
    }

    #[test]
    fn move_gap_start_01() {
        let text = "Hello world!";
        let mut leaf = Text::from_str(text);
        for i in 0..(text.len() + 1) {
            leaf.move_gap_start(i);
            assert_eq!(leaf.chunks(), [&text[..i], &text[i..]]);
        }
    }

    #[test]
    fn move_gap_start_02() {
        let text = "Hello world!";
        let mut leaf = Text::from_str(text);
        for i in 0..(text.len() + 1) {
            let ii = text.len() - i;
            leaf.move_gap_start(ii);
            assert_eq!(leaf.chunks(), [&text[..ii], &text[ii..]]);
        }
    }

    #[test]
    fn move_gap_start_03() {
        let text = "こんにちは！";
        let mut leaf = Text::from_str(text);
        for i in 0..=(text.len() / 3) {
            let ii = text.len() - (i * 3);
            leaf.move_gap_start(ii);
            assert_eq!(leaf.chunks(), [&text[..ii], &text[ii..]]);
        }
    }

    #[test]
    #[should_panic]
    fn move_gap_start_04() {
        let text = "こんにちは！";
        let mut leaf = Text::from_str(text);
        for i in 0..(text.len() + 1) {
            let ii = text.len() - i;
            leaf.move_gap_start(ii);
            assert_eq!(leaf.chunks(), [&text[..ii], &text[ii..]]);
        }
    }

    #[test]
    fn is_char_boundary_01() {
        let text = "Hello world!";
        let mut leaf = Text::from_str(text);
        for gap_i in 0..=text.len() {
            leaf.move_gap_start(gap_i);
            for i in 0..(text.len() + 1) {
                assert_eq!(text.is_char_boundary(i), leaf.is_char_boundary(i));
            }
        }
    }

    #[test]
    fn is_char_boundary_02() {
        let text = "みんな、こんにちは！";
        let mut leaf = Text::from_str(text);
        for gap_i in 0..=(text.len() / 3) {
            leaf.move_gap_start(gap_i * 3);
            for i in 0..(text.len() + 1) {
                assert_eq!(text.is_char_boundary(i), leaf.is_char_boundary(i));
            }
        }
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
            leaf_1.move_gap_start(i1);
            leaf_2.move_gap_start(i2);
            assert_eq!(leaf_1, leaf_2);
        }
    }

    #[test]
    fn comparison_str_true() {
        let text = "Hello world!";
        let mut leaf = Text::from_str(text);

        let gap_idx_list = [0, 6, leaf.len()];

        for i in gap_idx_list {
            leaf.move_gap_start(i);
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
            leaf_1.move_gap_start(i1);
            leaf_2.move_gap_start(i2);
            assert!(leaf_1 != leaf_2);
        }
    }

    #[test]
    fn comparison_str_false() {
        let text = "Hello world!";
        let mut leaf = Text::from_str("Hella world!");

        let gap_idx_list = [0, 6, leaf.len()];

        for i in gap_idx_list {
            leaf.move_gap_start(i);
            assert!(leaf != text);
            assert!(&leaf != text);
        }
    }

    #[test]
    fn insert_01() {
        let mut leaf = Text::from_str("");
        leaf.insert(0, "o ");
        assert_eq!(leaf, "o ");
        leaf.insert(0, "He");
        assert_eq!(leaf, "Heo ");
        leaf.insert(2, "ll");
        assert_eq!(leaf, "Hello ");
        leaf.insert(6, "world!");
        assert_eq!(leaf, "Hello world!");
    }

    #[test]
    fn remove_01() {
        let mut leaf = Text::from_str("Hello world!");
        leaf.remove([4, 6]);
        assert_eq!(leaf, "Hellworld!");
        leaf.remove([0, 3]);
        assert_eq!(leaf, "lworld!");
        leaf.remove([4, 7]);
        assert_eq!(leaf, "lwor");
        leaf.remove([0, 4]);
        assert_eq!(leaf, "");
    }

    #[test]
    fn split_01() {
        let text = "Hello world!";
        let mut leaf = Text::from_str(text);
        for j in 0..(text.len() + 1) {
            leaf.move_gap_start(j);
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
        let mut leaf = Text::from_str("");
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
    fn append_01() {
        for i in 0..7 {
            let mut leaf_1 = Text::from_str("Hello ");
            let mut leaf_2 = Text::from_str("world!");
            leaf_1.move_gap_start(i);
            leaf_2.move_gap_start(i);

            leaf_1.append(&leaf_2);

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
                    leaf_1.distribute(&mut leaf_2);
                    assert_eq!(leaf_1, expected_left);
                    assert_eq!(leaf_2, expected_right);
                }
            }
        }
    }

    #[test]
    fn distribute_02() {
        let text = "こんにちは！！";
        let expected_left = "こんにち";
        let expected_right = "は！！";
        for split_i in 0..=(text.len() / 3) {
            for gap_l_i in 0..=split_i {
                for gap_r_i in 0..=((text.len() / 3) - split_i) {
                    let split_i = split_i * 3;
                    let gap_l_i = gap_l_i * 3;
                    let gap_r_i = gap_r_i * 3;
                    let mut leaf_1 = Text::from_str(&text[..split_i]);
                    let mut leaf_2 = Text::from_str(&text[split_i..]);
                    leaf_1.distribute(&mut leaf_2);
                    assert_eq!(leaf_1, expected_left);
                    assert_eq!(leaf_2, expected_right);
                }
            }
        }
    }
}
