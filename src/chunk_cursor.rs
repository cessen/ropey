use crate::{
    tree::{Node, TextInfo},
    RopeSlice,
};

#[cfg(any(
    feature = "metric_lines_lf",
    feature = "metric_lines_lf_cr",
    feature = "metric_lines_unicode"
))]
use crate::LineType;

#[derive(Debug, Clone)]
struct StackItem<'a> {
    node: &'a Node,
    info: &'a TextInfo,

    /// The absolute byte offset of this node from the start of the root node.
    /// Importantly, *not* from the start of `byte_range` in ChunkCursor: this
    /// doesn't take into account trimming.
    byte_offset: usize,

    /// For internal nodes, the current child index corresponding to the
    /// current chunk.
    child_idx: usize,
}

/// Cursor for traversing the chunks of a `Rope` or `RopeSlice`.
///
/// The methods `next()` and `prev()` move the cursor to the next or previous
/// chunk, and `chunk()` fetches the chunk that the cursor is currently on.
///
/// For example:
///
/// ```
/// # use ropey::Rope;
/// // Assume this rope has the chunks "Hello world, h" and "ow are you?".
/// let text = Rope::from_str("Hello world, how are you?");
/// # let text = {
/// # let mut builder = ropey::RopeBuilder::new();
/// # // Note: `_append_chunk_as_leaf()` is NOT part af the public API.
/// # // Do not use it outside of Ropey's code base.
/// # builder._append_chunk_as_leaf("Hello world, h");
/// # builder._append_chunk_as_leaf("ow are you?");
/// # builder.finish()
/// # };
///
/// let mut cursor = text.chunk_cursor();
///
/// assert_eq!(cursor.chunk(), "Hello world, h");
/// assert_eq!(cursor.next(), true);
/// assert_eq!(cursor.chunk(), "ow are you?");
/// assert_eq!(cursor.next(), false);
/// assert_eq!(cursor.chunk(), "ow are you?");
/// assert_eq!(cursor.prev(), true);
/// assert_eq!(cursor.chunk(), "Hello world, h");
/// ```
///
/// Note that unlike Ropey's iterators, `ChunkCursor` sits *on* a chunk, not
/// between chunks.

