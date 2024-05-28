use crate::tree::Node;

//=============================================================

#[derive(Debug, Clone)]
pub struct Chunks<'a> {
    node_stack: Vec<(&'a Node, usize)>, // (node ref, index of current child)

    // The byte range within the root node that is considered part of this
    // iterator's contents.
    byte_range: [usize; 2],

    // The offset within the root node (*not* `byte_range`) that of the current
    // un-trimmed chunk.
    current_byte_idx: usize,

    // An indicator that we are at the start of the iterator, before* the first
    // *chunk.  This is needed to distinguish e.g. `current_byte_idx == 0` from
    // *meaning we're on the first chunk vs before it.
    at_start_sentinel: bool,

    is_reversed: bool,
}

impl<'a> Chunks<'a> {
    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn next(&mut self) -> Option<&'a str> {
        if self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    /// Advances the iterator backward and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn prev(&mut self) -> Option<&'a str> {
        if self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    /// Reverses the direction of iteration.
    ///
    /// NOTE: this is distinct from the standard library's `rev()` method for
    /// `DoubleEndedIterator`.  Unlike that method, this reverses the direction
    /// of the iterator without changing its position in the stream.
    #[inline(always)]
    pub fn reversed(mut self) -> Chunks<'a> {
        self.is_reversed = !self.is_reversed;
        self
    }

    /// Returns the byte offset of the current chunk from the start of the text.
    #[inline]
    pub fn byte_offset(&self) -> usize {
        self.current_byte_idx
            .saturating_sub(self.byte_range[0])
            .min(self.byte_range[1] - self.byte_range[0])
    }

    //---------------------------------------------------------

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
                    is_reversed: false,
                },
                0,
            );
        }

        let mut chunks = Chunks {
            node_stack: vec![],
            byte_range: byte_range,
            current_byte_idx: 0,
            at_start_sentinel: false,
            is_reversed: false,
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

    fn next_impl(&mut self) -> Option<&'a str> {
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
            // Start at the node above the leaf.
            let mut stack_idx = self.node_stack.len() - 2;

            // Find the deepest node that's not at its end already.
            while self.node_stack[stack_idx].1 >= (self.node_stack[stack_idx].0.child_count() - 1) {
                debug_assert!(stack_idx > 0);
                stack_idx -= 1;
            }

            // Refill the stack starting from that node.
            self.node_stack[stack_idx].1 += 1;
            while self.node_stack[stack_idx].0.is_internal() {
                let child_i = self.node_stack[stack_idx].1;
                let node = &self.node_stack[stack_idx].0.children().nodes()[child_i];

                stack_idx += 1;
                self.node_stack[stack_idx] = (node, 0);
            }

            debug_assert!(stack_idx == self.node_stack.len() - 1);
        }

        // Finally, return the chunk text.
        self.current_chunk()
    }

    fn prev_impl(&mut self) -> Option<&'a str> {
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
            // Start at the node above the leaf.
            let mut stack_idx = self.node_stack.len() - 2;

            // Find the deepest node that's not at its start already.
            while self.node_stack[stack_idx].1 == 0 {
                debug_assert!(stack_idx > 0);
                stack_idx -= 1;
            }

            // Refill the stack starting from that node.
            self.node_stack[stack_idx].1 -= 1;
            while self.node_stack[stack_idx].0.is_internal() {
                let child_i = self.node_stack[stack_idx].1;
                let node = &self.node_stack[stack_idx].0.children().nodes()[child_i];
                let position = match *node {
                    Node::Leaf(_) => 0,
                    Node::Internal(ref children) => children.len() - 1,
                };

                stack_idx += 1;
                self.node_stack[stack_idx] = (node, position);
            }

            debug_assert!(stack_idx == self.node_stack.len() - 1);
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
    #[inline(always)]
    fn next(&mut self) -> Option<&'a str> {
        Chunks::next(self)
    }
}

//=============================================================

#[derive(Debug, Clone)]
pub struct Bytes<'a> {
    chunks: Chunks<'a>,
    current_chunk: &'a [u8],
    byte_idx_in_chunk: usize,
    is_reversed: bool,
}

impl<'a> Bytes<'a> {
    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn next(&mut self) -> Option<u8> {
        if self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    /// Advances the iterator backward and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn prev(&mut self) -> Option<u8> {
        if self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    /// Reverses the direction of iteration.
    ///
    /// NOTE: this is distinct from the standard library's `rev()` method for
    /// `DoubleEndedIterator`.  Unlike that method, this reverses the direction
    /// of the iterator without changing its position in the stream.
    #[inline(always)]
    pub fn reversed(mut self) -> Bytes<'a> {
        self.is_reversed = !self.is_reversed;
        self
    }

