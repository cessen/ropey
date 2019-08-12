//! Iterators over a `Rope`'s data.
//!
//! All iterators here can also be used with `RopeSlice`'s.  When used
//! with a `RopeSlice`, they iterate over only the data that the
//! `RopeSlice` refers to.  For the line and chunk iterators, the data
//! of the first and last yielded item will be truncated to match the
//! `RopeSlice`.

use std::str;
use std::sync::Arc;

use slice::RopeSlice;
use str_utils::{
    byte_to_line_idx, char_to_byte_idx, char_to_line_idx, ends_with_line_break, line_to_byte_idx,
    line_to_char_idx,
};
use tree::{Node, TextInfo};

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
            chunk_iter: Chunks::new_with_range(node, (start_char, end_char)),
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
            chunk_iter: Chunks::new_with_range(node, (start_char, end_char)),
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
        node_stack: Vec<(&'a Arc<Node>, usize)>, // (node ref, index of current child)
        total_bytes: usize,                      // Total bytes in the data range of the iterator.
        byte_idx: isize, // The index of the current byte relative to the data range start.
    },
    Light {
        text: &'a str,
        is_end: bool,
    },
}

impl<'a> Chunks<'a> {
    pub(crate) fn new(node: &Arc<Node>) -> Chunks {
        Chunks::new_with_range(node, (0, node.char_count()))
    }

