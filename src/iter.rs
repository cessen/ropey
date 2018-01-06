//! Iterators over a `Rope`'s data.
//!
//! All iterators here can also be used with `RopeSlice`'s.  When used
//! with a `RopeSlice`, they iterate over only the data that the
//! `RopeSlice` refers to.  For the line, chunk, and grapheme iterators,
//! the data of the first and last yielded item will be truncated to
//! match the `RopeSlice`.

use std::str;
use std::sync::Arc;

use unicode_segmentation;
use unicode_segmentation::UnicodeSegmentation;

use tree::Node;
use slice::RopeSlice;

//==========================================================

/// An iterator over a `Rope`'s bytes.
pub struct Bytes<'a> {
    chunk_iter: Chunks<'a>,
    cur_chunk: str::Bytes<'a>,
}

impl<'a> Bytes<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Bytes {
        Bytes {
            chunk_iter: Chunks::new(node),
            cur_chunk: "".bytes(),
        }
    }

    pub(crate) fn new_with_range(node: &Arc<Node>, start_char: usize, end_char: usize) -> Bytes {
        Bytes {
            chunk_iter: Chunks::new_with_range(node, start_char, end_char),
            cur_chunk: "".bytes(),
        }
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        loop {
            if let Some(c) = self.cur_chunk.next() {
                return Some(c);
            } else if let Some(chunk) = self.chunk_iter.next() {
                self.cur_chunk = chunk.bytes();
                continue;
            } else {
                return None;
            }
        }
    }
}

//==========================================================

/// An iterator over a `Rope`'s chars.
pub struct Chars<'a> {
    chunk_iter: Chunks<'a>,
    cur_chunk: str::Chars<'a>,
}

impl<'a> Chars<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Chars {
        Chars {
            chunk_iter: Chunks::new(node),
            cur_chunk: "".chars(),
        }
    }

    pub(crate) fn new_with_range(node: &Arc<Node>, start_char: usize, end_char: usize) -> Chars {
        Chars {
            chunk_iter: Chunks::new_with_range(node, start_char, end_char),
            cur_chunk: "".chars(),
        }
    }
}

impl<'a> Iterator for Chars<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        loop {
            if let Some(c) = self.cur_chunk.next() {
                return Some(c);
            } else if let Some(chunk) = self.chunk_iter.next() {
                self.cur_chunk = chunk.chars();
                continue;
            } else {
                return None;
            }
        }
    }
}

//==========================================================

/// An iterator over a `Rope`'s grapheme clusters.
///
/// The grapheme clusters returned are the extended grapheme
/// clusters in [Unicode Standard Annex #29](https://www.unicode.org/reports/tr29/).
/// Each grapheme cluster is returned as a utf8 `&str` slice.
pub struct Graphemes<'a> {
    chunk_iter: Chunks<'a>,
    cur_chunk: unicode_segmentation::Graphemes<'a>,
    extended: bool,
}

impl<'a> Graphemes<'a> {
    pub(crate) fn new(node: &Arc<Node>, extended: bool) -> Graphemes {
        Graphemes {
            chunk_iter: Chunks::new(node),
            cur_chunk: UnicodeSegmentation::graphemes("", extended),
            extended: extended,
        }
    }

    pub(crate) fn new_with_range(
        node: &Arc<Node>,
        extended: bool,
        start_char: usize,
        end_char: usize,
    ) -> Graphemes {
        Graphemes {
            chunk_iter: Chunks::new_with_range(node, start_char, end_char),
            cur_chunk: UnicodeSegmentation::graphemes("", extended),
            extended: extended,
        }
    }
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        loop {
            if let Some(g) = self.cur_chunk.next() {
                return Some(g);
            } else if let Some(chunk) = self.chunk_iter.next() {
                self.cur_chunk = UnicodeSegmentation::graphemes(chunk, self.extended);
                continue;
            } else {
                return None;
            }
        }
    }
}

//==========================================================

