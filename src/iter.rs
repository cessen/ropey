use crate::tree::Node;

//=============================================================

#[derive(Debug, Clone)]
pub struct Chunks<'a> {
    node_stack: Vec<(&'a Node, usize)>, // (node ref, index of current child)
    byte_range: [usize; 2],
    current_byte_idx: usize,
    at_start_sentinel: bool,
}

impl<'a> Chunks<'a> {
    /// Returns the Chunks iterator as well as the actual start byte of the
    /// first chunk, from the start of Node's contents.
    ///
    /// Note that all parameters are relative to the entire contents of `node`.
    /// In particular, `at_byte_idx` is NOT relative to `byte_range`, it is an
    /// offset from the start of the full contents of `node`.
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> (Chunks, usize) {
        debug_assert!(byte_range[0] <= at_byte_idx && at_byte_idx <= byte_range[1]);

        // Special case: if it's an empty rope, don't store anything.
        if byte_range[0] == byte_range[1] || node.is_empty() {
            return (
                Chunks {
                    node_stack: vec![],
                    byte_range: [0, 0],
                    current_byte_idx: 0,
                    at_start_sentinel: true,
                },
                0,
            );
        }

        let mut chunks = Chunks {
            node_stack: vec![],
            byte_range: byte_range,
            current_byte_idx: 0,
            at_start_sentinel: false,
        };

        // Find the chunk the contains `at_byte_idx` and set that as the current
        // chunk of the iterator.
        let mut current_node = node;
        let mut local_byte_idx = at_byte_idx;
        loop {
            match *current_node {
                Node::Leaf(ref text) => {
                    if at_byte_idx >= byte_range[1] {
                        chunks.current_byte_idx += text.len();
                    }
                    chunks.node_stack.push((current_node, 0));
                    break;
                }

                Node::Internal(ref children) => {
                    let (child_i, acc_byte_idx) = children.search_byte_idx_only(local_byte_idx);

                    chunks.current_byte_idx += acc_byte_idx;
                    local_byte_idx -= acc_byte_idx;

                    chunks.node_stack.push((current_node, child_i));
                    current_node = &children.nodes()[child_i];
                }
            }
        }

        let byte_offset = chunks.current_byte_idx;

        // Take one step back so that `.next()` will return the chunk that
        // we found.
        chunks.prev();

        (chunks, byte_offset.max(byte_range[0]).min(byte_range[1]))
    }

    fn current_chunk(&self) -> Option<&'a str> {
        if self.current_byte_idx >= self.byte_range[1] {
            return None;
        }

        let text = self.node_stack.last().unwrap().0.leaf_text();
        let trimmed_chunk = {
            let mut chunk = text.text();
            if (self.current_byte_idx + chunk.len()) > self.byte_range[1] {
                chunk = &chunk[..(self.byte_range[1] - self.current_byte_idx)];
            }
            if self.current_byte_idx < self.byte_range[0] {
                chunk = &chunk[(self.byte_range[0] - self.current_byte_idx)..];
            }
            chunk
        };

        Some(trimmed_chunk)
    }

    /// Advances the iterator backward and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    pub fn prev(&mut self) -> Option<&'a str> {
        // Already at the start, or it's an empty rope.
        if self.current_byte_idx <= self.byte_range[0] || self.node_stack.is_empty() {
            self.at_start_sentinel = true;
            return None;
        }

        // Just getting started at the end.
        if self.current_byte_idx >= self.byte_range[1] {
            self.current_byte_idx -= self.node_stack.last().unwrap().0.leaf_text().len();
            return self.current_chunk();
        }

        // Progress the stack backwards.
        if self.node_stack.len() > 1 {
            // Pop the leaf.
            self.node_stack.pop();

            // Find the deepest node that's not at its start already.
            while self.node_stack.last().unwrap().1 == 0 {
                debug_assert!(self.node_stack.len() > 1);
                self.node_stack.pop();
            }

            // Refill the stack starting from that node.
            self.node_stack.last_mut().unwrap().1 -= 1;
            while self.node_stack.last().unwrap().0.is_internal() {
                let child_i = self.node_stack.last().unwrap().1;
                let node = &self.node_stack.last().unwrap().0.children().nodes()[child_i];
                let position = match *node {
                    Node::Leaf(_) => 0,
                    Node::Internal(ref children) => children.len() - 1,
                };
                self.node_stack.push((node, position));
            }
        }

        self.current_byte_idx -= self.node_stack.last().unwrap().0.leaf_text().len();

        // Finally, return the chunk text.
        self.current_chunk()
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    fn next(&mut self) -> Option<&'a str> {
        // Already at the end, or it's an empty rope.
        if self.current_byte_idx >= self.byte_range[1] || self.node_stack.is_empty() {
            return None;
        }

        // Just starting.
        if self.at_start_sentinel {
            self.at_start_sentinel = false;
            return self.current_chunk();
        }

        self.current_byte_idx += self.node_stack.last().unwrap().0.leaf_text().len();

        // Progress the stack.
        if self.current_byte_idx < self.byte_range[1] && self.node_stack.len() > 1 {
            // Pop the leaf.
            self.node_stack.pop();

            // Find the deepest node that's not at its end already.
            while self.node_stack.last().unwrap().1
                >= (self.node_stack.last().unwrap().0.child_count() - 1)
            {
                debug_assert!(self.node_stack.len() > 1);
                self.node_stack.pop();
            }

            // Refill the stack starting from that node.
            self.node_stack.last_mut().unwrap().1 += 1;
            while self.node_stack.last().unwrap().0.is_internal() {
                let child_i = self.node_stack.last().unwrap().1;
                let node = &self.node_stack.last().unwrap().0.children().nodes()[child_i];
                self.node_stack.push((node, 0));
            }
        }

        // Finally, return the chunk text.
        self.current_chunk()
    }
}

