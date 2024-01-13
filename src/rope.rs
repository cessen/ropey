// use std::io;
// use std::iter::FromIterator;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use crate::{
    end_bound_to_num,
    iter::Chunks,
    rope_builder::RopeBuilder,
    start_bound_to_num,
    tree::{Children, Node, Text, TextInfo, MAX_TEXT_SIZE},
};

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
use crate::LineType;

#[derive(Clone)]
pub struct Rope {
    pub(crate) root: Node,
    pub(crate) root_info: TextInfo,
}

impl Rope {
    //---------------------------------------------------------
    // Constructors.

    /// Creates an empty `Rope`.
    #[inline]
    pub fn new() -> Self {
        Rope {
            root: Node::Leaf(Arc::new(Text::new())),
            root_info: TextInfo::new(),
        }
    }

    /// Creates a `Rope` with the contents of `text`.
    #[inline]
    pub fn from_str(text: &str) -> Self {
        RopeBuilder::new().build_at_once(text)
    }

    //-----------------------------------------------------------------------
    // Edit methods

    /// Inserts `text` at byte index `byte_idx`.
    ///
    /// Runs in O(M log N) time, where N is the length of the `Rope` and M
    /// is the length of `text`.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::Rope;
    /// let mut rope = Rope::from_str("Hello!");
    /// rope.insert(5, " world");
    ///
    /// assert_eq!("Hello world!", rope);
    /// ```
    ///
    /// # Panics
    ///
    /// - If `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
    /// - If `byte_idx` is not on a char boundary.
    pub fn insert(&mut self, byte_idx: usize, text: &str) {
        assert!(
            byte_idx <= self.len_bytes(),
            "`byte_idx` ({}) is out of bounds (Rope length: {}).",
            byte_idx,
            self.len_bytes(),
        );

        if text.is_empty() {
            return;
        }

        // We have two cases here:
        //
        // 1. The insertion text is small enough to fit in a single node.
        // 2. The insertion text is larger than a single node can hold.
        //
        // Case #1 is easy to handle: it's just a standard insertion.  However,
        // case #2 needs more careful handling.  We handle case #2 by splitting
        // the insertion text into node-sized chunks and repeatedly inserting
        // them.
        //
        // In practice, both cases are rolled into one here, where case #1 is
        // just a special case that naturally falls out of the handling of
        // case #2.
        let mut text = text;
        while !text.is_empty() {
            // Split a chunk off from the end of the text.
            // We do this from the end instead of the front so that the repeated
            // insertions can keep re-using the same insertion point.
            //
            // NOTE: the chunks are at most `MAX_TEXT_SIZE - 4` rather than just
            // `MAX_TEXT_SIZE` to guarantee that nodes can split into node-sized
            // chunks even in the face of multi-byte chars that may prevent
            // splits at certain byte indices.  This is a subtle issue that in
            // practice only very rarely manifest, but causes panics when it
            // does.  Please do not remove that `- 4`!
            let split_idx = crate::find_split_r(
                text.len() - (MAX_TEXT_SIZE - 4).min(text.len()),
                text.as_bytes(),
            );
            let ins_text = &text[split_idx..];
            text = &text[..split_idx];

            // Do the insertion.
            let (new_root_info, residual) = self
                .root
                .insert_at_byte_idx(byte_idx, ins_text, self.root_info)
                .unwrap_or_else(|()| {
                    panic!("`byte_idx` ({}) is not at a char boundary.", byte_idx)
                });
            self.root_info = new_root_info;

            // Handle root split.
            if let Some((right_info, right_node)) = residual {
                let mut left_node = Node::Internal(Arc::new(Children::new()));
                std::mem::swap(&mut left_node, &mut self.root);

                let children = self.root.children_mut();
                children.push((self.root_info, left_node));
                children.push((right_info, right_node));
                self.root_info = children.combined_text_info();
            }
        }
    }