#[derive(Debug, Clone)]
pub struct ChunkCursor<'a> {
    // Note: empty and ignored when `str_slice` below is `Some`.
    node_stack: Vec<StackItem<'a>>,

    // If this is `Some`, then the chunk cursor is operating on a string slice,
    // not a normal rope.  In that case, `node_stack` above is unused and should
    // be empty.
    str_slice: Option<&'a str>,

    // The byte range within the root node/string slice that is considered part
    // of this cursor's contents.  For string slices (as opposed to rope slices)
    // this should always be set to `[0, length_of_str]`.
    byte_range: [usize; 2],
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
        // Already at the end.
        if self.at_last() {
            return false;
        }

        debug_assert!(self.node_stack.len() > 1);

        // Start at the node above the leaf.
        let mut stack_idx = self.node_stack.len() - 2;

        // Find the deepest node that's not at its end already.
        while self.node_stack[stack_idx].child_idx
            >= (self.node_stack[stack_idx].node.child_count() - 1)
        {
            debug_assert!(stack_idx > 0);
            stack_idx -= 1;
        }

        // Refill the stack starting from that node.
        self.node_stack[stack_idx].child_idx += 1;
        let byte_offset = {
            let child = &self.node_stack[stack_idx + 1];
            child.byte_offset + child.info.bytes
        };
        while self.node_stack[stack_idx].node.is_internal() {
            let child_i = self.node_stack[stack_idx].child_idx;
            let node = &self.node_stack[stack_idx].node.children().nodes()[child_i];
            let info = &self.node_stack[stack_idx].node.children().info()[child_i];

            stack_idx += 1;
            self.node_stack[stack_idx] = StackItem {
                node: node,
                info: info,
                byte_offset: byte_offset,
                child_idx: 0,
            };
        }

        debug_assert!(stack_idx == self.node_stack.len() - 1);

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
        // Already at the start.
        if self.at_first() {
            return false;
        }

        debug_assert!(self.node_stack.len() > 1);

        // Start at the node above the leaf.
        let mut stack_idx = self.node_stack.len() - 2;

        // Find the deepest node that's not at its start already.
        while self.node_stack[stack_idx].child_idx == 0 {
            debug_assert!(stack_idx > 0);
            stack_idx -= 1;
        }

        // Refill the stack starting from that node.
        self.node_stack[stack_idx].child_idx -= 1;
        let byte_offset_end = self.node_stack[stack_idx + 1].byte_offset;
        while self.node_stack[stack_idx].node.is_internal() {
            let child_i = self.node_stack[stack_idx].child_idx;
            let node = &self.node_stack[stack_idx].node.children().nodes()[child_i];
            let info = &self.node_stack[stack_idx].node.children().info()[child_i];
            let byte_offset = {
                let len = self.node_stack[stack_idx].node.children().info()[child_i].bytes;
                byte_offset_end - len
            };
            let position = match *node {
                Node::Leaf(_) => 0,
                Node::Internal(ref children) => children.len() - 1,
            };

            stack_idx += 1;
            self.node_stack[stack_idx] = StackItem {
                node: node,
                info: info,
                byte_offset: byte_offset,
                child_idx: position,
            };
        }

        debug_assert!(stack_idx == self.node_stack.len() - 1);

        true
    }

    /// Returns the current chunk.
    ///
    /// Runs in O(1) time.
    #[inline(always)]
    pub fn chunk(&self) -> &'a str {
        if let Some(text) = self.str_slice {
            return text;
        }

        self.chunk_slice().as_str().unwrap()
    }

    /// Returns the current chunk as a rope slice.
    ///
    /// Since it's a chunk, it's guaranteed to be contiguous text.
    pub(crate) fn chunk_slice(&self) -> RopeSlice<'a> {
        if let Some(text) = self.str_slice {
            return text.into();
        }

        let leaf = self.node_stack.last().unwrap();

        let start = self.byte_range[0].saturating_sub(leaf.byte_offset);
        let end = if (leaf.byte_offset + leaf.info.bytes) > self.byte_range[1] {
            self.byte_range[1] - leaf.byte_offset
        } else {
            leaf.info.bytes
        };

        RopeSlice::new(leaf.node, leaf.info, [start, end])
    }

    /// Returns whether the cursor is at the first chunk.
    ///
    /// Runs in O(1) time.
    pub fn at_first(&self) -> bool {
        if self.str_slice.is_some() {
            return true;
        }

        let leaf = &self.node_stack.last().unwrap();
        leaf.byte_offset <= self.byte_range[0]
    }

    /// Returns whether the cursor is at the last chunk.
    ///
    /// Runs in O(1) time.
    pub fn at_last(&self) -> bool {
        if self.str_slice.is_some() {
            return true;
        }

        let leaf = &self.node_stack.last().unwrap();
        (leaf.byte_offset + leaf.info.bytes) >= self.byte_range[1]
    }

    /// Returns the byte offset from the start of the text to the start of the
    /// current chunk.
    ///
    /// Runs in O(1) time.
    #[inline]
    pub fn byte_offset(&self) -> usize {
        if self.str_slice.is_some() {
            return 0;
        }

        // Offset from start of root.
        let offset = self.node_stack.last().unwrap().byte_offset;

        // Trimmed offset.
        offset
            .min(self.byte_range[1])
            .saturating_sub(self.byte_range[0])
    }

    /// Returns the byte offset from the start of the current chunk to the end of the text.
    #[inline]
    pub(crate) fn byte_offset_from_end(&self) -> usize {
        if self.str_slice.is_some() {
            return self.byte_range[1];
        }

        // Offset from start of root.
        let offset = self.node_stack.last().unwrap().byte_offset;

        // Trimmed and reversed offset.
        self.byte_range[1].saturating_sub(offset.max(self.byte_range[0]))
    }

    //---------------------------------------------------------

    /// Returns a chunk cursor with its current chunk being the one that
    /// contains the byte at `at_byte_idx`.
    ///
    /// Note that all parameters are relative to the entire contents of `node`.
    /// In particular, `at_byte_idx` is NOT relative to `byte_range`, it is an
    /// offset from the start of the full contents of `node`.
    pub(crate) fn new(
        node: &'a Node,
        info: &'a TextInfo,
        byte_range: [usize; 2],
        at_byte_idx: usize,
    ) -> crate::Result<Self> {
        if at_byte_idx < byte_range[0] || at_byte_idx > byte_range[1] {
            return Err(crate::Error::OutOfBounds);
        }

        let mut cursor = ChunkCursor {
            node_stack: vec![],
            str_slice: None,
            byte_range: byte_range,
        };

        // Find the chunk the contains `at_byte_idx` and set that as the current
        // chunk of the cursor.
        let mut current_node = node;
        let mut current_info = info;
        let mut current_byte_idx = 0;
        let mut local_byte_idx = at_byte_idx;
        loop {
            match *current_node {
                Node::Leaf(_) => {
                    cursor.node_stack.push(StackItem {
                        node: current_node,
                        info: current_info,
                        byte_offset: current_byte_idx,
                        child_idx: 0,
                    });
                    break;
                }

                Node::Internal(ref children) => {
                    let (child_i, acc_byte_idx) =
                        children.search_byte_idx_only(local_byte_idx, false);

                    cursor.node_stack.push(StackItem {
                        node: current_node,
                        info: current_info,
                        byte_offset: current_byte_idx,
                        child_idx: child_i,
                    });

                    current_byte_idx += acc_byte_idx;
                    local_byte_idx -= acc_byte_idx;
                    current_node = &children.nodes()[child_i];
                    current_info = &children.info()[child_i];
                }
            }
        }

        // Handle a subtle corner case where the slice end is exactly on an
        // internal chunk boundary, causing the selected chunk to be the one
        // just *after* the slice range.
        if cursor.byte_offset() >= (byte_range[1] - byte_range[0]) {
            cursor.prev();
        }

        Ok(cursor)
    }

    pub(crate) fn from_str(text: &'a str) -> crate::Result<Self> {
        Ok(ChunkCursor {
            node_stack: vec![],
            str_slice: Some(text),
            byte_range: [0, text.len()],
        })
    }

    pub(crate) fn is_from_str_slice(&self) -> bool {
        self.str_slice.is_some()
    }

    /// Attempts to advance the cursor to the next chunk that contains a line
    /// boundary.
    ///
    /// A "line boundary" in this case means:
    ///
    /// - The start of the text.
    /// - The end of the text.
    /// - A line break character.
    ///
    /// On success returns the common ancestor of the from/to chunks, along
    /// with its text info and it's byte offset from the start of the text.
    /// Note that the offset may be negative, since the node is not clipped
    /// to the slice boundaries.
    ///
    /// On failure (when already at the last chunk), returns `None`, and
    /// leaves the cursor state as-is.
    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[inline(always)]
    pub(crate) fn next_with_line_boundary(
        &mut self,
        line_type: LineType,
    ) -> Option<(&'a Node, &'a TextInfo, isize)> {
        // Already at the end.
        if self.at_last() {
            return None;
        }

        debug_assert!(self.node_stack.len() > 1);

        // Start at the node above the leaf.
        let mut stack_idx = self.node_stack.len() - 2;

        // Find the deepest node that's not at its end already and has a
        // subsequent child node with a line break.
        // The idea behind this loop is that you're always *on* the
        // child you should move off of when you come in.
        loop {
            let next_offset = self.node_stack[stack_idx + 1].info.bytes
                + self.node_stack[stack_idx + 1].byte_offset;

            if next_offset >= self.byte_range[1] {
                break;
            }

            if (self.node_stack[stack_idx].child_idx + 1)
                >= self.node_stack[stack_idx].node.child_count()
            {
                debug_assert!(stack_idx > 0);
                stack_idx -= 1;
                continue;
            }

            self.node_stack[stack_idx].child_idx += 1;

            let child_i = self.node_stack[stack_idx].child_idx;
            self.node_stack[stack_idx + 1] = StackItem {
                node: &self.node_stack[stack_idx].node.children().nodes()[child_i],
                info: &self.node_stack[stack_idx].node.children().info()[child_i],
                byte_offset: next_offset,
                child_idx: 0,
            };

            if self.node_stack[stack_idx].node.children().info()[child_i].line_breaks(line_type) > 0
            {
                break;
            }
        }

        // Store common anscestor for returning later.
        let top_node = self.node_stack[stack_idx].node;
        let top_info = self.node_stack[stack_idx].info;
        let top_offset =
            self.node_stack[stack_idx].byte_offset as isize - self.byte_range[0] as isize;

        // Refill the stack starting from that node.
        // After the previous loop, we should now be on a child that either
        // contains the next line break or is the last node in the byte
        // range.
        stack_idx += 1; // We've already handled the immediate child in the previous loop.
        while self.node_stack[stack_idx].node.is_internal() {
            let item = &self.node_stack[stack_idx];
            let mut child_idx = 0;
            let mut child_byte_offset = item.byte_offset;
            let mut child_node = &item.node.children().nodes()[child_idx];
            let mut child_info = &item.node.children().info()[child_idx];
            while (child_byte_offset + child_info.bytes) < self.byte_range[1]
                && child_info.line_breaks(line_type) == 0
            {
                child_idx += 1;
                child_byte_offset += child_info.bytes;
                child_node = &item.node.children().nodes()[child_idx];
                child_info = &item.node.children().info()[child_idx];
            }

            self.node_stack[stack_idx].child_idx = child_idx;

            stack_idx += 1;
            self.node_stack[stack_idx] = StackItem {
                node: child_node,
                info: child_info,
                byte_offset: child_byte_offset,
                child_idx: 0,
            };
        }

        debug_assert!(stack_idx == self.node_stack.len() - 1);

        Some((top_node, top_info, top_offset))
    }

    /// Attempts to backtrack the cursor to the previous chunk that contains a
    /// line boundary.
    ///
    /// A "line boundary" in this case means:
    ///
    /// - The start of the text.
    /// - The end of the text.
    /// - A line break character.
    ///
    /// On success returns the common ancestor of the from/to chunks, along
    /// with its text info and its byte offset from the start of the text.
    /// Note that the offset may be negative, since the node is not clipped
    /// to the slice boundaries.
    ///
    /// On failure (when already at the prev chunk), returns `None`, and
    /// leaves the cursor state as-is.
    #[cfg(any(
        feature = "metric_lines_lf",
        feature = "metric_lines_lf_cr",
        feature = "metric_lines_unicode"
    ))]
    #[inline(always)]
    pub(crate) fn prev_with_line_boundary(
        &mut self,
        line_type: LineType,
    ) -> Option<(&'a Node, &'a TextInfo, isize)> {
        // Already at the start.
        if self.at_first() {
            return None;
        }

        debug_assert!(self.node_stack.len() > 1);

        // Start at the node above the leaf.
        let mut stack_idx = self.node_stack.len() - 2;

        // Find the deepest node that's not at its start already and has a
        // prior child node with a line break.
        // The idea behind this loop is that you're always *on* the
        // child you should move off of when you come in.
        loop {
            let current_offset = self.node_stack[stack_idx + 1].byte_offset;
            if current_offset <= self.byte_range[0] {
                break;
            }

            if self.node_stack[stack_idx].child_idx == 0 {
                debug_assert!(stack_idx > 0);
                stack_idx -= 1;
                continue;
            }

            self.node_stack[stack_idx].child_idx -= 1;

            let child_i = self.node_stack[stack_idx].child_idx;
            let child_info = &self.node_stack[stack_idx].node.children().info()[child_i];
            self.node_stack[stack_idx + 1] = StackItem {
                node: &self.node_stack[stack_idx].node.children().nodes()[child_i],
                info: child_info,
                byte_offset: current_offset - child_info.bytes,
                child_idx: 0,
            };

            if child_info.line_breaks(line_type) > 0 {
                break;
            }
        }

        // Store common anscestor for returning later.
        let top_node = self.node_stack[stack_idx].node;
        let top_info = self.node_stack[stack_idx].info;
        let top_offset =
            self.node_stack[stack_idx].byte_offset as isize - self.byte_range[0] as isize;

        // Refill the stack starting from that node.
        // After the previous loop, we should now be on a child that either
        // contains the next line break or is the last node in the byte
        // range.
        stack_idx += 1; // We've already handled the immediate child in the previous loop.
        while self.node_stack[stack_idx].node.is_internal() {
            let item = &self.node_stack[stack_idx];
            let mut child_idx = item.node.children().len() - 1;
            let mut child_node = &item.node.children().nodes()[child_idx];
            let mut child_info = &item.node.children().info()[child_idx];
            let mut child_byte_offset = item.byte_offset + item.info.bytes - child_info.bytes;
            while child_byte_offset > self.byte_range[0] && child_info.line_breaks(line_type) == 0 {
                child_idx -= 1;
                child_node = &item.node.children().nodes()[child_idx];
                child_info = &item.node.children().info()[child_idx];
                child_byte_offset -= child_info.bytes;
            }

            self.node_stack[stack_idx].child_idx = child_idx;

            stack_idx += 1;
            self.node_stack[stack_idx] = StackItem {
                node: child_node,
                info: child_info,
                byte_offset: child_byte_offset,
                child_idx: 0,
            };
        }

        debug_assert!(stack_idx == self.node_stack.len() - 1);

        Some((top_node, top_info, top_offset))
    }
}