//=============================================================

#[derive(Debug, Clone)]
pub struct Bytes<'a> {
    chunks: Chunks<'a>,
    current_chunk: &'a [u8],
    byte_idx_in_chunk: usize,
}

impl<'a> Bytes<'a> {
    #[inline(always)]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> Bytes {
        let (mut chunks, byte_start) = Chunks::new(node, byte_range, at_byte_idx);
        let first_chunk = chunks.next().unwrap_or("");

        Bytes {
            chunks: chunks,
            current_chunk: first_chunk.as_bytes(),
            byte_idx_in_chunk: at_byte_idx - byte_start,
        }
    }

    #[inline]
    pub fn prev(&mut self) -> Option<u8> {
        while self.byte_idx_in_chunk == 0 {
            if let Some(chunk) = self.chunks.prev() {
                self.current_chunk = chunk.as_bytes();
                self.byte_idx_in_chunk = chunk.len();
            } else {
                return None;
            }
        }

        self.byte_idx_in_chunk -= 1;
        Some(self.current_chunk[self.byte_idx_in_chunk])
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline]
    fn next(&mut self) -> Option<u8> {
        while self.byte_idx_in_chunk >= self.current_chunk.len() {
            if let Some(chunk) = self.chunks.next() {
                self.current_chunk = chunk.as_bytes();
                self.byte_idx_in_chunk = 0;
            } else {
                self.current_chunk = &[];
                self.byte_idx_in_chunk = 0;
                return None;
            }
        }

        let byte = self.current_chunk[self.byte_idx_in_chunk];
        self.byte_idx_in_chunk += 1;
        Some(byte)
    }
}

//=============================================================

#[derive(Debug, Clone)]
pub struct Chars<'a> {
    chunks: Chunks<'a>,
    current_chunk: &'a str,
    byte_idx_in_chunk: usize,
}

impl<'a> Chars<'a> {
    #[inline(always)]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> Chars {
        let (mut chunks, byte_start) = Chunks::new(node, byte_range, at_byte_idx);
        let first_chunk = chunks.next().unwrap_or("");

        assert!(first_chunk.is_char_boundary(at_byte_idx - byte_start));

        Chars {
            chunks: chunks,
            current_chunk: first_chunk,
            byte_idx_in_chunk: at_byte_idx - byte_start,
        }
    }

    #[inline]
    pub fn prev(&mut self) -> Option<char> {
        while self.byte_idx_in_chunk == 0 {
            if let Some(chunk) = self.chunks.prev() {
                self.current_chunk = chunk;
                self.byte_idx_in_chunk = chunk.len();
            } else {
                return None;
            }
        }

        self.byte_idx_in_chunk -= 1;
        while !self.current_chunk.is_char_boundary(self.byte_idx_in_chunk) {
            self.byte_idx_in_chunk -= 1;
        }
        // TODO: do this in a more efficient way than constructing a temporary
        // iterator.
        let char = self.current_chunk[self.byte_idx_in_chunk..]
            .chars()
            .next()
            .unwrap();
        Some(char)
    }
}