/// An iterator over a `Rope`'s lines.
///
/// The returned lines include the line-break at the end.
///
/// The last line is returned even if blank, in which case it
/// is returned as an empty slice.
pub struct Lines<'a> {
    node: &'a Arc<Node>,
    start_char: usize,
    end_char: usize,
    line_idx: usize,
}

impl<'a> Lines<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Lines {
        Lines {
            node: node,
            start_char: 0,
            end_char: node.text_info().chars as usize,
            line_idx: 0,
        }
    }

    pub(crate) fn new_with_range(node: &Arc<Node>, start_char: usize, end_char: usize) -> Lines {
        Lines {
            node: node,
            start_char: start_char,
            end_char: end_char,
            line_idx: node.char_to_line(start_char),
        }
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<RopeSlice<'a>> {
        if self.line_idx > self.node.line_break_count() {
            return None;
        } else {
            let a = self.node.line_to_char(self.line_idx).max(self.start_char);

            // Early out if we're past the specified end char
            if a > self.end_char {
                self.line_idx = self.node.line_break_count() + 1;
                return None;
            }

            let b = if self.line_idx < self.node.line_break_count() {
                self.node.line_to_char(self.line_idx + 1)
            } else {
                self.node.char_count()
            }.min(self.end_char);

            self.line_idx += 1;

            return Some(RopeSlice::new_with_range(self.node, a, b));
        }
    }
}

//==========================================================

/// An iterator over a `Rope`'s contiguous `str` chunks.
///
/// Internally, each `Rope` stores text as a segemented collection of utf8
/// strings. This iterator iterates over those segments, returning a
/// `&str` slice for each one.  It is useful for situations such as:
///
/// - Writing a rope's text data to disk.
/// - Streaming a rope's text data somewhere.
/// - Saving a rope to a non-utf8 encoding, doing the encoding conversion
///   incrementally as you go.
/// - Writing custom iterators over a rope's text data.
///
/// There are only two API guarantees about the chunks this iterator yields:
///
/// 1. They are in-order, non-overlapping, and complete (i.e. the entire
///    text is iterated over in order).
/// 2. Grapheme clusters are _never_ split between chunks.  (Grapheme
///    clusters in this case are defined as the extended grapheme
///    clusters in [Unicode Standard Annex #29](https://www.unicode.org/reports/tr29/))
///
/// There are no other API guarantees.  For example, chunks can
/// theoretically be of any size (including empty), line breaks and chunk
/// boundaries have no guaranteed relationship, etc.
///
/// The converse of this API is [`RopeBuilder`](../struct.RopeBuilder.html),
/// which is useful for efficiently streaming text data _into_ a rope.
pub struct Chunks<'a> {
    node_stack: Vec<&'a Arc<Node>>,
    start: usize,
    end: usize,
    idx: usize,
}

impl<'a> Chunks<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Chunks {
        Chunks {
            node_stack: vec![node],
            start: 0,
            end: node.text_info().bytes as usize,
            idx: 0,
        }
    }

    pub(crate) fn new_with_range(node: &Arc<Node>, start_char: usize, end_char: usize) -> Chunks {
        Chunks {
            node_stack: vec![node],
            start: node.char_to_byte(start_char),
            end: node.char_to_byte(end_char),
            idx: 0,
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.idx >= self.end {
            return None;
        }

        loop {
            if let Some(node) = self.node_stack.pop() {
                match **node {
                    Node::Leaf(ref text) => {
                        let start_byte = if self.start <= self.idx {
                            0
                        } else {
                            self.start - self.idx
                        };
                        let end_byte = if self.end >= (self.idx + text.len()) {
                            text.len()
                        } else {
                            self.end - self.idx
                        };
                        self.idx += text.len();
                        return Some(&text[start_byte..end_byte]);
                    }

                    Node::Internal(ref children) => {
                        // Find the first child that isn't before `self.start`,
                        // updating `self.idx` as we go.
                        let mut child_i = 0;
                        for inf in children.info().iter() {
                            if (self.idx + inf.bytes as usize) > self.start {
                                break;
                            } else {
                                self.idx += inf.bytes as usize;
                                child_i += 1;
                            }
                        }
                        // Push relevant children to the stack.
                        for child in (&children.nodes()[child_i..]).iter().rev() {
                            self.node_stack.push(child);
                        }
                    }
                }
            } else {
                return None;
            }
        }
    }
}