    /// Removes the text in the given byte index range.
    ///
    /// Uses range syntax, e.g. `2..7`, `2..`, etc.
    ///
    /// Runs in O(M + log N) time, where N is the length of the `Rope` and M
    /// is the length of the range being removed.
    ///
    /// # Example
    ///
    /// ```
    /// # use ropey::Rope;
    /// let mut rope = Rope::from_str("Hello world!");
    /// rope.remove(5..);
    ///
    /// assert_eq!("Hello", rope);
    /// ```
    ///
    /// # Panics
    ///
    /// - If the start of the range is greater than the end.
    /// - If the end of the range is out of bounds (i.e. `end > len_bytes()`).
    /// - If the range ends are not on char boundaries.
    #[inline(always)]
    pub fn remove<R>(&mut self, byte_range: R)
    where
        R: RangeBounds<usize>,
    {
        // Inner function to avoid code duplication on code gen due to the
        // generic type of `byte_range`.
        fn inner(rope: &mut Rope, start: Bound<&usize>, end: Bound<&usize>) {
            let start_idx = start_bound_to_num(start).unwrap_or(0);
            let end_idx = end_bound_to_num(end).unwrap_or_else(|| rope.len_bytes());

            assert!(
                start_idx <= end_idx,
                "Invalid byte range: start ({}) is greater than end ({}).",
                start_idx,
                end_idx,
            );
            assert!(
                end_idx <= rope.len_bytes(),
                "Byte range ([{}, {}]) is out of bounds (Rope length: {}).",
                start_idx,
                end_idx,
                rope.len_bytes(),
            );

            // Special case: if we're removing everything, just replace with a
            // fresh new rope.  This is to ensure the invariant that an empty
            // rope is always composed of a single empty leaf, which is not
            // ensured by the general removal code.
            if start_idx == 0 && end_idx == rope.len_bytes() {
                *rope = Rope::new();
                return;
            }

            let new_info = rope
                .root
                .remove_byte_range([start_idx, end_idx], rope.root_info)
                .unwrap_or_else(|()| {
                    panic!(
                        "Byte range [{}, {}] isn't on char boundaries.",
                        start_idx, end_idx
                    )
                });
            rope.root_info = new_info;

            // TODO: cleanup.
        }

        inner(self, byte_range.start_bound(), byte_range.end_bound());
    }

    //---------------------------------------------------------
    // Queries.

    pub fn len_bytes(&self) -> usize {
        self.root_info.bytes as usize
    }

    #[cfg(feature = "metric_chars")]
    pub fn len_chars(&self) -> usize {
        self.root_info.chars as usize
    }

    #[cfg(feature = "metric_utf16")]
    pub fn len_utf16(&self) -> usize {
        (self.root_info.chars + self.root_info.utf16_surrogates) as usize
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    pub fn len_lines(&self, line_type: LineType) -> usize {
        match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => (self.root_info.line_breaks_lf + 1) as usize,
            #[cfg(feature = "metric_lines_cr_lf")]
            LineType::CRLF => (self.root_info.line_breaks_cr_lf + 1) as usize,
            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => (self.root_info.line_breaks_unicode + 1) as usize,
        }
    }

    //---------------------------------------------------------
    // Metric conversions.

    #[cfg(feature = "metric_chars")]
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        assert!(byte_idx <= self.len_bytes());