    //---------------------------------------------------------

    #[inline(always)]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> Bytes {
        let (mut chunks, byte_start) = Chunks::new(node, byte_range, at_byte_idx);
        let first_chunk = chunks.next().unwrap_or("");

        Bytes {
            chunks: chunks,
            current_chunk: first_chunk.as_bytes(),
            byte_idx_in_chunk: at_byte_idx - byte_start,
            is_reversed: false,
        }
    }

    fn next_impl(&mut self) -> Option<u8> {
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

    fn prev_impl(&mut self) -> Option<u8> {
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
    #[inline(always)]
    fn next(&mut self) -> Option<u8> {
        Bytes::next(self)
    }
}

//=============================================================

#[derive(Debug, Clone)]
pub struct Chars<'a> {
    chunks: Chunks<'a>,
    current_chunk: &'a str,
    byte_idx_in_chunk: usize,
    is_reversed: bool,
}

impl<'a> Chars<'a> {
    /// Advances the iterator forward and returns the next value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn next(&mut self) -> Option<char> {
        if self.is_reversed {
            self.prev_impl()
        } else {
            self.next_impl()
        }
    }

    /// Advances the iterator backward and returns the previous value.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn prev(&mut self) -> Option<char> {
        if self.is_reversed {
            self.next_impl()
        } else {
            self.prev_impl()
        }
    }

    /// Reverses the direction of iteration.
    ///
    /// NOTE: this is distinct from the standard library's `rev()` method for
    /// `DoubleEndedIterator`.  Unlike that method, this reverses the direction
    /// of the iterator without changing its position in the stream.
    #[inline(always)]
    pub fn reversed(mut self) -> Chars<'a> {
        self.is_reversed = !self.is_reversed;
        self
    }

    //---------------------------------------------------------

    #[inline(always)]
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> Chars {
        let (mut chunks, byte_start) = Chunks::new(node, byte_range, at_byte_idx);
        let first_chunk = chunks.next().unwrap_or("");

        assert!(first_chunk.is_char_boundary(at_byte_idx - byte_start));

        Chars {
            chunks: chunks,
            current_chunk: first_chunk,
            byte_idx_in_chunk: at_byte_idx - byte_start,
            is_reversed: false,
        }
    }

    fn next_impl(&mut self) -> Option<char> {
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

    fn prev_impl(&mut self) -> Option<char> {
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
    #[inline(always)]
    fn next(&mut self) -> Option<char> {
        Chars::next(self)
    }
}

//=============================================================

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
mod lines {
    use crate::{
        tree::{Node, TextInfo},
        LineType, RopeSlice,
    };

    #[derive(Debug, Clone)]
    pub struct Lines<'a> {
        node: &'a Node,
        node_info: &'a TextInfo,
        byte_range: [usize; 2],
        line_range: [usize; 2],
        line_type: LineType,
        current_line_idx: usize,
        at_start_sentinel: bool,
        is_reversed: bool,
    }

    impl<'a> Lines<'a> {
        /// Advances the iterator forward and returns the next value.
        ///
        /// Runs in amortized O(1) time and worst-case O(log N) time.
        #[inline(always)]
        pub fn next(&mut self) -> Option<RopeSlice<'a>> {
            if self.is_reversed {
                self.prev_impl()
            } else {
                self.next_impl()
            }
        }

        /// Advances the iterator backward and returns the previous value.
        ///
        /// Runs in amortized O(1) time and worst-case O(log N) time.
        #[inline(always)]
        pub fn prev(&mut self) -> Option<RopeSlice<'a>> {
            if self.is_reversed {
                self.next_impl()
            } else {
                self.prev_impl()
            }
        }

        /// Reverses the direction of iteration.
        ///
        /// NOTE: this is distinct from the standard library's `rev()` method for
        /// `DoubleEndedIterator`.  Unlike that method, this reverses the direction
        /// of the iterator without changing its position in the stream.
        #[inline(always)]
        pub fn reversed(mut self) -> Lines<'a> {
            self.is_reversed = !self.is_reversed;
            self
        }

        //-----------------------------------------------------

        /// Note: unlike the other iterator constructors, this one takes
        /// `at_line_idx` relative to the slice defined by `byte_range`, not
        /// relative to the whole contents of `node`.
        pub(crate) fn new(
            node: &'a Node,
            node_info: &'a TextInfo,
            byte_range: [usize; 2],
            at_line_idx: usize,
            line_type: LineType,
        ) -> Lines<'a> {
            let start_line = {
                let (text, info) = node.get_text_at_byte(byte_range[0]);
                info.line_breaks(line_type)
                    + text.byte_to_line(byte_range[0] - info.bytes, line_type)
            };
            let end_line = {
                let (text, info) = node.get_text_at_byte(byte_range[1]);
                info.line_breaks(line_type)
                    + text.byte_to_line(byte_range[1] - info.bytes, line_type)
                    + 1
            };

            assert!(start_line + at_line_idx <= end_line);

            Lines {
                node: node,
                node_info: node_info,
                byte_range: byte_range,
                line_range: [start_line, end_line],
                line_type: line_type,
                current_line_idx: start_line + at_line_idx.saturating_sub(1),
                at_start_sentinel: at_line_idx == 0,
                is_reversed: false,
            }
        }

        fn current_line(&self) -> Option<RopeSlice<'a>> {
            if self.at_start_sentinel || self.current_line_idx >= self.line_range[1] {
                return None;
            }

            let start_byte = {
                let (text, start_info) = self
                    .node
                    .get_text_at_line_break(self.current_line_idx, self.line_type);

                start_info.bytes
                    + text.line_to_byte(
                        self.current_line_idx - start_info.line_breaks(self.line_type),
                        self.line_type,
                    )
            };
            let end_byte = {
                let (text, start_info) = self
                    .node
                    .get_text_at_line_break(self.current_line_idx + 1, self.line_type);

                start_info.bytes
                    + text.line_to_byte(
                        self.current_line_idx + 1 - start_info.line_breaks(self.line_type),
                        self.line_type,
                    )
            };

            Some(RopeSlice::new(
                self.node,
                self.node_info,
                [
                    start_byte.max(self.byte_range[0]),
                    end_byte.min(self.byte_range[1]),
                ],
            ))
        }

        fn next_impl(&mut self) -> Option<RopeSlice<'a>> {
            if self.current_line_idx >= self.line_range[1] {
                return None;
            }

            if !self.at_start_sentinel {
                self.current_line_idx += 1;
            } else {
                self.at_start_sentinel = false;
            }

            self.current_line()
        }

        fn prev_impl(&mut self) -> Option<RopeSlice<'a>> {
            if self.current_line_idx <= self.line_range[0] {
                self.at_start_sentinel = true;
                return None;
            }

            self.current_line_idx -= 1;
            self.current_line()
        }
    }

    impl<'a> Iterator for Lines<'a> {
        type Item = RopeSlice<'a>;

        /// Advances the iterator forward and returns the next value.
        ///
        /// Runs in amortized O(1) time and worst-case O(log N) time.
        #[inline(always)]
        fn next(&mut self) -> Option<RopeSlice<'a>> {
            Lines::next(self)
        }
    }
}

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_cr_lf",
    feature = "metric_lines_unicode"
))]
pub use lines::Lines;

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{rope_builder::RopeBuilder, Rope};

    #[cfg(feature = "metric_lines_cr_lf")]
    use crate::LineType;

    // 127 bytes, 103 chars, 1 line
    const TEXT: &str = "Hello there!  How're you doing?  It's \
                        a fine day, isn't it?  Aren't you glad \
                        we're alive?  こんにちは、みんなさん！";

    #[cfg(feature = "metric_lines_cr_lf")]
    fn lines_text() -> String {
        let mut text = String::new();
        text.push_str("\r\n");
        for _ in 0..16 {
            text.push_str(
                "Hello there!  How're you doing?  It's a fine day, \
                 isn't it?  Aren't you glad we're alive?\r\n\
                 こんにちは！元気ですか？日はいいですね。\
                 私たちが生きだって嬉しいではないか？\r\n",
            );
        }
        text
    }

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
        let mut byte_offset = 0;
        while let Some(chunk) = chunks.next() {
            assert_eq!(&text[..chunk.len()], chunk);
            assert_eq!(chunks.byte_offset(), byte_offset);
            stack.push(chunk);
            byte_offset += chunk.len();
            text = &text[chunk.len()..];
        }
        assert_eq!("", text);
        assert_eq!(chunks.byte_offset(), TEXT.len());

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
        assert_eq!(chunks.byte_offset(), 0);
        assert_eq!(None, chunks.next());
        assert_eq!(chunks.byte_offset(), 0);
        assert_eq!(None, chunks.prev());
        assert_eq!(chunks.byte_offset(), 0);
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

        assert_eq!(chunks.byte_offset(), 0);
        assert_eq!(Some("rld!"), chunks.next());
        assert_eq!(chunks.byte_offset(), 0);
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(chunks.byte_offset(), 4);
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(chunks.byte_offset(), 10);
        assert_eq!(Some("Hello "), chunks.next());
        assert_eq!(chunks.byte_offset(), 16);
        assert_eq!(Some("world!"), chunks.next());
        assert_eq!(chunks.byte_offset(), 22);
        assert_eq!(Some("Hell"), chunks.next());
        assert_eq!(chunks.byte_offset(), 28);
        assert_eq!(None, chunks.next());
        assert_eq!(chunks.byte_offset(), 32);

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
        assert_eq!(chunks.byte_offset(), 0);
        assert_eq!(None, chunks.next());
        assert_eq!(chunks.byte_offset(), 0);
        assert_eq!(None, chunks.prev());
        assert_eq!(chunks.byte_offset(), 0);
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
            let mut current_byte = r.chunk(i).1;

            for chunk1 in r.chunks_at(i) {
                let chunk2 = r.chunk(current_byte).0;
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
            let mut current_byte = s.chunk(i).1;

            for chunk1 in s.chunks_at(i) {
                let chunk2 = s.chunk(current_byte).0;
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
        let iter = r.bytes();

        test_bytes_against_text(iter, TEXT);
    }

    #[test]
    fn bytes_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let iter = s.bytes();

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
        let iter = r.chars();

        test_chars_against_text(iter, TEXT);
    }

    #[test]
    fn chars_iter_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let iter = s.chars();

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

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_01() {
        let r = Rope::from_str("hi\nyo\nbye");

        let mut lines = r.lines(LineType::CRLF);

        assert_eq!("hi\n", lines.next().unwrap());
        assert_eq!(None, lines.prev());

        assert_eq!("hi\n", lines.next().unwrap());
        assert_eq!("yo\n", lines.next().unwrap());
        assert_eq!("hi\n", lines.prev().unwrap());

        assert_eq!("yo\n", lines.next().unwrap());
        assert_eq!("bye", lines.next().unwrap());
        assert_eq!(None, lines.next());
        assert_eq!("bye", lines.prev().unwrap());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_02() {
        let text = "Hello there!\nHow goes it?";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines(LineType::CRLF).count());
        assert_eq!(2, s.lines(LineType::CRLF).count());

        let mut lines = r.lines(LineType::CRLF);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::CRLF);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_03() {
        let text = "Hello there!\nHow goes it?\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(3, r.lines(LineType::CRLF).count());
        assert_eq!(3, s.lines(LineType::CRLF).count());

        let mut lines = r.lines(LineType::CRLF);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::CRLF);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_04() {
        let text = "Hello there!\nHow goes it?\nYeah!";
        let r = Rope::from_str(text);
        let s1 = r.slice(..25);
        let s2 = r.slice(..26);

        assert_eq!(2, s1.lines(LineType::CRLF).count());
        assert_eq!(3, s2.lines(LineType::CRLF).count());

        let mut lines = s1.lines(LineType::CRLF);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s2.lines(LineType::CRLF);
        assert_eq!("Hello there!\n", lines.next().unwrap());
        assert_eq!("How goes it?\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_05() {
        let text = "";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(1, r.lines(LineType::CRLF).count());
        assert_eq!(1, s.lines(LineType::CRLF).count());

        let mut lines = r.lines(LineType::CRLF);
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::CRLF);
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_06() {
        let text = "a";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(1, r.lines(LineType::CRLF).count());
        assert_eq!(1, s.lines(LineType::CRLF).count());

        let mut lines = r.lines(LineType::CRLF);
        assert_eq!("a", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::CRLF);
        assert_eq!("a", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_07() {
        let text = "a\nb";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines(LineType::CRLF).count());
        assert_eq!(2, s.lines(LineType::CRLF).count());

        let mut lines = r.lines(LineType::CRLF);
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::CRLF);
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_08() {
        let text = "\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(2, r.lines(LineType::CRLF).count());
        assert_eq!(2, s.lines(LineType::CRLF).count());

        let mut lines = r.lines(LineType::CRLF);
        assert_eq!("\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::CRLF);
        assert_eq!("\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_09() {
        let text = "a\nb\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        assert_eq!(3, r.lines(LineType::CRLF).count());
        assert_eq!(3, s.lines(LineType::CRLF).count());

        let mut lines = r.lines(LineType::CRLF);
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());

        let mut lines = s.lines(LineType::CRLF);
        assert_eq!("a\n", lines.next().unwrap());
        assert_eq!("b\n", lines.next().unwrap());
        assert_eq!("", lines.next().unwrap());
        assert!(lines.next().is_none());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_10() {
        let text = lines_text();
        let r = Rope::from_str(&text);

        let mut itr = r.lines(LineType::CRLF);

        assert_eq!(None, itr.prev());
        assert_eq!(None, itr.prev());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_11() {
        let text = lines_text();
        let r = Rope::from_str(&text);

        let mut lines = Vec::new();
        let mut itr = r.lines(LineType::CRLF);

        while let Some(line) = itr.next() {
            lines.push(line);
        }

        while let Some(line) = itr.prev() {
            assert_eq!(line, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_12() {
        let text = lines_text();
        dbg!(text.len());
        let r = Rope::from_str(&text);
        let s = r.slice(34..2031);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::CRLF);

        while let Some(line) = itr.next() {
            lines.push(line);
        }

        while let Some(line) = itr.prev() {
            assert_eq!(line, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_13() {
        let text = "";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::CRLF);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_14() {
        let text = "a";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::CRLF);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_15() {
        let text = "a\nb";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::CRLF);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_16() {
        let text = "\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::CRLF);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_17() {
        let text = "a\nb\n";
        let r = Rope::from_str(text);
        let s = r.slice(..);

        let mut lines = Vec::new();
        let mut itr = s.lines(LineType::CRLF);

        while let Some(text) = itr.next() {
            lines.push(text);
        }

        while let Some(text) = itr.prev() {
            assert_eq!(text, lines.pop().unwrap());
        }

        assert!(lines.is_empty());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_18() {
        let text = lines_text();
        let r = Rope::from_str(&text);
        let s = r.slice(..);

        assert_eq!(34, r.lines(LineType::CRLF).count());
        assert_eq!(34, s.lines(LineType::CRLF).count());

        // Rope
        let mut lines = r.lines(LineType::CRLF);
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
        let mut lines = s.lines(LineType::CRLF);
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

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_19() {
    //     let r = Rope::from_str("a\nb\nc\nd\ne\nf\ng\nh\n");
    //     for (line, c) in r.lines(LineType::CRLF).zip('a'..='h') {
    //         assert_eq!(line, format!("{c}\n"))
    //     }
    //     for (line, c) in r
    //         .lines_at(r.len_lines() - 1)
    //         .reversed()
    //         .zip(('a'..='h').rev())
    //     {
    //         assert_eq!(line, format!("{c}\n"))
    //     }

    //     let r = Rope::from_str("ab\nc\nd\ne\nf\ng\nh\n");
    //     for (line, c) in r.slice(1..).lines(LineType::CRLF).zip('b'..='h') {
    //         assert_eq!(line, format!("{c}\n"))
    //     }
    // }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_01() {
        let text = lines_text();
        let r = Rope::from_str(&text);

        for i in 0..r.len_lines(LineType::CRLF) {
            let line = r.line(i, LineType::CRLF);
            let mut lines = r.lines_at(i, LineType::CRLF);
            assert_eq!(Some(line), lines.next());
        }

        let mut lines = r.lines_at(r.len_lines(LineType::CRLF), LineType::CRLF);
        assert_eq!(None, lines.next());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_02() {
        let text = lines_text();
        let r = Rope::from_str(&text);
        let s = r.slice(34..2031);

        for i in 0..s.len_lines(LineType::CRLF) {
            let line = s.line(i, LineType::CRLF);
            let mut lines = s.lines_at(i, LineType::CRLF);
            assert_eq!(Some(line), lines.next());
        }

        let mut lines = s.lines_at(s.len_lines(LineType::CRLF), LineType::CRLF);
        assert_eq!(None, lines.next());
    }

    #[cfg(feature = "metric_lines_cr_lf")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines_at_03() {
        let text = lines_text();
        let r = Rope::from_str(&text);
        let s = r.slice(34..34);

        let mut lines = s.lines_at(0, LineType::CRLF);
        assert_eq!("", lines.next().unwrap());

        let mut lines = s.lines_at(1, LineType::CRLF);
        assert_eq!(None, lines.next());
    }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_exact_size_iter_01() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(34..301);

    //     let mut line_count = s.len_lines();
    //     let mut lines = s.lines();

    //     assert_eq!(line_count, lines.len());

    //     while let Some(_) = lines.next() {
    //         line_count -= 1;
    //         assert_eq!(line_count, lines.len());
    //     }

    //     assert_eq!(lines.len(), 0);
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     assert_eq!(lines.len(), 0);
    //     assert_eq!(line_count, 0);
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_exact_size_iter_02() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(34..301);

    //     for i in 0..=s.len_lines() {
    //         let lines = s.lines_at(i);
    //         assert_eq!(s.len_lines() - i, lines.len());
    //     }
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_exact_size_iter_03() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(34..301);

    //     let mut line_count = 0;
    //     let mut lines = s.lines_at(s.len_lines());

    //     assert_eq!(line_count, lines.len());

    //     while lines.prev().is_some() {
    //         line_count += 1;
    //         assert_eq!(line_count, lines.len());
    //     }

    //     assert_eq!(lines.len(), s.len_lines());
    //     lines.prev();
    //     lines.prev();
    //     lines.prev();
    //     lines.prev();
    //     lines.prev();
    //     lines.prev();
    //     lines.prev();
    //     assert_eq!(lines.len(), s.len_lines());
    //     assert_eq!(line_count, s.len_lines());
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_exact_size_iter_04() {
    //     // Make sure splitting CRLF pairs at the end works properly.
    //     let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
    //     for i in 0..r.len_chars() {
    //         let s = r.slice(..i);
    //         let lines = s.lines();
    //         if cfg!(any(feature = "cr_lines", feature = "unicode_lines")) {
    //             assert_eq!(lines.len(), 1 + ((i + 1) / 2));
    //         } else {
    //             assert_eq!(lines.len(), 1 + (i / 2));
    //         }
    //         assert_eq!(lines.len(), lines.count());
    //     }
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_reverse_01() {
    //     let r = Rope::from_str(TEXT);
    //     let mut itr = r.lines();
    //     let mut stack = Vec::new();

    //     for _ in 0..8 {
    //         stack.push(itr.next().unwrap());
    //     }
    //     itr.reverse();
    //     for _ in 0..8 {
    //         assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
    //     }
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_reverse_02() {
    //     let r = Rope::from_str(TEXT);
    //     let mut itr = r.lines_at(r.len_lines() / 3);
    //     let mut stack = Vec::new();

    //     for _ in 0..8 {
    //         stack.push(itr.next().unwrap());
    //     }
    //     itr.reverse();
    //     for _ in 0..8 {
    //         assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
    //     }
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_reverse_03() {
    //     let r = Rope::from_str(TEXT);
    //     let mut itr = r.lines_at(r.len_lines() / 3);
    //     let mut stack = Vec::new();

    //     itr.reverse();
    //     for _ in 0..8 {
    //         stack.push(itr.next().unwrap());
    //     }
    //     itr.reverse();
    //     for _ in 0..8 {
    //         assert_eq!(stack.pop().unwrap(), itr.next().unwrap());
    //     }
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_reverse_04() {
    //     let mut itr = Lines::from_str("a\n", 1);

    //     assert_eq!(Some("a\n".into()), itr.next());
    //     assert_eq!(Some("".into()), itr.next());
    //     assert_eq!(None, itr.next());
    //     itr.reverse();
    //     assert_eq!(Some("".into()), itr.next());
    //     assert_eq!(Some("a\n".into()), itr.next());
    //     assert_eq!(None, itr.next());
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_reverse_exact_size_iter_01() {
    //     let r = Rope::from_str(TEXT);
    //     let s = r.slice(34..301);

    //     let mut lines = s.lines_at(4);
    //     lines.reverse();
    //     let mut line_count = 4;

    //     assert_eq!(4, lines.len());

    //     while let Some(_) = lines.next() {
    //         line_count -= 1;
    //         assert_eq!(line_count, lines.len());
    //     }

    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     lines.next();
    //     assert_eq!(lines.len(), 0);
    //     assert_eq!(line_count, 0);
    // }

    // #[test]
    // #[cfg_attr(miri, ignore)]
    // fn lines_reverse_exact_size_iter_02() {
    //     // Make sure splitting CRLF pairs at the end works properly.
    //     let r = Rope::from_str("\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
    //     for i in 0..r.len_chars() {
    //         let s = r.slice(..i);
    //         let lines = s.lines_at((i + 1) / 2).reversed();
    //         assert_eq!(lines.len(), lines.count());
    //     }
    // }
}
