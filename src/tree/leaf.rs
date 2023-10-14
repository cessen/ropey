use super::{text_info::TextInfo, LEAF_SIZE};

/// A leaf node of the Rope, containing text.
///
/// Leaf nodes store their text as a gap buffer.  However, with the
/// exception of the methods for getting direct access to the left/right
/// text chunks of the gap buffer, all of its APIs behave as if the text
/// is a simple contiguous string.
#[derive(Copy, Clone)]
pub(crate) struct Leaf {
    buffer: [u8; LEAF_SIZE],
    gap_start: u16,
    gap_size: u16,
}

impl Leaf {
    /// Creates a new `Leaf` with the same contents as the given `&str`.
    pub fn from_str(string: &str) -> Self {
        assert!(string.len() <= LEAF_SIZE);

        let mut buffer = [0; LEAF_SIZE];
        buffer[..string.len()].copy_from_slice(string.as_bytes());

        Self {
            buffer: buffer,
            gap_start: string.len() as u16,
            gap_size: (LEAF_SIZE - string.len()) as u16,
        }
    }

    /// Returns the total length of the contained text in bytes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        LEAF_SIZE - self.free_capacity()
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
        todo!()
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

    /// Removes the text in the given right-exclusive byte range.
    ///
    /// Panics if the range isn't valid or doesn't lie on valid char
    /// indices.
    pub fn remove(&mut self, byte_idx_range: [usize; 2]) {
        assert!(byte_idx_range[0] <= byte_idx_range[1]);
        assert!(byte_idx_range[1] <= self.len());
        assert!(self.is_char_boundary(byte_idx_range[0]));
        assert!(self.is_char_boundary(byte_idx_range[1]));

        self.move_gap_start(byte_idx_range[0]);
        self.gap_size += (byte_idx_range[1] - byte_idx_range[0]) as u16;
    }

    /// Returns the chunk of text on the left of the gap.
    ///
    /// If there is no text, an empty string is returned.
    #[inline(always)]
    pub fn left_chunk(&self) -> &str {
        let chunk = &self.buffer[..self.gap_start as usize];
        debug_assert!(std::str::from_utf8(chunk).is_ok());
        unsafe { std::str::from_utf8_unchecked(chunk) }
    }

    /// Returns the chunk of text on the right of the gap.
    ///
    /// If there is no text, an empty string is returned.
    #[inline(always)]
    pub fn right_chunk(&self) -> &str {
        let chunk = &self.buffer[(self.gap_start + self.gap_size) as usize..];
        debug_assert!(std::str::from_utf8(chunk).is_ok());
        unsafe { std::str::from_utf8_unchecked(chunk) }
    }

    /// Splits the leaf into two leaves, with roughly half the text in
    /// each.
    ///
    /// This leaf will contain the left half of the text, and the
    /// returned leaf will contain the right half.
    pub fn split(&mut self) -> Self {
        let split_idx = {
            let mut idx = self.len() / 2;
            while !self.is_char_boundary(idx) {
                idx += 1;
            }
            idx
        };

        self.move_gap_start(split_idx);
        let right = Self::from_str(self.right_chunk());
        self.gap_size = LEAF_SIZE as u16 - self.gap_start;

        right
    }

    /// Appends the contents of another leaf to the end of this one.
    ///
    /// Panics if there isn't enough free space to append.
    pub fn append(&mut self, other: &Self) {
        assert!((self.len() + other.len()) <= LEAF_SIZE);

        self.move_gap_start(self.len());
        self.insert(self.len(), other.left_chunk());
        self.insert(self.len(), other.right_chunk());
    }

    pub fn move_gap_start(&mut self, byte_idx: usize) {
        assert!(byte_idx <= self.len());
        if byte_idx < self.gap_start as usize {
            self.buffer.copy_within(
                byte_idx..self.gap_start as usize,
                byte_idx + self.gap_size as usize,
            );
            self.gap_start = byte_idx as u16;
        } else if byte_idx > self.gap_start as usize {
            self.buffer.copy_within(
                (self.gap_start + self.gap_size) as usize..(byte_idx + self.gap_size as usize),
                self.gap_start as usize,
            );
            self.gap_start = byte_idx as u16;
        } else {
            // Gap is already there, so do nothing.
        }
    }

    //---------------------------------------------------------

    /// Converts the string byte index to the actual buffer index,
    /// accounting for the gap.
    #[inline(always)]
    fn real_idx(&self, byte_idx: usize) -> usize {
        if byte_idx >= self.gap_start as usize {
            self.gap_size as usize + byte_idx
        } else {
            byte_idx
        }
    }
}

//-------------------------------------------------------------

impl std::cmp::Eq for Leaf {}

