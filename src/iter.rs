//! Iterators over a `Rope`'s data.
//!
//! All iterators here can also be used with `RopeSlice`'s.  When used
//! with a `RopeSlice`, they iterate over only the data that the
//! `RopeSlice` refers to.  For the line and chunk, iterators, the data
//! of the first and last yielded item will be truncated to match the
//! `RopeSlice`.

use std::str;
use std::sync::Arc;

use slice::RopeSlice;
use str_utils::{
    char_to_byte_idx, char_to_line_idx, ends_with_line_break, line_to_byte_idx, line_to_char_idx,
};
use tree::Node;

//==========================================================

/// An iterator over a `Rope`'s bytes.
#[derive(Debug, Clone)]
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

    pub(crate) fn from_str(text: &str) -> Bytes {
        Bytes {
            chunk_iter: Chunks::new_empty(),
            cur_chunk: text.bytes(),
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
#[derive(Debug, Clone)]
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

    pub(crate) fn from_str(text: &str) -> Chars {
        Chars {
            chunk_iter: Chunks::new_empty(),
            cur_chunk: text.chars(),
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

/// An iterator over a `Rope`'s lines.
///
/// The returned lines include the line-break at the end.
///
/// The last line is returned even if blank, in which case it
/// is returned as an empty slice.
#[derive(Debug, Clone)]
pub struct Lines<'a>(LinesEnum<'a>);

#[derive(Debug, Clone)]
enum LinesEnum<'a> {
    Full {
        node: &'a Arc<Node>,
        start_char: usize,
        end_char: usize,
        line_idx: usize,
    },
    Light {
        text: &'a str,
        done: bool,
    },
}

impl<'a> Lines<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Lines {
        Lines(LinesEnum::Full {
            node: node,
            start_char: 0,
            end_char: node.text_info().chars as usize,
            line_idx: 0,
        })
    }

    pub(crate) fn new_with_range(node: &Arc<Node>, start_char: usize, end_char: usize) -> Lines {
        Lines(LinesEnum::Full {
            node: node,
            start_char: start_char,
            end_char: end_char,
            line_idx: {
                let (chunk, _, c, l) = node.get_chunk_at_char(start_char);
                l + char_to_line_idx(chunk, start_char - c)
            },
        })
    }

    pub(crate) fn from_str(text: &str) -> Lines {
        Lines(LinesEnum::Light {
            text: text,
            done: false,
        })
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<RopeSlice<'a>> {
        match *self {
            Lines(LinesEnum::Full {
                ref mut node,
                start_char,
                end_char,
                ref mut line_idx,
            }) => {
                if *line_idx > node.line_break_count() {
                    return None;
                } else {
                    let a = {
                        // Find the char that corresponds to the start of the line.
                        let (chunk, _, c, l) = node.get_chunk_at_line_break(*line_idx);
                        let a = (c + line_to_char_idx(chunk, *line_idx - l)).max(start_char);

                        // Early out if we're past the specified end char
                        if a > end_char {
                            *line_idx = node.line_break_count() + 1;
                            return None;
                        }

                        a
                    };

                    let b = if *line_idx < node.line_break_count() {
                        // Find the char that corresponds to the end of the line.
                        let (chunk, _, c, l) = node.get_chunk_at_line_break(*line_idx + 1);
                        c + line_to_char_idx(chunk, *line_idx + 1 - l)
                    } else {
                        node.char_count()
                    }
                    .min(end_char);

                    *line_idx += 1;

                    return Some(RopeSlice::new_with_range(node, a, b));
                }
            }
            Lines(LinesEnum::Light {
                ref mut text,
                ref mut done,
            }) => {
                if *done {
                    return None;
                } else {
                    let split_idx = line_to_byte_idx(text, 1);
                    let t = &text[..split_idx];
                    *text = &text[split_idx..];
                    if text.is_empty() {
                        *done = !ends_with_line_break(t);
                    }
                    return Some(t.into());
                }
            }
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
/// - Writing a rope's utf8 text data to disk (but see
///   [`Rope::write_to()`](../struct.Rope.html#method.write_to) for a
///   convenience function that does this).
/// - Streaming a rope's text data somewhere.
/// - Saving a rope to a non-utf8 encoding, doing the encoding conversion
///   incrementally as you go.
/// - Writing custom iterators over a rope's text data.
///
/// There are precisely two guarantees about the yielded chunks:
///
/// - All chunks are yielded, and they are yielded in order.
/// - CRLF pairs are never split across chunks.
///
/// There are no guarantees about the size of yielded chunks, and except for
/// CRLF pairs there are no guarantees about where the chunks are split.  For
/// example, they may be zero-sized, they don't necessarily align with line
/// breaks, etc.
#[derive(Debug, Clone)]
pub struct Chunks<'a>(ChunksEnum<'a>);

#[derive(Debug, Clone)]
enum ChunksEnum<'a> {
    Full {
        node_stack: Vec<&'a Arc<Node>>,
        start: usize,
        end: usize,
        idx: usize,
    },
    Light {
        text: &'a str,
    },
}

impl<'a> Chunks<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Chunks {
        Chunks(ChunksEnum::Full {
            node_stack: vec![node],
            start: 0,
            end: node.text_info().bytes as usize,
            idx: 0,
        })
    }

    pub(crate) fn new_empty() -> Chunks<'static> {
        Chunks(ChunksEnum::Light { text: "" })
    }

    pub(crate) fn new_with_range(node: &Arc<Node>, start_char: usize, end_char: usize) -> Chunks {
        let start_byte = {
            let (chunk, b, c, _) = node.get_chunk_at_char(start_char);
            b + char_to_byte_idx(chunk, start_char - c)
        };
        let end_byte = {
            let (chunk, b, c, _) = node.get_chunk_at_char(end_char);
            b + char_to_byte_idx(chunk, end_char - c)
        };
        Chunks(ChunksEnum::Full {
            node_stack: vec![node],
            start: start_byte,
            end: end_byte,
            idx: 0,
        })
    }

    pub(crate) fn from_str(text: &str) -> Chunks {
        Chunks(ChunksEnum::Light { text: text })
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        match *self {
            Chunks(ChunksEnum::Full {
                ref mut node_stack,
                start,
                end,
                ref mut idx,
            }) => {
                if *idx >= end {
                    return None;
                }

                loop {
                    if let Some(node) = node_stack.pop() {
                        match **node {
                            Node::Leaf(ref text) => {
                                let start_byte = if start <= *idx { 0 } else { start - *idx };
                                let end_byte = if end >= (*idx + text.len()) {
                                    text.len()
                                } else {
                                    end - *idx
                                };
                                *idx += text.len();
                                return Some(&text[start_byte..end_byte]);
                            }

                            Node::Internal(ref children) => {
                                // Find the first child that isn't before `start`,
                                // updating `idx` as we go.
                                let mut child_i = 0;
                                for inf in children.info().iter() {
                                    if (*idx + inf.bytes as usize) > start {
                                        break;
                                    } else {
                                        *idx += inf.bytes as usize;
                                        child_i += 1;
                                    }
                                }
                                // Push relevant children to the stack.
                                for child in (&children.nodes()[child_i..]).iter().rev() {
                                    node_stack.push(child);
                                }
                            }
                        }
                    } else {
                        return None;
                    }
                }
            }
            Chunks(ChunksEnum::Light { ref mut text }) => {
                if text.is_empty() {
                    return None;
                } else {
                    let t = *text;
                    *text = "";
                    return Some(t);
                }
            }
        }
    }
}