//===========================================================

#[cfg(test)]
mod tests {
    use unicode_segmentation::UnicodeSegmentation;
    use rope::Rope;

    const TEXT: &str = "\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        Hello there!  How're you doing?  It's a fine day, \
                        isn't it?  Aren't you glad we're alive?\r\n\
                        こんにちは！元気ですか？日はいいですね。\
                        私たちが生きだって嬉しいではないか？\r\n\
                        ";

    #[test]
    fn bytes_01() {
        let r = Rope::from_str(TEXT);
        for (br, bt) in r.bytes().zip(TEXT.bytes()) {
            assert_eq!(br, bt);
        }
    }

    #[test]
    fn chars_01() {
        let r = Rope::from_str(TEXT);
        for (cr, ct) in r.chars().zip(TEXT.chars()) {
            assert_eq!(cr, ct);
        }
    }

    #[test]
    fn graphemes_01() {
        let r = Rope::from_str(TEXT);
        for (gr, gt) in r.graphemes()
            .zip(UnicodeSegmentation::graphemes(TEXT, true))
        {
            assert_eq!(gr, gt);
        }
    }

    #[test]
    fn lines_01() {
        let r = Rope::from_str(TEXT);

        assert_eq!(34, r.lines().count());

        let mut lines = r.lines();

        assert_eq!("\r\n", lines.next().unwrap());

        for _ in 0..16 {
            assert_eq!(
                "Hello there!  How're you doing?  It's a fine day, \
                 isn't it?  Aren't you glad we're alive?\r\n",
                lines.next().unwrap()
            );
            assert_eq!(
                "こんにちは！元気ですか？日はいいですね。\
                 私たちが生きだって嬉しいではないか？\r\n",
                lines.next().unwrap()
            );
        }

        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_02() {
        let text = "Hello there!\nHow goes it?";
        let r = Rope::from_str(text);

        assert_eq!(2, r.lines().count());

        let mut lines = r.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn chunks_01() {
        let r = Rope::from_str(TEXT);

        let mut idx = 0;
        for chunk in r.chunks() {
            assert_eq!(chunk, &TEXT[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }
    }

    #[test]
    fn bytes_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 116;
        let s_end = 331;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start, s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        for (br, bt) in s1.bytes().zip(s2.bytes()) {
            assert_eq!(br, bt);
        }
    }

    #[test]
    fn chars_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 116;
        let s_end = 331;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start, s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        for (cr, ct) in s1.chars().zip(s2.chars()) {
            assert_eq!(cr, ct);
        }
    }

    #[test]
    fn graphemes_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 116;
        let s_end = 331;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start, s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        for (gr, gt) in s1.graphemes().zip(UnicodeSegmentation::graphemes(s2, true)) {
            assert_eq!(gr, gt);
        }
    }

    #[test]
    fn graphemes_sliced_02() {
        let text = "\r\n\r\n\r\n\r\n\r\n\r\n\r\n";
        let r = Rope::from_str(text);

        let s1 = r.slice(5, 11);
        let s2 = &text[5..11];

        assert_eq!(4, s1.graphemes().count());

        for (gr, gt) in s1.graphemes().zip(UnicodeSegmentation::graphemes(s2, true)) {
            assert_eq!(gr, gt);
        }
    }

    #[test]
    fn lines_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 116;
        let s_end = 331;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start, s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        for (liner, linet) in s1.lines().zip(s2.lines()) {
            assert_eq!(liner.to_string().trim_right(), linet);
        }
    }

    #[test]
    fn chunks_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 116;
        let s_end = 331;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start, s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        let mut idx = 0;
        for chunk in s1.chunks() {
            assert_eq!(chunk, &s2[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }
    }
}