impl std::cmp::PartialEq<Leaf> for Leaf {
    fn eq(&self, other: &Leaf) -> bool {
        if self.len() != other.len() {
            return false;
        }

        let mut a = [self.left_chunk().as_bytes(), self.right_chunk().as_bytes()].into_iter();
        let mut b = [
            other.left_chunk().as_bytes(),
            other.right_chunk().as_bytes(),
        ]
        .into_iter();

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

impl std::cmp::PartialEq<str> for Leaf {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let left = self.left_chunk().as_bytes();
        let right = self.right_chunk().as_bytes();

        (left == &other.as_bytes()[..left.len()]) && (right == &other.as_bytes()[left.len()..])
    }
}

impl std::cmp::PartialEq<&str> for Leaf {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl std::cmp::PartialEq<Leaf> for str {
    #[inline]
    fn eq(&self, other: &Leaf) -> bool {
        other == self
    }
}

impl std::cmp::PartialEq<Leaf> for &str {
    #[inline]
    fn eq(&self, other: &Leaf) -> bool {
        other == self
    }
}

impl std::fmt::Debug for Leaf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_fmt(format_args!(
            "Leaf {{ \"{}\", \"{}\" }}",
            self.left_chunk(),
            self.right_chunk()
        ))
    }
}

//-------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_01() {
        let leaf = Leaf::from_str("");
        assert_eq!(leaf.left_chunk(), "");
        assert_eq!(leaf.right_chunk(), "");
    }

    #[test]
    fn from_str_02() {
        let text = "Hello world!";
        let leaf = Leaf::from_str(text);
        assert_eq!(leaf.left_chunk(), text);
        assert_eq!(leaf.right_chunk(), "");
    }

    #[test]
    fn move_gap_start_01() {
        let text = "Hello world!";
        let mut leaf = Leaf::from_str(text);
        for i in 0..(text.len() + 1) {
            leaf.move_gap_start(i);
            assert_eq!(leaf.left_chunk(), &text[..i]);
            assert_eq!(leaf.right_chunk(), &text[i..]);
        }
    }

    #[test]
    fn move_gap_start_02() {
        let text = "Hello world!";
        let mut leaf = Leaf::from_str(text);
        for i in 0..(text.len() + 1) {
            let ii = text.len() - i;
            leaf.move_gap_start(ii);
            assert_eq!(leaf.left_chunk(), &text[..ii]);
            assert_eq!(leaf.right_chunk(), &text[ii..]);
        }
    }

    #[test]
    fn is_char_boundary_01() {
        let text = "Hello world!";
        let leaf = Leaf::from_str(text);
        for i in 0..(text.len() + 1) {
            assert_eq!(text.is_char_boundary(i), leaf.is_char_boundary(i));
        }
    }

    #[test]
    fn is_char_boundary_02() {
        let text = "みんな、こんにちは！";
        let leaf = Leaf::from_str(text);
        for i in 0..(text.len() + 1) {
            assert_eq!(text.is_char_boundary(i), leaf.is_char_boundary(i));
        }
    }

    #[test]
    fn comparison_true() {
        let text = "Hello world!";
        let mut leaf_1 = Leaf::from_str(text);
        let mut leaf_2 = Leaf::from_str(text);
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
        let mut leaf = Leaf::from_str(text);

        let gap_idx_list = [0, 6, leaf.len()];

        for i in gap_idx_list {
            leaf.move_gap_start(i);
            assert_eq!(leaf, text);
            assert_eq!(&leaf, text);
        }
    }

    #[test]
    fn comparison_false() {
        let mut leaf_1 = Leaf::from_str("Hello world!");
        let mut leaf_2 = Leaf::from_str("Hella world!");
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
        let mut leaf = Leaf::from_str("Hella world!");

        let gap_idx_list = [0, 6, leaf.len()];

        for i in gap_idx_list {
            leaf.move_gap_start(i);
            assert!(leaf != text);
            assert!(&leaf != text);
        }
    }

    #[test]
    fn insert_01() {
        let mut leaf = Leaf::from_str("");
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
        let mut leaf = Leaf::from_str("Hello world!");
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
        let mut leaf = Leaf::from_str("Hello world!");
        let right = leaf.split();
        assert_eq!(leaf, "Hello ");
        assert_eq!(right, "world!");
    }

    #[test]
    fn split_02() {
        let mut leaf = Leaf::from_str("Hello world!");
        leaf.move_gap_start(8);
        let right = leaf.split();
        assert_eq!(leaf, "Hello ");
        assert_eq!(right, "world!");
    }

    #[test]
    fn split_03() {
        let mut leaf = Leaf::from_str("");
        let right = leaf.split();
        assert_eq!(leaf, "");
        assert_eq!(right, "");
    }

    #[test]
    fn split_04() {
        let mut leaf = Leaf::from_str("H");
        let right = leaf.split();
        assert_eq!(leaf, "");
        assert_eq!(right, "H");
    }

    #[test]
    fn split_05() {
        let mut leaf = Leaf::from_str("人");
        let right = leaf.split();
        assert_eq!(leaf, "人");
        assert_eq!(right, "");
    }

    #[test]
    fn append_01() {
        for i in 0..7 {
            let mut leaf_1 = Leaf::from_str("Hello ");
            let mut leaf_2 = Leaf::from_str("world!");
            leaf_1.move_gap_start(i);
            leaf_2.move_gap_start(i);

            leaf_1.append(&leaf_2);

            assert_eq!("Hello world!", leaf_1);
        }
    }
}