        let (start_info, text, _) = self.root.get_text_at_byte(byte_idx, Some(self.root_info));
        start_info.chars as usize + text.byte_to_char(byte_idx - start_info.bytes as usize)
    }

    #[cfg(feature = "metric_chars")]
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        assert!(char_idx <= self.len_chars());

        let (start_info, text, _) = self.root.get_text_at_char(char_idx, Some(self.root_info));
        start_info.bytes as usize + text.char_to_byte(char_idx - start_info.chars as usize)
    }

    #[cfg(feature = "metric_utf16")]
    pub fn byte_to_utf16(&self, byte_idx: usize) -> usize {
        assert!(byte_idx <= self.len_bytes());

        let (start_info, text, _) = self.root.get_text_at_byte(byte_idx, Some(self.root_info));
        start_info.chars as usize
            + start_info.utf16_surrogates as usize
            + text.byte_to_utf16(byte_idx - start_info.bytes as usize)
    }

    #[cfg(feature = "metric_utf16")]
    pub fn utf16_to_byte(&self, utf16_idx: usize) -> usize {
        assert!(utf16_idx <= self.len_utf16());

        let (start_info, text, _) = self.root.get_text_at_utf16(utf16_idx, Some(self.root_info));
        start_info.bytes as usize
            + text.utf16_to_byte(
                utf16_idx - (start_info.chars as usize + start_info.utf16_surrogates as usize),
            )
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    pub fn byte_to_line(&self, byte_idx: usize, line_type: LineType) -> usize {
        assert!(byte_idx <= self.len_bytes());

        let (start_info, text, _) = self.root.get_text_at_byte(byte_idx, Some(self.root_info));

        let start_line_breaks = match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => start_info.line_breaks_lf as usize,

            #[cfg(feature = "metric_lines_cr_lf")]
            LineType::CRLF => start_info.line_breaks_cr_lf as usize,

            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => start_info.line_breaks_unicode as usize,
        };

        start_line_breaks + text.byte_to_line(byte_idx - start_info.bytes as usize, line_type)
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    pub fn line_to_byte(&self, line_idx: usize, line_type: LineType) -> usize {
        assert!(line_idx <= self.len_lines(line_type));

        let (start_info, text, _) =
            self.root
                .get_text_at_line_break(line_idx, Some(self.root_info), line_type);

        let start_line_breaks = match line_type {
            #[cfg(feature = "metric_lines_lf")]
            LineType::LF => start_info.line_breaks_lf as usize,

            #[cfg(feature = "metric_lines_cr_lf")]
            LineType::CRLF => start_info.line_breaks_cr_lf as usize,

            #[cfg(feature = "metric_lines_unicode")]
            LineType::All => start_info.line_breaks_unicode as usize,
        };

        start_info.bytes as usize
            + text.line_to_byte(line_idx - start_line_breaks as usize, line_type)
    }

    //---------------------------------------------------------
    // Iterators.

    pub fn chunks(&self) -> Chunks<'_> {
        Chunks::new(&self.root)
    }

    //---------------------------------------------------------
    // Debugging helpers.

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_invariants(&self) {
        self.assert_equal_leaf_depth();
        self.assert_no_empty_internal();
        self.assert_no_empty_non_root_leaf();
        self.assert_accurate_text_info();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_equal_leaf_depth(&self) {
        self.root.assert_equal_leaf_depth();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_no_empty_internal(&self) {
        self.root.assert_no_empty_internal();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_no_empty_non_root_leaf(&self) {
        if self.root.is_leaf() {
            // The root is allowed to be empty if it's a leaf.
            return;
        }
        self.root.assert_no_empty_leaf();
    }

    /// NOT PART OF THE PUBLIC API (hidden from docs for a reason!)
    #[doc(hidden)]
    pub fn assert_accurate_text_info(&self) {
        self.root.assert_accurate_text_info();
    }
}

//==============================================================
// Comparison impls.

impl<'a> std::cmp::PartialEq<&'a str> for Rope {
    #[inline]
    fn eq(&self, other: &&'a str) -> bool {
        if self.len_bytes() != other.len() {
            return false;
        }
        let other = other.as_bytes();

        let mut idx = 0;
        for chunk in self.chunks().flatten() {
            let chunk = chunk.as_bytes();
            if chunk != &other[idx..(idx + chunk.len())] {
                return false;
            }
            idx += chunk.len();
        }

        return true;
    }
}

impl<'a> std::cmp::PartialEq<Rope> for &'a str {
    #[inline]
    fn eq(&self, other: &Rope) -> bool {
        other == self
    }
}

//==============================================================
// Conversions.

impl From<String> for Rope {
    fn from(s: String) -> Rope {
        Rope::from_str(&s)
    }
}

impl From<Rope> for String {
    fn from(r: Rope) -> String {
        (&r).into()
    }
}

impl<'a> From<&'a Rope> for String {
    #[inline]
    fn from(r: &'a Rope) -> Self {
        let mut s = String::with_capacity(r.len_bytes());
        s.extend(r.chunks().flatten());
        s
    }
}

impl<'a> From<&'a str> for Rope {
    fn from(s: &'a str) -> Rope {
        Rope::from_str(&s)
    }
}

impl<'a> From<std::borrow::Cow<'a, str>> for Rope {
    #[inline]
    fn from(s: std::borrow::Cow<'a, str>) -> Self {
        Rope::from_str(&s)
    }
}

impl<'a> From<Rope> for std::borrow::Cow<'a, str> {
    #[inline]
    fn from(r: Rope) -> Self {
        std::borrow::Cow::Owned(String::from(r))
    }
}

/// Attempts to borrow the contents of the `Rope`, but will convert to an
/// owned string if the contents is not contiguous in memory.
///
/// Runs in best case O(1), worst case O(N).
impl<'a> From<&'a Rope> for std::borrow::Cow<'a, str> {
    #[inline]
    fn from(r: &'a Rope) -> Self {
        todo!()
    }
}

impl<'a> FromIterator<&'a str> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = &'a str>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(chunk);
        }
        builder.finish()
    }
}

impl<'a> FromIterator<std::borrow::Cow<'a, str>> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = std::borrow::Cow<'a, str>>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(&chunk);
        }
        builder.finish()
    }
}

impl FromIterator<String> for Rope {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = String>,
    {
        let mut builder = RopeBuilder::new();
        for chunk in iter {
            builder.append(&chunk);
        }
        builder.finish()
    }
}

//==============================================================
// Other impls.

impl std::default::Default for Rope {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Rope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list().entries(self.chunks()).finish()
    }
}