#[cfg(test)]
mod tests {
    use crate::{rope_builder::RopeBuilder, Rope, RopeSlice};

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

    #[cfg(feature = "metric_lines_lf_cr")]
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
    fn chunk_cursor_07() {
        let texts = [TEXT, ""];
        for text in texts {
            let t: RopeSlice = text.into();

            let mut cursor = t.chunk_cursor();

            assert!(cursor.at_first());
            assert!(cursor.at_last());
            assert_eq!(cursor.byte_offset(), 0);
            assert_eq!(cursor.chunk(), text);

            assert_eq!(false, cursor.next());
            assert!(cursor.at_first());
            assert!(cursor.at_last());
            assert_eq!(cursor.byte_offset(), 0);
            assert_eq!(cursor.chunk(), text);

            assert_eq!(false, cursor.prev());
            assert!(cursor.at_first());
            assert!(cursor.at_last());
            assert_eq!(cursor.byte_offset(), 0);
            assert_eq!(cursor.chunk(), text);
        }
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

    #[test]
    fn chunk_cursor_at_03() {
        // This tests a subtle corner case where the slice end aligns with
        // an internal chunk boundary, which would erronerously cause the
        // chunk cursor to be created on an empty chunk just *after* the slice
        // contents.  It requires a lot of nodes to trigger, because it needs
        // the tree to have enough depth.
        let r = {
            let mut rb = RopeBuilder::new();
            for _ in 0..100 {
                rb._append_chunk_as_leaf("A");
            }
            rb.finish()
        };

        for i in 1..=100 {
            let s = r.slice(..i);
            let cursor = s.chunk_cursor_at(i);
            assert_eq!("A", cursor.chunk());
        }
    }

    #[test]
    #[should_panic]
    fn chunk_cursor_at_04() {
        let r = Rope::from_str("foo");
        r.chunk_cursor_at(4);
    }

    #[test]
    #[should_panic]
    fn chunk_cursor_at_05() {
        let r = Rope::from_str("foo");
        let s = r.slice(1..2);
        s.chunk_cursor_at(2);
    }

    #[test]
    fn chunk_cursor_at_06() {
        let texts = [TEXT, ""];
        for text in texts {
            let t: RopeSlice = text.into();

            for i in 0..=text.len() {
                let cursor = t.chunk_cursor_at(i);

                assert!(cursor.at_first());
                assert!(cursor.at_last());
                assert_eq!(cursor.byte_offset(), 0);
                assert_eq!(cursor.chunk(), text);
            }
        }
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn chunk_cursor_line_boundary_01() {
        use crate::LineType::LF_CR;
        let r = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk_as_leaf("AAA");
            rb._append_chunk_as_leaf("B\nB");
            rb._append_chunk_as_leaf("C\nC");
            rb._append_chunk_as_leaf("DDD");
            rb._append_chunk_as_leaf("EEE");
            rb._append_chunk_as_leaf("F\nF");
            rb._append_chunk_as_leaf("GGG");
            rb._append_chunk_as_leaf("HHH");
            rb._append_chunk_as_leaf("III");
            rb._append_chunk_as_leaf("J\nJ");
            rb._append_chunk_as_leaf("KKK");
            rb.finish()
        };

        let mut cursor = r.chunk_cursor();

        // Forward.
        assert_eq!(0, cursor.byte_offset());
        assert_eq!("AAA", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!(3, cursor.byte_offset());
        assert_eq!("B\nB", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!(6, cursor.byte_offset());
        assert_eq!("C\nC", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!(15, cursor.byte_offset());
        assert_eq!("F\nF", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!(27, cursor.byte_offset());
        assert_eq!("J\nJ", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!(30, cursor.byte_offset());
        assert_eq!("KKK", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_none());
        assert_eq!(30, cursor.byte_offset());
        assert_eq!("KKK", cursor.chunk());

        // Backward.
        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!(27, cursor.byte_offset());
        assert_eq!("J\nJ", cursor.chunk());

        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!(15, cursor.byte_offset());
        assert_eq!("F\nF", cursor.chunk());

        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!(6, cursor.byte_offset());
        assert_eq!("C\nC", cursor.chunk());

        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!(3, cursor.byte_offset());
        assert_eq!("B\nB", cursor.chunk());

        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!(0, cursor.byte_offset());
        assert_eq!("AAA", cursor.chunk());

        assert!(cursor.prev_with_line_boundary(LF_CR).is_none());
        assert_eq!(0, cursor.byte_offset());
        assert_eq!("AAA", cursor.chunk());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn chunk_cursor_line_boundary_02() {
        use crate::LineType::LF_CR;
        let r = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk_as_leaf("AAA");
            rb._append_chunk_as_leaf("B\nB");
            rb._append_chunk_as_leaf("C\nC");
            rb._append_chunk_as_leaf("DDD");
            rb._append_chunk_as_leaf("EEE");
            rb._append_chunk_as_leaf("F\nF");
            rb._append_chunk_as_leaf("GGG");
            rb._append_chunk_as_leaf("HHH");
            rb._append_chunk_as_leaf("III");
            rb._append_chunk_as_leaf("J\nJ");
            rb._append_chunk_as_leaf("KKK");
            rb.finish()
        };
        let s = r.slice(4..29);

        let mut cursor = s.chunk_cursor();

        // Forward.
        assert_eq!(0, cursor.byte_offset());
        assert_eq!("\nB", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!(2, cursor.byte_offset());
        assert_eq!("C\nC", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!(11, cursor.byte_offset());
        assert_eq!("F\nF", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!(23, cursor.byte_offset());
        assert_eq!("J\n", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_none());
        assert_eq!(23, cursor.byte_offset());
        assert_eq!("J\n", cursor.chunk());

        // Backward.
        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!(11, cursor.byte_offset());
        assert_eq!("F\nF", cursor.chunk());

        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!(2, cursor.byte_offset());
        assert_eq!("C\nC", cursor.chunk());

        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!(0, cursor.byte_offset());
        assert_eq!("\nB", cursor.chunk());

        assert!(cursor.prev_with_line_boundary(LF_CR).is_none());
        assert_eq!(0, cursor.byte_offset());
        assert_eq!("\nB", cursor.chunk());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn chunk_cursor_line_boundary_03() {
        use crate::LineType::LF_CR;
        let r = {
            let mut rb = RopeBuilder::new();
            rb._append_chunk_as_leaf("AAA");
            rb._append_chunk_as_leaf("B\nB");
            rb._append_chunk_as_leaf("C\nC");
            rb._append_chunk_as_leaf("DDD");
            rb._append_chunk_as_leaf("EEE");
            rb._append_chunk_as_leaf("F\nF");
            rb._append_chunk_as_leaf("GGG");
            rb._append_chunk_as_leaf("HHH");
            rb._append_chunk_as_leaf("III");
            rb._append_chunk_as_leaf("J\nJ");
            rb._append_chunk_as_leaf("KKK");
            rb.finish()
        };
        let s = r.slice(4..29);

        let mut cursor = s.chunk_cursor();

        assert_eq!("\nB", cursor.chunk());
        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!("C\nC", cursor.chunk());
        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!("\nB", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!("C\nC", cursor.chunk());
        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!("F\nF", cursor.chunk());
        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!("C\nC", cursor.chunk());

        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!("F\nF", cursor.chunk());
        assert!(cursor.next_with_line_boundary(LF_CR).is_some());
        assert_eq!("J\n", cursor.chunk());
        assert!(cursor.prev_with_line_boundary(LF_CR).is_some());
        assert_eq!("F\nF", cursor.chunk());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn chunk_cursor_line_boundary_04() {
        use crate::LineType::LF_CR;
        let text = lines_text();
        let r = Rope::from_str(&text);

        let s = r.slice(..);
        let mut cursor = s.chunk_cursor();

        // Forward.
        while cursor.next_with_line_boundary(LF_CR).is_some() {}
        assert!(cursor.at_last());
        assert_eq!(text.len(), cursor.byte_offset() + cursor.chunk().len());

        // Backward.
        while cursor.prev_with_line_boundary(LF_CR).is_some() {}
        assert!(cursor.at_first());
        assert_eq!(0, cursor.byte_offset());
    }

    #[cfg(feature = "metric_lines_lf_cr")]
    #[test]
    fn chunk_cursor_line_boundary_05() {
        use crate::LineType::LF_CR;
        let l_text = lines_text();
        let texts = [&l_text, ""];
        for text in texts {
            let t: RopeSlice = text.into();
            let mut cursor = t.chunk_cursor();

            assert!(cursor.at_first());
            assert!(cursor.at_last());
            assert_eq!(cursor.byte_offset(), 0);
            assert_eq!(cursor.chunk(), text);

            // Forward.
            assert!(cursor.next_with_line_boundary(LF_CR).is_none());
            assert!(cursor.at_first());
            assert!(cursor.at_last());
            assert_eq!(cursor.byte_offset(), 0);
            assert_eq!(cursor.chunk(), text);

            // Backward.
            assert!(cursor.prev_with_line_boundary(LF_CR).is_none());
            assert!(cursor.at_first());
            assert!(cursor.at_last());
            assert_eq!(cursor.byte_offset(), 0);
            assert_eq!(cursor.chunk(), text);
        }
    }
}
