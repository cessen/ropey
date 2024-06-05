use crate::tree::Node;

#[derive(Debug, Clone)]
pub struct ChunkCursor<'a> {
    node_stack: Vec<(&'a Node, usize)>, // (node ref, index of current child)

    // The byte range within the root node that is considered part of this
    // cursor's contents.
    byte_range: [usize; 2],

    // The offset within the root node (*not* `byte_range`) of the current
    // un-trimmed chunk.
    current_byte_idx: usize,
}

impl<'a> ChunkCursor<'a> {
    /// Attempts to advance the cursor to the next chunk.
    ///
    /// Returns true on success.  Returns false if it's already on the last
    /// chunk, in which case the cursor remains on the last chunk.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        // Special case for empty cursors.
        if self.byte_range[0] == self.byte_range[1] {
            return false;
        }

        let current_leaf_len = self.node_stack.last().unwrap().0.leaf_text().len();

        // Already at the end.
        if (self.current_byte_idx + current_leaf_len) >= self.byte_range[1] {
            return false;
        }

        self.current_byte_idx += current_leaf_len;

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

        true
    }

    /// Attempts to backtrack the cursor to the previous chunk.
    ///
    /// Returns true on success.  Returns false if it's already on the first
    /// chunk, in which case the cursor remains on the first chunk.
    ///
    /// Runs in amortized O(1) time and worst-case O(log N) time.
    #[inline(always)]
    pub fn prev(&mut self) -> bool {
        // Special case for empty cursors.
        if self.byte_range[0] == self.byte_range[1] {
            return false;
        }

        // Already at the start.
        if self.current_byte_idx <= self.byte_range[0] {
            return false;
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

        true
    }

    /// Returns the current chunk.
    pub fn chunk(&self) -> &'a str {
        // Special case for empty cursors.
        if self.byte_range[0] == self.byte_range[1] {
            return "";
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

        trimmed_chunk
    }

    /// Returns the byte offset from the start of the text to the start of the current chunk.
    #[inline]
    pub fn byte_offset(&self) -> usize {
        self.current_byte_idx
            .min(self.byte_range[1])
            .saturating_sub(self.byte_range[0])
    }

    /// Returns the byte offset from the start of the current chunk to the end of the text.
    #[inline]
    pub fn byte_offset_from_end(&self) -> usize {
        self.byte_range[1].saturating_sub(self.current_byte_idx.max(self.byte_range[0]))
    }

    //---------------------------------------------------------

    /// Returns a chunk cursor with its current chunk being the one that
    /// contains the byte at `at_byte_idx`.
    ///
    /// Note that all parameters are relative to the entire contents of `node`.
    /// In particular, `at_byte_idx` is NOT relative to `byte_range`, it is an
    /// offset from the start of the full contents of `node`.
    pub(crate) fn new(node: &Node, byte_range: [usize; 2], at_byte_idx: usize) -> ChunkCursor {
        debug_assert!(byte_range[0] <= at_byte_idx && at_byte_idx <= byte_range[1]);

        // Special case: if it's an empty rope, don't store anything.
        if byte_range[0] == byte_range[1] || node.is_empty() {
            return ChunkCursor {
                node_stack: vec![],
                byte_range: [0, 0],
                current_byte_idx: 0,
            };
        }

        let mut cursor = ChunkCursor {
            node_stack: vec![],
            byte_range: byte_range,
            current_byte_idx: 0,
        };

        // Find the chunk the contains `at_byte_idx` and set that as the current
        // chunk of the cursor.
        let mut current_node = node;
        let mut local_byte_idx = at_byte_idx;
        loop {
            match *current_node {
                Node::Leaf(_) => {
                    cursor.node_stack.push((current_node, 0));
                    break;
                }

                Node::Internal(ref children) => {
                    let (child_i, acc_byte_idx) = children.search_byte_idx_only(local_byte_idx);

                    cursor.current_byte_idx += acc_byte_idx;
                    local_byte_idx -= acc_byte_idx;

                    cursor.node_stack.push((current_node, child_i));
                    current_node = &children.nodes()[child_i];
                }
            }
        }

        cursor
    }
}

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

    #[test]
    fn chunk_cursor_01() {
        let r = Rope::from_str(TEXT);

        let mut text = TEXT;
        let mut cursor = r.chunk_cursor();
        let mut stack = Vec::new();

        // Forward.
        let mut byte_offset = 0;
        loop {
            let chunk = cursor.chunk();
            assert_eq!(&text[..chunk.len()], chunk);
            assert_eq!(cursor.byte_offset(), byte_offset);
            stack.push(chunk);
            byte_offset += chunk.len();
            text = &text[chunk.len()..];

            if !cursor.next() {
                break;
            }
        }
        assert_eq!("", text);
        assert_eq!(cursor.byte_offset(), TEXT.len() - cursor.chunk().len());

        // Backward.
        loop {
            let chunk = cursor.chunk();
            assert_eq!(stack.pop().unwrap(), chunk);

            if !cursor.prev() {
                break;
            }
        }
        assert_eq!(0, stack.len());
    }

    #[test]
    fn chunk_cursor_02() {
        let r = Rope::from_str(TEXT);

        let text_slice = &TEXT[3..45];
        let mut text = text_slice;
        let s = r.slice(3..45);
        let mut cursor = s.chunk_cursor();
        let mut stack = Vec::new();

        // Forward.
        let mut byte_offset = 0;
        loop {
            let chunk = cursor.chunk();
            assert_eq!(&text[..chunk.len()], chunk);
            assert_eq!(cursor.byte_offset(), byte_offset);
            stack.push(chunk);
            byte_offset += chunk.len();
            text = &text[chunk.len()..];

            if !cursor.next() {
                break;
            }
        }
        assert_eq!("", text);
        assert_eq!(
            cursor.byte_offset(),
            text_slice.len() - cursor.chunk().len()
        );

        // Backward.
        loop {
            let chunk = cursor.chunk();
            assert_eq!(stack.pop().unwrap(), chunk);

            if !cursor.prev() {
                break;
            }
        }
        assert_eq!(0, stack.len());
    }

    #[test]
    fn chunk_cursor_03() {
        let r = hello_world_repeat_rope();

        let mut cursor = r.chunk_cursor();

        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("world!", cursor.chunk());

        assert_eq!(false, cursor.next());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(false, cursor.next());
        assert_eq!("world!", cursor.chunk());

        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());

        assert_eq!(false, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(false, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());
    }

    #[test]
    fn chunk_cursor_04() {
        let r = hello_world_repeat_rope();
        let s = r.slice(3..45);

        let mut cursor = s.chunk_cursor();

        assert_eq!("lo ", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.next());
        assert_eq!("wor", cursor.chunk());

        assert_eq!(false, cursor.next());
        assert_eq!("wor", cursor.chunk());
        assert_eq!(false, cursor.next());
        assert_eq!("wor", cursor.chunk());

        assert_eq!("wor", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("Hello ", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("world!", cursor.chunk());
        assert_eq!(true, cursor.prev());
        assert_eq!("lo ", cursor.chunk());

        assert_eq!(false, cursor.prev());
        assert_eq!("lo ", cursor.chunk());
        assert_eq!(false, cursor.prev());
        assert_eq!("lo ", cursor.chunk());
    }

    #[test]
    fn chunk_cursor_05() {
        let r = Rope::from_str("");

        let mut cursor = r.chunk_cursor();

        assert_eq!(cursor.byte_offset(), 0);
        assert_eq!("", cursor.chunk());

        assert_eq!(false, cursor.next());
        assert_eq!(cursor.byte_offset(), 0);
        assert_eq!("", cursor.chunk());

        assert_eq!(false, cursor.prev());
        assert_eq!(cursor.byte_offset(), 0);
        assert_eq!("", cursor.chunk());
    }

    #[test]
    fn chunk_cursor_06() {
        let r = hello_world_repeat_rope();
        let s = r.slice(14..14);

        let mut cursor = s.chunk_cursor();

        assert_eq!(cursor.byte_offset(), 0);
        assert_eq!("", cursor.chunk());

        assert_eq!(false, cursor.next());
        assert_eq!(cursor.byte_offset(), 0);
        assert_eq!("", cursor.chunk());

        assert_eq!(false, cursor.prev());
        assert_eq!(cursor.byte_offset(), 0);
        assert_eq!("", cursor.chunk());
    }

    #[test]
    fn chunk_cursor_at_01() {
        let r = Rope::from_str(TEXT);

        for i in 0..=TEXT.len() {
            let cursor = r.chunk_cursor_at(i);
            let chunk = cursor.chunk();
            let byte_offset = cursor.byte_offset();

            assert!(i >= byte_offset && i <= (byte_offset + chunk.len()));
            assert_eq!(&TEXT[byte_offset..(byte_offset + chunk.len())], chunk);
        }

        let cursor_1 = r.chunk_cursor_at(TEXT.len() - 1);
        let cursor_2 = r.chunk_cursor_at(TEXT.len());
        assert_eq!(cursor_1.byte_offset(), cursor_2.byte_offset());
        assert_eq!(cursor_1.chunk(), cursor_2.chunk());
    }

    #[test]
    fn chunk_cursor_at_02() {
        let r = Rope::from_str(TEXT);
        let s = r.slice(5..124);
        let text = &TEXT[5..124];

        for i in 0..=text.len() {
            let cursor = s.chunk_cursor_at(i);
            let chunk = cursor.chunk();
            let byte_offset = cursor.byte_offset();

            assert!(i >= byte_offset && i <= (byte_offset + chunk.len()));
            assert_eq!(&text[byte_offset..(byte_offset + chunk.len())], chunk);
        }

        let cursor_1 = s.chunk_cursor_at(text.len() - 1);
        let cursor_2 = s.chunk_cursor_at(text.len());
        assert_eq!(cursor_1.byte_offset(), cursor_2.byte_offset());
        assert_eq!(cursor_1.chunk(), cursor_2.chunk());
    }
}