impl std::fmt::Display for Rope {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for chunk in self.chunks().flatten() {
            write!(f, "{}", chunk)?
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 127 bytes, 103 chars, 1 line
    const TEXT: &str = "Hello there!  How're you doing?  It's \
                        a fine day, isn't it?  Aren't you glad \
                        we're alive?  こんにちは、みんなさん！";
    // // 124 bytes, 100 chars, 4 lines
    // const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
    //                           a fine day, isn't it?\nAren't you glad \
    //                           we're alive?\nこんにちは、みんなさん！";
    // 143 bytes, 107 chars, 111 utf16 code units, 1 line
    const TEXT_EMOJI: &str = "Hello there!🐸  How're you doing?🐸  It's \
                              a fine day, isn't it?🐸  Aren't you glad \
                              we're alive?🐸  こんにちは、みんなさん！";

    fn string_remove(text: &mut String, byte_start: usize, byte_end: usize) {
        let text_r = text.split_off(byte_end);
        text.truncate(byte_start);
        text.push_str(&text_r);
    }

    #[test]
    fn remove_01() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(0..4);
        rope.remove(5..7);
        rope.remove(28..37);
        rope.remove(35..109);

        assert_eq!(rope, "o the!  How're you doing?  Ie day, ！");
    }

    #[test]
    fn remove_02() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(..42);

        assert_eq!(
            rope,
            "ne day, isn't it?  Aren't you glad we're \
             alive?  こんにちは、みんなさん！"
        );
    }

    #[test]
    fn remove_03() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(42..);

        assert_eq!(rope, "Hello there!  How're you doing?  It's a fi");
    }

    #[test]
    fn remove_04() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(..);

        assert_eq!(rope, "");
    }

    #[test]
    fn remove_05() {
        let mut rope = Rope::from_str(TEXT);
        rope.remove(42..42);

        assert_eq!(rope, TEXT);
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn byte_to_char_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.byte_to_char(0));
        assert_eq!(1, r.byte_to_char(1));
        assert_eq!(2, r.byte_to_char(2));

        assert_eq!(91, r.byte_to_char(91));
        assert_eq!(91, r.byte_to_char(92));
        assert_eq!(91, r.byte_to_char(93));

        assert_eq!(92, r.byte_to_char(94));
        assert_eq!(92, r.byte_to_char(95));
        assert_eq!(92, r.byte_to_char(96));

        assert_eq!(102, r.byte_to_char(124));
        assert_eq!(102, r.byte_to_char(125));
        assert_eq!(102, r.byte_to_char(126));
        assert_eq!(103, r.byte_to_char(127));
    }

    #[cfg(feature = "metric_chars")]
    #[test]
    fn char_to_byte_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(0, r.char_to_byte(0));
        assert_eq!(1, r.char_to_byte(1));
        assert_eq!(2, r.char_to_byte(2));

        assert_eq!(91, r.char_to_byte(91));
        assert_eq!(94, r.char_to_byte(92));
        assert_eq!(97, r.char_to_byte(93));
        assert_eq!(100, r.char_to_byte(94));

        assert_eq!(124, r.char_to_byte(102));
        assert_eq!(127, r.char_to_byte(103));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn byte_to_utf16_01() {
        let r = Rope::from_str(TEXT_EMOJI);

        assert_eq!(0, r.byte_to_utf16(0));

        assert_eq!(12, r.byte_to_utf16(12));
        assert_eq!(12, r.byte_to_utf16(13));
        assert_eq!(14, r.byte_to_utf16(16));

        assert_eq!(33, r.byte_to_utf16(35));
        assert_eq!(33, r.byte_to_utf16(36));
        assert_eq!(35, r.byte_to_utf16(39));

        assert_eq!(63, r.byte_to_utf16(67));
        assert_eq!(63, r.byte_to_utf16(70));
        assert_eq!(65, r.byte_to_utf16(71));

        assert_eq!(95, r.byte_to_utf16(101));
        assert_eq!(95, r.byte_to_utf16(102));
        assert_eq!(97, r.byte_to_utf16(105));

        assert_eq!(111, r.byte_to_utf16(143));
    }

    #[cfg(feature = "metric_utf16")]
    #[test]
    fn utf16_to_byte_01() {
        let r = Rope::from_str(TEXT_EMOJI);

        assert_eq!(0, r.utf16_to_byte(0));

        assert_eq!(12, r.utf16_to_byte(12));
        assert_eq!(16, r.utf16_to_byte(14));

        assert_eq!(35, r.utf16_to_byte(33));
        assert_eq!(39, r.utf16_to_byte(35));

        assert_eq!(67, r.utf16_to_byte(63));
        assert_eq!(71, r.utf16_to_byte(65));

        assert_eq!(101, r.utf16_to_byte(95));
        assert_eq!(105, r.utf16_to_byte(97));

        assert_eq!(143, r.utf16_to_byte(111));
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn byte_to_line_01() {
        todo!()
    }

    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_cr_lf",
        feature = "metric_lines_unicode"
    ))]
    #[test]
    fn line_to_byte_01() {
        todo!()
    }
}
