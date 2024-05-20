use crate::tree::Node;

//=============================================================

#[derive(Debug, Clone)]
pub struct Chunks<'a> {
    node_stack: Vec<(&'a Node, usize)>, // (node ref, index of current child)
    byte_range: [usize; 2],
    current_byte_idx: usize,
    at_start: bool,
}

impl<'a> Chunks<'a> {
    /// Returns the Chunks iterator as well as the actual start byte of the
    /// first chunk, from the start of Node's contents.
    #[inline(always)]
    pub(crate) fn new(
        node: &Node,
        byte_range: [usize; 2],
        mut at_byte_idx: usize,
    ) -> (Chunks, usize) {
        debug_assert!(byte_range[0] <= at_byte_idx && at_byte_idx <= byte_range[1]);

        // Special case: if it's an empty rope, don't store anything.
        if byte_range[0] == byte_range[1] || node.is_empty() {
            return (
                Chunks {
                    node_stack: vec![],
                    byte_range: [0, 0],
                    current_byte_idx: 0,
                    at_start: true,
                },
                0,
            );
        }

        let mut chunks = Chunks {
            node_stack: vec![],
            byte_range: byte_range,
            current_byte_idx: 0,
            at_start: true,
        };

        // TODO: make this work properly for arbitrary start indices. In
        // particular, handling how the `at_start` flag, etc. work. Easiest
        // might be to find the spot naively, and just walk the iterator back
        // one step from there.
        let mut current_node = node;
        let mut byte_offset = 0;
        loop {
            match *current_node {
                Node::Leaf(_) => {
                    chunks.node_stack.push((current_node, 0));
                    chunks.current_byte_idx = byte_offset;
                    break;
                }

                Node::Internal(ref children) => {
                    let (child_i, acc_byte_idx) = children.search_byte_idx_only(at_byte_idx);

                    byte_offset += acc_byte_idx;
                    at_byte_idx -= acc_byte_idx;

                    chunks.node_stack.push((current_node, child_i));
                    current_node = &children.nodes()[child_i];
                }
            }
        }

        (chunks, byte_offset.max(byte_range[0]))
    }

    #[inline(always)]
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
            self.at_start = true;
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
        if self.at_start {
            self.at_start = false;
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
    current_chunk: Option<std::slice::Iter<'a, u8>>,
}

impl<'a> Bytes<'a> {
    #[inline(always)]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> Bytes {
        let (mut chunks, byte_start) = Chunks::new(node, byte_range, at_byte_idx);
        let first_chunk = chunks
            .next()
            .map(|text| text.as_bytes()[(at_byte_idx - byte_start)..].iter());

        Bytes {
            chunks: chunks,
            current_chunk: first_chunk,
        }
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<u8> {
        loop {
            if self.current_chunk.is_none() {
                return None;
            }

            if let Some(byte) = self.current_chunk.as_mut().unwrap().next() {
                return Some(*byte);
            }

            self.current_chunk = self.chunks.next().map(|text| text.as_bytes().iter());
        }
    }
}

//=============================================================

#[derive(Debug, Clone)]
pub struct Chars<'a> {
    chunks: Chunks<'a>,
    current_chunk: Option<std::str::Chars<'a>>,
}

impl<'a> Chars<'a> {
    #[inline(always)]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> Chars {
        let (mut chunks, byte_start) = Chunks::new(node, byte_range, at_byte_idx);
        let first_chunk = chunks
            .next()
            .map(|text| text[(at_byte_idx - byte_start)..].chars());

        Chars {
            chunks: chunks,
            current_chunk: first_chunk,
        }
    }
}

impl<'a> Iterator for Chars<'a> {
    type Item = char;

    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    fn next(&mut self) -> Option<char> {
        loop {
            if self.current_chunk.is_none() {
                return None;
            }

            if let Some(char) = self.current_chunk.as_mut().unwrap().next() {
                return Some(char);
            }

            self.current_chunk = self.chunks.next().map(|text| text.chars());
        }
    }
}

//=============================================================

#[cfg(test)]
mod tests {
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
    fn chunks_iter_02() {
        let r = Rope::from_str("");

        let mut chunks = r.chunks();
        assert_eq!(None, chunks.next());
        assert_eq!(None, chunks.prev());
    }

    #[test]
    fn chunks_iter_03() {
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
    fn chunks_iter_04() {
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
    fn chunks_iter_05() {
        let r = hello_world_repeat_rope();
        let s = r.slice(14..14);

        let mut chunks = s.chunks();
        assert_eq!(None, chunks.next());
        assert_eq!(None, chunks.prev());
    }
    #[test]
    fn chunks_iter_06() {
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
    fn chunks_iter_07() {
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

    // NOTE: when you add support for starting iterators at specific indices,
    // ensure that the Bytes iterator can be created with a starting index that
    // splits a char.

    #[test]
    fn bytes_iter_01() {
        let r = Rope::from_str(TEXT);

        let mut iter1 = TEXT.bytes();
        let mut iter2 = r.bytes();

        loop {
            let b1 = iter1.next();
            let b2 = iter2.next();

            assert_eq!(b1, b2);

            if b1.is_none() && b2.is_none() {
                break;
            }
        }
    }

    #[test]
    fn bytes_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);

        let mut iter1 = TEXT[5..124].bytes();
        let mut iter2 = s.bytes();

        loop {
            let b1 = iter1.next();
            let b2 = iter2.next();

            assert_eq!(b1, b2);

            if b1.is_none() && b2.is_none() {
                break;
            }
        }
    }

    #[test]
    fn bytes_iter_03() {
        let r = Rope::from_str("");

        assert_eq!(None, r.bytes().next());
    }

    #[test]
    fn chars_iter_01() {
        let r = Rope::from_str(TEXT);

        let mut iter1 = TEXT.chars();
        let mut iter2 = r.chars();

        loop {
            let b1 = iter1.next();
            let b2 = iter2.next();

            assert_eq!(b1, b2);

            if b1.is_none() && b2.is_none() {
                break;
            }
        }
    }

    #[test]
    fn chars_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);

        let mut iter1 = TEXT[5..124].chars();
        let mut iter2 = s.chars();

        loop {
            let b1 = iter1.next();
            let b2 = iter2.next();

            assert_eq!(b1, b2);

            if b1.is_none() && b2.is_none() {
                break;
            }
        }
    }

    #[test]
    fn chars_iter_03() {
        let r = Rope::from_str("");

        assert_eq!(None, r.chars().next());
    }
}