//===========================================================

#[cfg(test)]
mod tests {
    use Rope;

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
    fn lines_01() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(..);

        assert_eq!(34, r.lines().count());
        assert_eq!(34, s.lines().count());

        // Rope
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

        // Slice
        let mut lines = s.lines();
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
        let s = r.slice(..);

        assert_eq!(2, r.lines().count());
        assert_eq!(2, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_03() {
        let text = "Hello there!\nHow goes it?\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(3, r.lines().count());
        assert_eq!(3, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_04() {
        let text = "Hello there!\nHow goes it?\nYeah!";
        let r = Rope::from_str(text);
        let s1 = r.slice(..25);
        let s2 = r.slice(..26);

        assert_eq!(2, s1.lines().count());
        assert_eq!(3, s2.lines().count());

        let mut lines = s1.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s2.lines();
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_05() {
        let text = "";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(1, r.lines().count());
        assert_eq!(1, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_06() {
        let text = "a";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(1, r.lines().count());
        assert_eq!(1, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("a", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("a", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_07() {
        let text = "a\nb";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines().count());
        assert_eq!(2, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_08() {
        let text = "\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines().count());
        assert_eq!(2, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_09() {
        let text = "a\nb\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(3, r.lines().count());
        assert_eq!(3, s.lines().count());

        let mut lines = r.lines();
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines();
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
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

        let s1 = r.slice(s_start..s_end);
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

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        for (cr, ct) in s1.chars().zip(s2.chars()) {
            assert_eq!(cr, ct);
        }
    }

    #[test]
    fn lines_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 116;
        let s_end = 331;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        for (liner, linet) in s1.lines().zip(s2.lines()) {
            assert_eq!(liner.to_string().trim_end(), linet);
        }
    }

    #[test]
    fn chunks_sliced_01() {
        let r = Rope::from_str(TEXT);

        let s_start = 116;
        let s_end = 331;
        let s_start_byte = r.char_to_byte(s_start);
        let s_end_byte = r.char_to_byte(s_end);

        let s1 = r.slice(s_start..s_end);
        let s2 = &TEXT[s_start_byte..s_end_byte];

        let mut idx = 0;
        for chunk in s1.chunks() {
            assert_eq!(chunk, &s2[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }
    }
}