impl<'a> Iterator for Chars<'a> {
    type Item = char;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline]
    fn next(&mut self) -> Option<char> {
        while self.byte_idx_in_chunk >= self.current_chunk.len() {
            if let Some(chunk) = self.chunks.next() {
                self.current_chunk = chunk;
                self.byte_idx_in_chunk = 0;
            } else {
                self.current_chunk = "";
                self.byte_idx_in_chunk = 0;
                return None;
            }
        }

        // TODO: do this in a more efficient way than constructing a temporary
        // iterator and then also redundantly recomputing its utf8 length which
        // the internals of that temporary iterator clearly already know.
        let char = self.current_chunk[self.byte_idx_in_chunk..]
            .chars()
            .next()
            .unwrap();
        self.byte_idx_in_chunk += char.len_utf8();
        Some(char)
    }
}

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{rope_builder::RopeBuilder, Rope};

    // 127 bytes, 103 chars, 1 line
    const TEXT: &str = "Hello there!  How're you doing?  It's \
                        a fine day, isn't it?  Aren't you glad \
                        we're alive?  こんにちは、みんなさん！";

    fn hello_world_repeat_rope() -> Rope {
        let mut rb = RopeBuilder::new();
        for _ in 0..4 {
            rb._append_chunk_as_leaf("Hello ");
            rb._append_chunk_as_leaf("world!");
        }
        rb.finish()
    }

    /// Note: ensures that the chunks as given become individual leaf nodes in
    /// the rope.
    fn make_rope_from_chunks(chunks: &[&str]) -> Rope {
        let mut rb = RopeBuilder::new();
        for chunk in chunks {
            rb._append_chunk_as_leaf(chunk);
        }
        rb.finish()
    }

    #[test]
    fn chunks_iter_01() {
        let r = Rope::from_str(TEXT);

        let mut text = TEXT;
        let mut chunks = r.chunks();
        let mut stack = Vec::new();

        // Forward.
        while let Some(chunk) = chunks.next() {
            assert_eq!(&text[..chunk.len()], chunk);
            stack.push(chunk);
            text = &text[chunk.len()..];
        }
        assert_eq!("", text);

        // Backward.
        while let Some(chunk) = chunks.prev() {
            assert_eq!(stack.pop().unwrap(), chunk);
        }
        assert_eq!(0, stack.len());
    }

    #[test]
    fn chunks_iter_02() {
        let r = hello_world_repeat_rope();

        let mut chunks = r.chunks();

        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(None, chunks.next());

        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_03() {
        let r = Rope::from_str("");

        let mut chunks = r.chunks();
        assert_eq!(None, chunks.next());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_04() {
        let r = hello_world_repeat_rope();
        let s = r.slice(3..45);

        let mut chunks = s.chunks();

        assert_eq!(Some("lo "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("wor"), chunks.next());
        assert_eq!(None, chunks.next());

        assert_eq!(Some("wor"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("lo "), chunks.prev());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_05() {
        let r = hello_world_repeat_rope();
        let s = r.slice(8..40);

        let mut chunks = s.chunks();

        assert_eq!(Some("rld!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(Some("Hell"), chunks.next());
        assert_eq!(None, chunks.next());

        assert_eq!(Some("Hell"), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("world!"), chunks.prev());
        assert_eq!(Some("Hello "), chunks.prev());
        assert_eq!(Some("rld!"), chunks.prev());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_06() {
        let r = hello_world_repeat_rope();
        let s = r.slice(14..14);

        let mut chunks = s.chunks();
        assert_eq!(None, chunks.next());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_07() {
        let r = Rope::from_str("A");
        let mut chunks = r.chunks();

        assert_eq!(Some("A"), chunks.next());
        assert_eq!(None, chunks.prev());

        assert_eq!(Some("A"), chunks.next());
        assert_eq!(None, chunks.next());
        assert_eq!(Some("A"), chunks.prev());
        assert_eq!(None, chunks.prev());

        assert_eq!(Some("A"), chunks.next());
    }

    #[test]
    fn chunks_iter_08() {
        let r =
            make_rope_from_chunks(&["ABC", "DEF", "GHI", "JKL", "MNO", "PQR", "STU", "VWX", "YZ"]);
        let mut chunks = r.chunks();

        assert_eq!(Some("ABC"), chunks.next());
        assert_eq!(None, chunks.prev());

        assert_eq!(Some("ABC"), chunks.next());
        assert_eq!(Some("DEF"), chunks.next());
        assert_eq!(Some("ABC"), chunks.prev());

        assert_eq!(Some("DEF"), chunks.next());
        assert_eq!(Some("GHI"), chunks.next());
        assert_eq!(Some("JKL"), chunks.next());
        assert_eq!(Some("GHI"), chunks.prev());

        assert_eq!(Some("JKL"), chunks.next());
        assert_eq!(Some("MNO"), chunks.next());
        assert_eq!(Some("PQR"), chunks.next());
        assert_eq!(Some("STU"), chunks.next());
        assert_eq!(Some("VWX"), chunks.next());
        assert_eq!(Some("STU"), chunks.prev());

        assert_eq!(Some("VWX"), chunks.next());
        assert_eq!(Some("YZ"), chunks.next());
        assert_eq!(None, chunks.next());
        assert_eq!(Some("YZ"), chunks.prev());

        assert_eq!(None, chunks.next());
    }

    #[test]
    fn chunks_at_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..TEXT.len() {
            let mut current_byte = r.chunk_at_byte(i).1.bytes;

            for chunk1 in r.chunks_at(i) {
                let chunk2 = r.chunk_at_byte(current_byte).0;
                assert_eq!(chunk2, chunk1);
                current_byte += chunk2.len();
            }
        }

        let mut chunks = r.chunks_at(TEXT.len());
        assert_eq!(None, chunks.next());
    }

    #[test]
    fn chunks_at_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let text = &TEXT[5..124];

        for i in 0..text.len() {
            let mut current_byte = s.chunk_at_byte(i).1.bytes;

            for chunk1 in s.chunks_at(i) {
                let chunk2 = s.chunk_at_byte(current_byte).0;
                assert_eq!(chunk2, chunk1);
                current_byte += chunk2.len();
            }
        }

        let mut chunks = s.chunks_at(text.len());
        assert_eq!(None, chunks.next());
    }

    // NOTE: when you add support for starting iterators at specific indices,
    // ensure that the Bytes iterator can be created with a starting index that
    // splits a char.

    fn test_bytes_against_text(mut bytes: Bytes, text: &str) {
        // Forward.
        let mut iter_f = text.bytes();
        loop {
            let b1 = bytes.next();
            let b2 = iter_f.next();

            assert_eq!(b1, b2);

            if b1.is_none() && b2.is_none() {
                break;
            }
        }

        // Backward.
        let mut iter_b = text.bytes().rev();
        loop {
            let b1 = bytes.prev();
            let b2 = iter_b.next();

            assert_eq!(b1, b2);

            if b1.is_none() && b2.is_none() {
                break;
            }
        }
    }

    #[test]
    fn bytes_iter_01() {
        let r = Rope::from_str(TEXT);
        let mut iter = r.bytes();

        test_bytes_against_text(iter, TEXT);
    }

    #[test]
    fn bytes_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let mut iter = s.bytes();

        test_bytes_against_text(iter, &TEXT[5..124]);
    }

    #[test]
    fn bytes_iter_03() {
        let r = Rope::from_str("");

        assert_eq!(None, r.bytes().next());
        assert_eq!(None, r.bytes().prev());
    }

    #[test]
    fn bytes_at_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..TEXT.len() {
            let mut bytes = r.bytes_at(i);
            assert_eq!(TEXT.as_bytes()[i], bytes.next().unwrap());
        }

        let mut bytes = r.bytes_at(TEXT.len());
        assert_eq!(None, bytes.next());
    }

    #[test]
    fn bytes_at_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let text = &TEXT[5..124];

        for i in 0..text.len() {
            let mut bytes = s.bytes_at(i);
            assert_eq!(text.as_bytes()[i], bytes.next().unwrap());
        }

        let mut bytes = s.bytes_at(text.len());
        assert_eq!(None, bytes.next());
    }

    fn test_chars_against_text(mut chars: Chars, text: &str) {
        // Forward.
        let mut iter_f = text.chars();
        loop {
            let c1 = chars.next();
            let c2 = iter_f.next();

            assert_eq!(c1, c2);

            if c1.is_none() && c2.is_none() {
                break;
            }
        }

        // Backward.
        let mut iter_b = text.chars().rev();
        loop {
            let c1 = chars.prev();
            let c2 = iter_b.next();

            assert_eq!(c1, c2);

            if c1.is_none() && c2.is_none() {
                break;
            }
        }
    }

    #[test]
    fn chars_iter_01() {
        let r = Rope::from_str(TEXT);
        let mut iter = r.chars();

        test_chars_against_text(iter, TEXT);
    }

    #[test]
    fn chars_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let mut iter = s.chars();

        test_chars_against_text(iter, &TEXT[5..124]);
    }

    #[test]
    fn chars_iter_03() {
        let r = Rope::from_str("");

        assert_eq!(None, r.chars().next());
        assert_eq!(None, r.chars().prev());
    }

    #[test]
    fn chars_at_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..TEXT.len() {
            if !TEXT.is_char_boundary(i) {
                continue;
            }
            let mut chars = r.chars_at(i);
            assert_eq!(TEXT[i..].chars().next(), chars.next());
        }

        let mut chars = r.chars_at(TEXT.len());
        assert_eq!(None, chars.next());
    }

    #[test]
    fn chars_at_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let text = &TEXT[5..124];

        for i in 0..text.len() {
            if !text.is_char_boundary(i) {
                continue;
            }
            let mut chars = s.chars_at(i);
            assert_eq!(text[i..].chars().next(), chars.next());
        }

        let mut chars = s.chars_at(text.len());
        assert_eq!(None, chars.next());
    }
}
