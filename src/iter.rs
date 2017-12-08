#![allow(dead_code)]

use std::str::{Bytes, Chars};

use node::Node;
use slice::RopeSlice;

//==========================================================

/// An iterator over a Rope's bytes.
pub struct RopeBytes<'a> {
    chunk_iter: RopeChunks<'a>,
    cur_chunk: Bytes<'a>,
}

impl<'a> RopeBytes<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeBytes<'b> {
        RopeBytes {
            chunk_iter: RopeChunks::new(node),
            cur_chunk: "".bytes(),
        }
    }

    pub(crate) fn new_with_range<'b>(
        node: &'b Node,
        start_char: usize,
        end_char: usize,
    ) -> RopeBytes<'b> {
        RopeBytes {
            chunk_iter: RopeChunks::new_with_range(node, start_char, end_char),
            cur_chunk: "".bytes(),
        }
    }
}

impl<'a> Iterator for RopeBytes<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        loop {
            if let Some(c) = self.cur_chunk.next() {
                return Some(c);
            } else {
                if let Some(chunk) = self.chunk_iter.next() {
                    self.cur_chunk = chunk.bytes();
                    continue;
                } else {
                    return None;
                }
            }
        }
    }
}

//==========================================================

/// An iterator over a Rope's chars.
pub struct RopeChars<'a> {
    chunk_iter: RopeChunks<'a>,
    cur_chunk: Chars<'a>,
}

impl<'a> RopeChars<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeChars<'b> {
        RopeChars {
            chunk_iter: RopeChunks::new(node),
            cur_chunk: "".chars(),
        }
    }

    pub(crate) fn new_with_range<'b>(
        node: &'b Node,
        start_char: usize,
        end_char: usize,
    ) -> RopeChars<'b> {
        RopeChars {
            chunk_iter: RopeChunks::new_with_range(node, start_char, end_char),
            cur_chunk: "".chars(),
        }
    }
}

impl<'a> Iterator for RopeChars<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        loop {
            if let Some(c) = self.cur_chunk.next() {
                return Some(c);
            } else {
                if let Some(chunk) = self.chunk_iter.next() {
                    self.cur_chunk = chunk.chars();
                    continue;
                } else {
                    return None;
                }
            }
        }
    }
}

//==========================================================

/// An iterator over a Rope's chars.
pub struct RopeLines<'a> {
    node: &'a Node,
    start_char: usize,
    end_char: usize,
    line_idx: usize,
}

impl<'a> RopeLines<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeLines<'b> {
        RopeLines {
            node: node,
            start_char: 0,
            end_char: node.text_info().chars as usize,
            line_idx: 0,
        }
    }

    pub(crate) fn new_with_range<'b>(
        node: &'b Node,
        start_char: usize,
        end_char: usize,
    ) -> RopeLines<'b> {
        RopeLines {
            node: node,
            start_char: start_char,
            end_char: end_char,
            line_idx: node.char_to_line(start_char),
        }
    }
}

impl<'a> Iterator for RopeLines<'a> {
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

            return Some(self.node.slice(a, b));
        }
    }
}

//==========================================================

/// An iterator over a Rope's contiguous str chunks.
pub struct RopeChunks<'a> {
    node_stack: Vec<&'a Node>,
    start: usize,
    end: usize,
    idx: usize,
}

impl<'a> RopeChunks<'a> {
    pub(crate) fn new<'b>(node: &'b Node) -> RopeChunks<'b> {
        RopeChunks {
            node_stack: vec![node],
            start: 0,
            end: node.text_info().bytes as usize,
            idx: 0,
        }
    }

    pub(crate) fn new_with_range<'b>(
        node: &'b Node,
        start_char: usize,
        end_char: usize,
    ) -> RopeChunks<'b> {
        RopeChunks {
            node_stack: vec![node],
            start: node.char_to_byte(start_char),
            end: node.char_to_byte(end_char),
            idx: 0,
        }
    }
}

impl<'a> Iterator for RopeChunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.idx >= self.end {
            return None;
        }

        loop {
            match self.node_stack.pop() {
                Some(&Node::Leaf(ref text)) => {
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

                Some(&Node::Internal {
                         ref info,
                         ref children,
                     }) => {
                    // Find the first child that isn't before `self.start`,
                    // updating `self.idx` as we go.
                    let mut child_i = 0;
                    for inf in info.iter() {
                        if (self.idx + inf.bytes as usize) > self.start {
                            break;
                        } else {
                            self.idx += inf.bytes as usize;
                            child_i += 1;
                        }
                    }
                    // Push relevant children to the stack.
                    for child in (&children[child_i..]).iter().rev() {
                        self.node_stack.push(child);
                    }
                }

                _ => {
                    return None;
                }
            }
        }
    }
}

//===========================================================

#[cfg(test)]
mod tests {
    use rope::Rope;
    use slice::RopeSlice;

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
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

        for (br, bt) in r.bytes().zip(TEXT.bytes()) {
            assert_eq!(br, bt);
        }
    }

    #[test]
    fn chars_01() {
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

        for (cr, ct) in r.chars().zip(TEXT.chars()) {
            assert_eq!(cr, ct);
        }
    }

    #[test]
    fn lines_01() {
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

        assert_eq!(34, r.lines().count());

        let mut lines = r.lines();

        assert_eq!("\r\n", &lines.next().unwrap().to_string());

        for _ in 0..16 {
            assert_eq!(
                "Hello there!  How're you doing?  It's a fine day, \
                 isn't it?  Aren't you glad we're alive?\r\n",
                &lines.next().unwrap().to_string()
            );
            assert_eq!(
                "こんにちは！元気ですか？日はいいですね。\
                 私たちが生きだって嬉しいではないか？\r\n",
                &lines.next().unwrap().to_string()
            );
        }

        assert_eq!("", &lines.next().unwrap().to_string());
        assert!(lines.next().is_none());
    }

    #[test]
    fn lines_02() {
        let text = "Hello there!\nHow goes it?";
        let mut r = Rope::new();

        for c in text.chars().rev() {
            r.insert(0, &c.to_string());
        }

        assert_eq!(2, r.lines().count());

        let mut lines = r.lines();
        assert_eq!("Hello there!\n", &lines.next().unwrap().to_string());
        assert_eq!("How goes it?", &lines.next().unwrap().to_string());
        assert!(lines.next().is_none());
    }

    #[test]
    fn chunks_01() {
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

        let mut idx = 0;
        for chunk in r.chunks() {
            assert_eq!(chunk, &TEXT[idx..(idx + chunk.len())]);
            idx += chunk.len();
        }
    }

    #[test]
    fn bytes_sliced_01() {
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

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
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

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
    fn lines_sliced_01() {
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

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
        let mut r = Rope::new();

        for c in TEXT.chars().rev() {
            r.insert(0, &c.to_string());
        }

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