    pub(crate) fn new_empty() -> Chunks<'static> {
        Chunks(ChunksEnum::Light {
            text: "",
            is_end: false,
        })
    }

    pub(crate) fn new_with_range(node: &Arc<Node>, char_idx_range: (usize, usize)) -> Chunks {
        Chunks::new_with_range_at(node, char_idx_range.0, char_idx_range).0
    }

    /// The main workhorse function for creating new `Chunks` iterators.
    ///
    /// Creates a new `Chunks` iterator from the given node, starting the
    /// iterator at the chunk containing the `at_char` char index (i.e. the
    /// `next()` method will yield the chunk containing that char).  The range
    /// of the iterator is bounded by `char_idx_range`.
    ///
    /// Both `at_char` and `char_idx_range` are relative to the beginning of
    /// of the passed node.
    ///
    /// Passing an `at_char` equal to the max of `char_idx_range` creates an
    /// iterator at the end of forward iteration.
    ///
    /// Returns the iterator and the byte/char/line index of its start relative
    /// to the start of the node.
    pub(crate) fn new_with_range_at(
        node: &Arc<Node>,
        at_char: usize,
        char_idx_range: (usize, usize),
    ) -> (Chunks, usize, usize, usize) {
        debug_assert!(at_char >= char_idx_range.0 && at_char <= char_idx_range.1);

        // Calculate the start and end bytes of the iterator.
        let start_byte = {
            let (chunk, b, c, _) = node.get_chunk_at_char(char_idx_range.0);
            b + char_to_byte_idx(chunk, char_idx_range.0 - c)
        };
        let end_byte = {
            let (chunk, b, c, _) = node.get_chunk_at_char(char_idx_range.1);
            b + char_to_byte_idx(chunk, char_idx_range.1 - c)
        };

        // If root is a leaf, return light version of the iter.
        if node.is_leaf() {
            if at_char == char_idx_range.1 && end_byte > start_byte {
                return (
                    Chunks(ChunksEnum::Light {
                        text: &node.leaf_text()[start_byte..end_byte],
                        is_end: true,
                    }),
                    end_byte - start_byte,
                    char_idx_range.1 - char_idx_range.0,
                    byte_to_line_idx(
                        &node.leaf_text()[start_byte..end_byte],
                        end_byte - start_byte,
                    ),
                );
            } else {
                return (
                    Chunks(ChunksEnum::Light {
                        text: &node.leaf_text()[start_byte..end_byte],
                        is_end: false,
                    }),
                    0,
                    0,
                    0,
                );
            }
        }

        // Create and populate the node stack, and determine the char index
        // within the first chunk, and byte index of the start of that chunk.
        let mut info = TextInfo::new();
        let mut char_idx = at_char;
        let node_stack = {
            let mut node_stack = Vec::new();
            let mut node_ref = node;
            loop {
                match **node_ref {
                    Node::Leaf(_) => {
                        break;
                    }
                    Node::Internal(ref children) => {
                        let (child_i, acc_info) = children.search_char_idx(char_idx);
                        node_stack.push((node_ref, child_i));
                        node_ref = &children.nodes()[child_i];
                        info += acc_info;
                        char_idx -= acc_info.chars as usize;
                    }
                }
            }
            node_stack
        };

        // Create the iterator.
        (
            Chunks(ChunksEnum::Full {
                node_stack: node_stack,
                total_bytes: end_byte - start_byte,
                byte_idx: info.bytes as isize - start_byte as isize,
            }),
            info.bytes as usize,
            info.chars as usize,
            info.line_breaks as usize,
        )
    }

    pub(crate) fn from_str(text: &str, at_end: bool) -> Chunks {
        Chunks(ChunksEnum::Light {
            text: text,
            is_end: at_end,
        })
    }

    pub fn prev(&mut self) -> Option<&'a str> {
        match *self {
            Chunks(ChunksEnum::Full {
                ref mut node_stack,
                total_bytes,
                ref mut byte_idx,
            }) => {
                if *byte_idx <= 0 {
                    return None;
                }

                // Progress the node stack if needed.
                let mut stack_idx = node_stack.len() - 1;
                if node_stack[stack_idx].1 == 0 {
                    while node_stack[stack_idx].1 == 0 {
                        if stack_idx == 0 {
                            return None;
                        } else {
                            stack_idx -= 1;
                        }
                    }
                    node_stack[stack_idx].1 -= 1;
                    while stack_idx < (node_stack.len() - 1) {
                        let child_i = node_stack[stack_idx].1;
                        let node = &node_stack[stack_idx].0.children().nodes()[child_i];
                        node_stack[stack_idx + 1] = (node, node.child_count() - 1);
                        stack_idx += 1;
                    }
                    node_stack[stack_idx].1 += 1;
                }

                // Fetch the node and child index.
                let (node, ref mut child_i) = node_stack.last_mut().unwrap();
                *child_i -= 1;

                // Get the text, sliced to the appropriate range.
                let text = node.children().nodes()[*child_i].leaf_text();
                *byte_idx -= text.len() as isize;
                let text_slice = {
                    let start_byte = if *byte_idx < 0 {
                        (-*byte_idx) as usize
                    } else {
                        0
                    };
                    let end_byte = text.len().min((total_bytes as isize - *byte_idx) as usize);
                    &text[start_byte..end_byte]
                };

                // Return the text.
                return Some(text_slice);
            }

            Chunks(ChunksEnum::Light {
                text,
                ref mut is_end,
            }) => {
                if !*is_end {
                    return None;
                } else {
                    *is_end = false;
                    return Some(text);
                }
            }
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        match *self {
            Chunks(ChunksEnum::Full {
                ref mut node_stack,
                total_bytes,
                ref mut byte_idx,
            }) => {
                if *byte_idx >= total_bytes as isize {
                    return None;
                }

                // Progress the node stack if needed.
                let mut stack_idx = node_stack.len() - 1;
                if node_stack[stack_idx].1 >= node_stack[stack_idx].0.child_count() {
                    while node_stack[stack_idx].1 >= (node_stack[stack_idx].0.child_count() - 1) {
                        if stack_idx == 0 {
                            return None;
                        } else {
                            stack_idx -= 1;
                        }
                    }
                    node_stack[stack_idx].1 += 1;
                    while stack_idx < (node_stack.len() - 1) {
                        let child_i = node_stack[stack_idx].1;
                        let node = &node_stack[stack_idx].0.children().nodes()[child_i];
                        node_stack[stack_idx + 1] = (node, 0);
                        stack_idx += 1;
                    }
                }

                // Fetch the node and child index.
                let (node, ref mut child_i) = node_stack.last_mut().unwrap();

                // Get the text, sliced to the appropriate range.
                let text = node.children().nodes()[*child_i].leaf_text();
                let text_slice = {
                    let start_byte = if *byte_idx < 0 {
                        (-*byte_idx) as usize
                    } else {
                        0
                    };
                    let end_byte = text.len().min((total_bytes as isize - *byte_idx) as usize);
                    &text[start_byte..end_byte]
                };

                // Book keeping.
                *byte_idx += text.len() as isize;
                *child_i += 1;

                // Return the text.
                return Some(text_slice);
            }

            Chunks(ChunksEnum::Light {
                text,
                ref mut is_end,
            }) => {
                if *is_end {
                    return None;
                } else {
                    *is_end = true;
                    return Some(text);
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
    fn chunks_02() {
        let r = Rope::from_str("");
        let mut itr = r.chunks();

        assert_eq!(Some(""), itr.next());
        assert_eq!(None, itr.next());
    }

    #[test]
    fn chunks_03() {
        let r = Rope::from_str(TEXT);

        let mut itr = r.chunks();

        assert_eq!(None, itr.prev());
    }

    #[test]
    fn chunks_04() {
        let r = Rope::from_str(TEXT);

        let mut chunks = Vec::new();
        let mut itr = r.chunks();

        while let Some(text) = itr.next() {
            chunks.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, chunks.pop().unwrap());
        }

        assert!(chunks.is_empty());
    }

    #[test]
    fn chunks_at_char_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..r.len_chars() {
            let (chunk, b, c, l) = r.chunk_at_char(i);
            let (mut chunks, bs, cs, ls) = r.chunks_at_char(i);

            assert_eq!(b, bs);
            assert_eq!(c, cs);
            assert_eq!(l, ls);
            assert_eq!(Some(chunk), chunks.next());
        }
    }

    #[test]
    fn chunks_at_char_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(34..301);

        for i in 0..s.len_chars() {
            let (chunk, b, c, l) = s.chunk_at_char(i);
            let (mut chunks, bs, cs, ls) = s.chunks_at_char(i);

            assert_eq!(b, bs);
            assert_eq!(c, cs);
            assert_eq!(l, ls);
            assert_eq!(Some(chunk), chunks.next());
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
