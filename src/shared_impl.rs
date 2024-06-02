//! The definitions in this module assume that the following methods are defined
//! on both Rope and RopeSlice:
//!
//! - `get_root()`: returns the root node of the Rope or RopeSlice.
//! - `get_root_info()`: returns the TextInfo of the root node.
//! - `get_full_info()`: like `get_root_info()` except it only returns the info
//!   if it represents the full extent of the Rope or RopeSlice.  Otherwise
//!   returns None.
//! - `get_byte_range()`: returns the range of bytes of the root node that are
//!   considered part of the actual text.

macro_rules! shared_main_impl_methods {
    () => {
        //-----------------------------------------------------
        // Queries.

        /// Total number of bytes in the text.
        ///
        /// Runs in O(1) time.
        #[inline(always)]
        pub fn len_bytes(&self) -> usize {
            self.get_byte_range()[1] - self.get_byte_range()[0]
        }

        /// Total number of `char`s in the text.
        ///
        /// Runs in best case O(1) time, worst-case O(log N).
        #[cfg(feature = "metric_chars")]
        #[inline]
        pub fn len_chars(&self) -> usize {
            if let Some(info) = self.get_full_info() {
                info.chars
            } else {
                let char_start_idx = self._byte_to_char(self.get_byte_range()[0]);
                let char_end_idx = self._byte_to_char(self.get_byte_range()[1]);
                char_end_idx - char_start_idx
            }
        }

        /// Total number of utf16 code units that would be in the text if it
        /// were encoded as utf16.
        ///
        /// Runs in best case O(1) time, worst-case O(log N).
        #[cfg(feature = "metric_utf16")]
        #[inline]
        pub fn len_utf16(&self) -> usize {
            if let Some(info) = self.get_full_info() {
                info.utf16
            } else {
                let utf16_start_idx = self._byte_to_utf16(self.get_byte_range()[0]);
                let utf16_end_idx = self._byte_to_utf16(self.get_byte_range()[1]);
                utf16_end_idx - utf16_start_idx
            }
        }

        /// Total number of lines in the text, according to the passed line type.
        ///
        /// Runs in best case O(1) time, worst-case O(log N).
        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        #[inline]
        pub fn len_lines(&self, line_type: LineType) -> usize {
            if let Some(info) = self.get_full_info() {
                info.line_breaks(line_type) + 1
            } else {
                let line_start_idx = self._byte_to_line(self.get_byte_range()[0], line_type);
                let line_end_idx = self._byte_to_line(self.get_byte_range()[1], line_type);
                let ends_with_crlf_split =
                    self._is_relevant_crlf_split(self.get_byte_range()[1], line_type);

                line_end_idx - line_start_idx + 1 + ends_with_crlf_split as usize
            }
        }

        /// Returns whether `byte_idx` is a `char` boundary.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[inline]
        pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
            assert!(byte_idx <= self.len_bytes());

            let (text, offset) = self.chunk(byte_idx);
            crate::is_char_boundary(byte_idx - offset, text.as_bytes())
        }

        /// Returns the byte index of the closest char boundary less than or
        /// equal to `byte_idx`.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[inline]
        pub fn floor_char_boundary(&self, byte_idx: usize) -> usize {
            assert!(byte_idx <= self.len_bytes());

            let (text, offset) = self.chunk(byte_idx);
            offset + crate::floor_char_boundary(byte_idx - offset, text.as_bytes())
        }

        /// Returns the byte index of the closest char boundary greater than or
        /// equal to `byte_idx`.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[inline]
        pub fn ceil_char_boundary(&self, byte_idx: usize) -> usize {
            assert!(byte_idx <= self.len_bytes());

            let (text, offset) = self.chunk(byte_idx);
            offset + crate::ceil_char_boundary(byte_idx - offset, text.as_bytes())
        }

        /// If there is a trailing line break, returns its byte index.
        /// Otherwise returns `None`.
        ///
        /// Note: a CRLF pair is always treated as a single unit, and thus
        /// this function will return the index of the CR in such cases, even
        /// with `LineType::LF` where CR is not on its own recognized as a line
        /// break.
        ///
        /// Runs in O(log N) time.
        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        pub fn trailing_line_break_idx(&self, line_type: LineType) -> Option<usize> {
            // Silence unused parameter warning with certain feature
            // configurations.
            let _ = line_type;

            if self.len_bytes() == 0 {
                return None;
            }

            // We try to do everything with just one chunk fetch.  However,
            // there is a single case when checking for CRLF where we might need
            // to fetch a second chunk, which is handled further below.  But
            // otherwise, we do everything with this one chunk.
            let last_chunk = self.chunk(self.len_bytes() - 1).0.as_bytes();
            let last_byte = last_chunk[last_chunk.len() - 1];

            // First handle LF and CRLF since that's the most typical case, and
            // also because it's the same for all line types.
            if last_byte == 0x0A {
                if self.len_bytes() > 1 {
                    let second_to_last_byte = if last_chunk.len() > 1 {
                        last_chunk[last_chunk.len() - 2]
                    } else {
                        // We need to fetch another chunk just for this one case.
                        self.byte(self.len_bytes() - 2)
                    };

                    if second_to_last_byte == 0x0D {
                        return Some(self.len_bytes() - 2);
                    }
                }

                return Some(self.len_bytes() - 1);
            }

            // That was the only case for `LineType::LF`, so early out if that's
            // the line type.
            #[cfg(feature = "metric_lines_lf")]
            if line_type == LineType::LF {
                return None;
            }

            // Next we handle CR on its own.
            if last_byte == 0x0D {
                return Some(self.len_bytes() - 1);
            }

            // That was the last case for `LineType::LF_CR`, so early out if
            // that's the line type.
            #[cfg(feature = "metric_lines_lf_cr")]
            if line_type == LineType::LF_CR {
                return None;
            }

            // Last char and its byte index.
            let last_char_byte_idx = crate::floor_char_boundary(last_chunk.len() - 1, last_chunk);
            let last_char = &last_chunk[last_char_byte_idx..];

            // Handle the remaining unicode cases.
            match last_char {
                // - VT (Vertical Tab)
                // - FF (Form Feed)
                // - NEL (Next Line)
                // - Line Separator
                // - Paragraph Separator
                &[0xb] | &[0xc] | &[0xc2, 0x85] | &[0xe2, 0x80, 0xa8] | &[0xe2, 0x80, 0xa9] => {
                    Some(last_char_byte_idx)
                }
                _ => None,
            }
        }

        //-----------------------------------------------------
        // Fetching.

        /// Returns the byte at `byte_idx`.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx >=
        /// len_bytes()`).
        #[inline(always)]
        pub fn byte(&self, byte_idx: usize) -> u8 {
            self.get_byte(byte_idx).unwrap()
        }

        /// Returns the `char` at `byte_idx`.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// - If `byte_idx` is out of bounds (i.e. `byte_idx >= len_bytes()`).
        /// - If `byte_idx` is not a char boundary.
        #[inline]
        pub fn char_at_byte(&self, byte_idx: usize) -> char {
            match self.try_char_at_byte(byte_idx) {
                Ok(ch) => ch,
                Err(NonCharBoundary) => panic!("Attempt to get a char at a non-char boundary."),
                Err(e) => e.panic_with_msg(),
            }
        }

        /// Returns the line at `line_idx`, according to the given line type.
        ///
        /// Note: lines are zero-indexed.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `line_idx` is out of bounds (i.e. `line_idx >= len_lines()`).
        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        #[inline(always)]
        pub fn line(&self, line_idx: usize, line_type: LineType) -> RopeSlice {
            self.get_line(line_idx, line_type).unwrap()
        }

        /// Returns the chunk containing the byte at `byte_idx`.
        ///
        /// Also returns the byte index of the beginning of the chunk.
        ///
        /// Note: for convenience, a one-past-the-end `byte_idx` returns the last
        /// chunk of the `RopeSlice`.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[inline(always)]
        pub fn chunk(&self, byte_idx: usize) -> (&str, usize) {
            self.get_chunk(byte_idx).unwrap()
        }

        //-----------------------------------------------------
        // Metric conversions.

        /// Returns the char index of the given byte.
        ///
        /// Notes:
        ///
        /// - If the byte is in the middle of a multi-byte char, returns the
        ///   index of the char that the byte belongs to.
        /// - `byte_idx` can be one-past-the-end, which will return
        ///   one-past-the-end char index.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[cfg(feature = "metric_chars")]
        #[inline]
        pub fn byte_to_char(&self, byte_idx: usize) -> usize {
            assert!(byte_idx <= self.len_bytes());

            if self.get_full_info().is_some() {
                self._byte_to_char(byte_idx)
            } else {
                self._byte_to_char(self.get_byte_range()[0] + byte_idx)
                    - self._byte_to_char(self.get_byte_range()[0])
            }
        }

        /// Returns the byte index of the given char.
        ///
        /// Notes:
        ///
        /// - `char_idx` can be one-past-the-end, which will return
        ///   one-past-the-end byte index.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `char_idx` is out of bounds (i.e. `char_idx > len_chars()`).
        #[cfg(feature = "metric_chars")]
        #[inline]
        pub fn char_to_byte(&self, char_idx: usize) -> usize {
            assert!(char_idx <= self.len_chars());
            if self.get_full_info().is_some() {
                self._char_to_byte(char_idx)
            } else {
                let char_start_idx = self._byte_to_char(self.get_byte_range()[0]);
                self._char_to_byte(char_start_idx + char_idx) - self.get_byte_range()[0]
            }
        }

        /// Returns the utf16 code unit index of the given byte.
        ///
        /// Ropey stores text internally as utf8, but sometimes it is necessary
        /// to interact with external APIs that still use utf16.  This function is
        /// primarily intended for such situations, and is otherwise not very
        /// useful.
        ///
        /// Note: if the byte is not on a char boundary, this returns the utf16
        /// code unit index corresponding to the char that the byte belongs to.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[cfg(feature = "metric_utf16")]
        #[inline]
        pub fn byte_to_utf16(&self, byte_idx: usize) -> usize {
            assert!(byte_idx <= self.len_bytes());
            if self.get_full_info().is_some() {
                self._byte_to_utf16(byte_idx)
            } else {
                self._byte_to_utf16(self.get_byte_range()[0] + byte_idx)
                    - self._byte_to_utf16(self.get_byte_range()[0])
            }
        }

        /// Returns the byte index of the given utf16 code unit.
        ///
        /// Ropey stores text internally as utf8, but sometimes it is necessary
        /// to interact with external APIs that still use utf16.  This function is
        /// primarily intended for such situations, and is otherwise not very
        /// useful.
        ///
        /// Note: if the utf16 code unit is in the middle of a char, returns the
        /// byte index of the char that it belongs to.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `utf16_cu_idx` is out of bounds
        /// (i.e. `utf16_idx > len_utf16()`).
        #[cfg(feature = "metric_utf16")]
        #[inline]
        pub fn utf16_to_byte(&self, utf16_idx: usize) -> usize {
            assert!(utf16_idx <= self.len_utf16());
            if self.get_full_info().is_some() {
                self._utf16_to_byte(utf16_idx)
            } else {
                let utf16_start_idx = self._byte_to_utf16(self.get_byte_range()[0]);
                self._utf16_to_byte(utf16_start_idx + utf16_idx) - self.get_byte_range()[0]
            }
        }

        /// Returns the line index of the line that the given byte belongs to.
        ///
        /// Notes:
        ///
        /// - Counts lines according to the passed line type.
        /// - Lines are zero-indexed.  This is functionally equivalent to
        ///   counting the line endings before the specified byte.
        /// - `byte_idx` can be one-past-the-end, which will return the
        ///   last line index.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        #[inline]
        pub fn byte_to_line(&self, byte_idx: usize, line_type: LineType) -> usize {
            assert!(byte_idx <= self.len_bytes());
            if self.get_full_info().is_some() {
                self._byte_to_line(byte_idx, line_type)
            } else {
                let crlf_split = if byte_idx == self.get_byte_range()[1] {
                    self._is_relevant_crlf_split(self.get_byte_range()[1], line_type)
                } else {
                    false
                };

                self._byte_to_line(self.get_byte_range()[0] + byte_idx, line_type)
                    - self._byte_to_line(self.get_byte_range()[0], line_type)
                    + crlf_split as usize
            }
        }

        /// Returns the byte index of the start of the given line.
        ///
        /// Notes:
        ///
        /// - Counts lines according to the passed line type.
        /// - Lines are zero-indexed.
        /// - `line_idx` can be one-past-the-end, which will return
        ///   one-past-the-end byte index.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `line_idx` is out of bounds (i.e. `line_idx > len_lines()`).
        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        #[inline]
        pub fn line_to_byte(&self, line_idx: usize, line_type: LineType) -> usize {
            assert!(line_idx <= self.len_lines(line_type));
            if self.get_full_info().is_some() {
                self._line_to_byte(line_idx, line_type)
            } else {
                let line_start_idx = self._byte_to_line(self.get_byte_range()[0], line_type);
                self._line_to_byte(line_start_idx + line_idx, line_type)
                    .saturating_sub(self.get_byte_range()[0])
                    .min(self.len_bytes())
            }
        }

        //-----------------------------------------------------
        // Iterators.

        /// Creates an iterator over the bytes of the `Rope`.
        ///
        /// Runs in O(log N) time.
        #[inline]
        pub fn bytes(&self) -> Bytes<'_> {
            Bytes::new(
                &self.get_root(),
                self.get_byte_range(),
                self.get_byte_range()[0],
            )
        }

        /// Creates an iterator over the bytes of the `Rope`, starting at byte
        /// `byte_idx`.
        ///
        /// If `byte_idx == len_bytes()` then an iterator at the end of the
        /// `Rope` is created (i.e. `next()` will return `None`).
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[inline]
        pub fn bytes_at(&self, byte_idx: usize) -> Bytes<'_> {
            Bytes::new(
                self.get_root(),
                self.get_byte_range(),
                self.get_byte_range()[0] + byte_idx,
            )
        }

        /// Creates an iterator over the chars of the `Rope`.
        ///
        /// Runs in O(log N) time.
        #[inline]
        pub fn chars(&self) -> Chars<'_> {
            Chars::new(
                self.get_root(),
                self.get_byte_range(),
                self.get_byte_range()[0],
            )
        }

        /// Creates an iterator over the chars of the `Rope`, starting at the char
        /// at `byte_idx`.
        ///
        /// If `byte_idx == len_bytes()` then an iterator at the end of the
        /// `Rope` is created (i.e. `next()` will return `None`).
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// - If `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        /// - If `byte_idx` is not a char boundary.
        #[inline]
        pub fn chars_at(&self, byte_idx: usize) -> Chars<'_> {
            Chars::new(
                self.get_root(),
                self.get_byte_range(),
                self.get_byte_range()[0] + byte_idx,
            )
        }

        /// Creates an iterator over the lines of the `Rope`.
        ///
        /// Note: the iterator will iterate over lines according to the passed
        /// line type.
        ///
        /// Runs in O(log N) time.
        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        #[inline]
        pub fn lines(&self, line_type: LineType) -> Lines<'_> {
            Lines::new(
                self.get_root(),
                self.get_root_info(),
                self.get_byte_range(),
                0,
                line_type,
            )
        }

        /// Creates an iterator over the lines of the `Rope`, starting at line
        /// `line_idx`.
        ///
        /// Notes:
        /// - The iterator will iterate over lines according to the passed
        ///   line type.
        /// - If `line_idx == len_lines()` then an iterator at the end of the
        ///   `Rope` is created (i.e. `next()` will return `None`).
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `line_idx` is out of bounds (i.e. `line_idx > len_lines()`).
        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        #[inline]
        pub fn lines_at(&self, line_idx: usize, line_type: LineType) -> Lines<'_> {
            Lines::new(
                self.get_root(),
                self.get_root_info(),
                self.get_byte_range(),
                line_idx,
                line_type,
            )
        }

        /// Creates an iterator over the chunks of the `Rope`.
        ///
        /// Runs in O(log N) time.
        #[inline]
        pub fn chunks(&self) -> Chunks<'_> {
            Chunks::new(
                self.get_root(),
                self.get_byte_range(),
                self.get_byte_range()[0],
            )
            .0
        }

        /// Creates an iterator over the chunks of the `Rope`, with the iterator
        /// starting at the chunk containing `byte_idx`.
        ///
        /// Also returns the byte index of the beginning of the first chunk to
        /// be yielded.
        ///
        /// If `byte_idx == len_bytes()` an iterator at the end of the `Rope`
        /// (yielding `None` on a call to `next()`) is created.
        ///
        /// Runs in O(log N) time.
        ///
        /// # Panics
        ///
        /// Panics if `byte_idx` is out of bounds (i.e. `byte_idx > len_bytes()`).
        #[inline]
        pub fn chunks_at(&self, byte_idx: usize) -> Chunks<'_> {
            Chunks::new(
                &self.get_root(),
                self.get_byte_range(),
                self.get_byte_range()[0] + byte_idx,
            )
            .0
        }

        //-----------------------------------------------------
        // Internal utility methods.

        #[cfg(feature = "metric_chars")]
        fn _byte_to_char(&self, byte_idx: usize) -> usize {
            let (text, start_info) = self.get_root().get_text_at_byte(byte_idx);
            start_info.chars + text.byte_to_char(byte_idx - start_info.bytes)
        }

        #[cfg(feature = "metric_chars")]
        fn _char_to_byte(&self, char_idx: usize) -> usize {
            let (text, start_info) = self.get_root().get_text_at_char(char_idx);
            start_info.bytes + text.char_to_byte(char_idx - start_info.chars)
        }

        #[cfg(feature = "metric_utf16")]
        fn _byte_to_utf16(&self, byte_idx: usize) -> usize {
            let (text, start_info) = self.get_root().get_text_at_byte(byte_idx);
            start_info.utf16 + text.byte_to_utf16(byte_idx - start_info.bytes)
        }

        #[cfg(feature = "metric_utf16")]
        fn _utf16_to_byte(&self, utf16_idx: usize) -> usize {
            let (text, start_info) = self.get_root().get_text_at_utf16(utf16_idx);
            start_info.bytes + text.utf16_to_byte(utf16_idx - start_info.utf16)
        }

        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        fn _byte_to_line(&self, byte_idx: usize, line_type: LineType) -> usize {
            let (text, start_info) = self.get_root().get_text_at_byte(byte_idx);

            start_info.line_breaks(line_type)
                + text.byte_to_line(byte_idx - start_info.bytes, line_type)
        }

        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        fn _line_to_byte(&self, line_idx: usize, line_type: LineType) -> usize {
            let (text, start_info) = self.get_root().get_text_at_line_break(line_idx, line_type);

            start_info.bytes
                + text.line_to_byte(line_idx - start_info.line_breaks(line_type), line_type)
        }

        /// Returns whether splitting at `byte_idx` would split a CRLF pair,
        /// if such  a split would be relevant to the line-counting metrics
        /// of `line_type`.   Specifically, CRLF pairs are not relevant to
        /// LF-only line metrics, so  for that line type this will always
        /// return false.  Otherwise it will  return if a CRLF pair would
        /// be split.
        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        pub(crate) fn _is_relevant_crlf_split(&self, byte_idx: usize, line_type: LineType) -> bool {
            self.get_root().is_relevant_crlf_split(byte_idx, line_type)
        }
    };
}

//=============================================================
// Non-panicking.

macro_rules! shared_no_panic_impl_methods {
    () => {
        //-----------------------------------------------------
        // Fetching.

        pub fn get_byte(&self, byte_idx: usize) -> Option<u8> {
            if byte_idx >= self.len_bytes() {
                return None;
            }

            let (text, offset) = self.chunk(byte_idx);
            Some(text.as_bytes()[byte_idx - offset])
        }

        pub fn try_char_at_byte(&self, byte_idx: usize) -> Result<char> {
            if byte_idx >= self.len_bytes() {
                return Err(OutOfBounds);
            }

            let (text, offset) = self.chunk(byte_idx);
            let local_idx = byte_idx - offset;

            if !crate::is_char_boundary(local_idx, text.as_bytes()) {
                return Err(NonCharBoundary);
            }

            // TODO: something more efficient than constructing a temporary
            // iterator.
            Ok(text[(byte_idx - offset)..].chars().next().unwrap())
        }

        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_lf_cr",
            feature = "metric_lines_unicode"
        ))]
        pub fn get_line(&self, line_idx: usize, line_type: LineType) -> Option<RopeSlice> {
            if line_idx >= self.len_lines(line_type) {
                return None;
            }

            let start_byte = self.line_to_byte(line_idx, line_type);
            let end_byte = self.line_to_byte(line_idx + 1, line_type);

            Some(self.slice(start_byte..end_byte))
        }

        pub fn get_chunk(&self, byte_idx: usize) -> Option<(&str, usize)> {
            if byte_idx > self.len_bytes() {
                return None;
            }

            let (chunk, start_byte) = self
                .get_root()
                .get_text_at_byte_fast(self.get_byte_range()[0] + byte_idx);

            if self.get_full_info().is_some() {
                // Simple case: we have a full rope, so no adjustments are
                // needed.
                Some((chunk.text(), start_byte))
            } else {
                // Trim chunk.
                let front_trim_idx = self.get_byte_range()[0].saturating_sub(start_byte);
                let back_trim_idx = (self.get_byte_range()[1] - start_byte).min(chunk.len());
                let trimmed_chunk = &chunk.text()[front_trim_idx..back_trim_idx];

                // Compute left-side text info.
                let local_start_byte = start_byte.saturating_sub(self.get_byte_range()[0]);

                Some((trimmed_chunk, local_start_byte))
            }
        }
    };
}

//=============================================================
// Stdlib trait impls.

macro_rules! shared_std_impls {
    ($rope:ty) => {
        //-----------------------------------------------------
        // Comparisons.

        impl std::cmp::Eq for $rope {}

        impl std::cmp::PartialEq<$rope> for $rope {
            fn eq(&self, other: &$rope) -> bool {
                if self.len_bytes() != other.len_bytes() {
                    return false;
                }

                let mut chunk_itr_1 = self.chunks();
                let mut chunk_itr_2 = other.chunks();
                let mut chunk1 = chunk_itr_1.next().unwrap_or("").as_bytes();
                let mut chunk2 = chunk_itr_2.next().unwrap_or("").as_bytes();

                loop {
                    if chunk1.len() > chunk2.len() {
                        if &chunk1[..chunk2.len()] != chunk2 {
                            return false;
                        } else {
                            chunk1 = &chunk1[chunk2.len()..];
                            chunk2 = &[];
                        }
                    } else if &chunk2[..chunk1.len()] != chunk1 {
                        return false;
                    } else {
                        chunk2 = &chunk2[chunk1.len()..];
                        chunk1 = &[];
                    }

                    if chunk1.is_empty() {
                        if let Some(chunk) = chunk_itr_1.next() {
                            chunk1 = chunk.as_bytes();
                        } else {
                            break;
                        }
                    }

                    if chunk2.is_empty() {
                        if let Some(chunk) = chunk_itr_2.next() {
                            chunk2 = chunk.as_bytes();
                        } else {
                            break;
                        }
                    }
                }

                return true;
            }
        }

        impl std::cmp::PartialEq<&str> for $rope {
            #[inline]
            fn eq(&self, other: &&str) -> bool {
                if self.len_bytes() != other.len() {
                    return false;
                }
                let other = other.as_bytes();

                let mut idx = 0;
                for chunk in self.chunks() {
                    let chunk = chunk.as_bytes();
                    if chunk != &other[idx..(idx + chunk.len())] {
                        return false;
                    }
                    idx += chunk.len();
                }

                return true;
            }
        }

        impl std::cmp::PartialEq<$rope> for &str {
            #[inline]
            fn eq(&self, other: &$rope) -> bool {
                other == self
            }
        }

        impl std::cmp::PartialEq<str> for $rope {
            #[inline(always)]
            fn eq(&self, other: &str) -> bool {
                std::cmp::PartialEq::<&str>::eq(self, &other)
            }
        }

        impl std::cmp::PartialEq<$rope> for str {
            #[inline(always)]
            fn eq(&self, other: &$rope) -> bool {
                std::cmp::PartialEq::<&str>::eq(other, &self)
            }
        }

        impl std::cmp::PartialEq<String> for $rope {
            #[inline(always)]
            fn eq(&self, other: &String) -> bool {
                self == other.as_str()
            }
        }

        impl std::cmp::PartialEq<$rope> for String {
            #[inline(always)]
            fn eq(&self, other: &$rope) -> bool {
                other == self.as_str()
            }
        }

        impl std::cmp::PartialEq<std::borrow::Cow<'_, str>> for $rope {
            #[inline]
            fn eq(&self, other: &std::borrow::Cow<str>) -> bool {
                *self == **other
            }
        }

        impl std::cmp::PartialEq<$rope> for std::borrow::Cow<'_, str> {
            #[inline]
            fn eq(&self, other: &$rope) -> bool {
                *other == **self
            }
        }

        impl std::cmp::Ord for $rope {
            fn cmp(&self, other: &$rope) -> std::cmp::Ordering {
                let mut chunk_itr_1 = self.chunks();
                let mut chunk_itr_2 = other.chunks();
                let mut chunk1 = chunk_itr_1.next().unwrap_or("").as_bytes();
                let mut chunk2 = chunk_itr_2.next().unwrap_or("").as_bytes();

                loop {
                    if chunk1.len() >= chunk2.len() {
                        let compared = chunk1[..chunk2.len()].cmp(chunk2);
                        if compared != std::cmp::Ordering::Equal {
                            return compared;
                        }

                        chunk1 = &chunk1[chunk2.len()..];
                        chunk2 = &[];
                    } else {
                        let compared = chunk1.cmp(&chunk2[..chunk1.len()]);
                        if compared != std::cmp::Ordering::Equal {
                            return compared;
                        }

                        chunk1 = &[];
                        chunk2 = &chunk2[chunk1.len()..];
                    }

                    if chunk1.is_empty() {
                        if let Some(chunk) = chunk_itr_1.next() {
                            chunk1 = chunk.as_bytes();
                        } else {
                            break;
                        }
                    }

                    if chunk2.is_empty() {
                        if let Some(chunk) = chunk_itr_2.next() {
                            chunk2 = chunk.as_bytes();
                        } else {
                            break;
                        }
                    }
                }

                self.len_bytes().cmp(&other.len_bytes())
            }
        }

        impl std::cmp::PartialOrd<$rope> for $rope {
            #[inline]
            fn partial_cmp(&self, other: &$rope) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        //-----------------------------------------------------
        // Conversions.

        impl From<$rope> for String {
            fn from(r: $rope) -> String {
                (&r).into()
            }
        }

        impl<'a> From<&'a $rope> for String {
            #[inline]
            fn from(r: &'a $rope) -> Self {
                let mut s = String::with_capacity(r.len_bytes());
                s.extend(r.chunks());
                s
            }
        }

        /// Attempts to borrow the text contents, but will return `None` if the
        /// contents is not contiguous in memory.
        ///
        /// Runs in best case O(1), worst case O(N).
        impl<'a> From<&'a $rope> for Option<&'a str> {
            #[inline]
            fn from(r: &'a $rope) -> Self {
                match r.get_root() {
                    Node::Leaf(ref text) => {
                        let [start, end] = r.get_byte_range();
                        Some(&text.text()[start..end])
                    }
                    Node::Internal(_) => None,
                }
            }
        }

        /// Attempts to borrow the text contents, but will convert to an
        /// owned string if the contents is not contiguous in memory.
        ///
        /// Runs in best case O(1), worst case O(N).
        impl<'a> From<&'a $rope> for std::borrow::Cow<'a, str> {
            #[inline]
            fn from(r: &'a $rope) -> Self {
                match r.get_root() {
                    Node::Leaf(ref text) => {
                        let [start, end] = r.get_byte_range();
                        std::borrow::Cow::Borrowed(&text.text()[start..end])
                    }
                    Node::Internal(_) => std::borrow::Cow::Owned(String::from(r)),
                }
            }
        }

        //-----------------------------------------------------
        // Misc.

        impl std::fmt::Debug for $rope {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.debug_list().entries(self.chunks()).finish()
            }
        }

        impl std::fmt::Display for $rope {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                for chunk in self.chunks() {
                    write!(f, "{}", chunk)?
                }
                Ok(())
            }
        }

        impl std::hash::Hash for $rope {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                // `std::hash::Hasher` only guarantees the same hash output for
                // exactly the same calls to `Hasher::write()`.  Just submitting
                // the same data in the same order isn't enough--it also has
                // to be split the same between calls.  So we go to some effort
                // here to ensure that we always submit the text data in the
                // same fixed-size blocks, even if those blocks don't align with
                // chunk boundaries at all.
                //
                // The naive approach is to always copy to a fixed-size buffer
                // and submit the buffer whenever it fills up.  We conceptually
                // follow that approach here, but we do a little better by
                // skipping the buffer and directly passing the data without
                // copying when possible.
                const BLOCK_SIZE: usize = 256;

                let mut buffer = [0u8; BLOCK_SIZE];
                let mut buffer_len = 0;

                for chunk in self.chunks() {
                    let mut data = chunk.as_bytes();

                    while !data.is_empty() {
                        if buffer_len == 0 && data.len() >= BLOCK_SIZE {
                            // Process data directly, skipping the buffer.
                            let (head, tail) = data.split_at(BLOCK_SIZE);
                            state.write(head);
                            data = tail;
                        } else if buffer_len == BLOCK_SIZE {
                            // Process the filled buffer.
                            state.write(&buffer[..]);
                            buffer_len = 0;
                        } else {
                            // Append to the buffer.
                            let n = (BLOCK_SIZE - buffer_len).min(data.len());
                            let (head, tail) = data.split_at(n);
                            buffer[buffer_len..(buffer_len + n)].copy_from_slice(head);
                            buffer_len += n;
                            data = tail;
                        }
                    }
                }

                // Write any remaining unprocessed data in the buffer.
                if buffer_len > 0 {
                    state.write(&buffer[..buffer_len]);
                }

                // Same strategy as `&str` in stdlib, so that e.g. two adjacent
                // fields in a `#[derive(Hash)]` struct with "Hi " and "there"
                // vs "Hi t" and "here" give the struct a different hash.
                state.write_u8(0xff)
            }
        }
    };
}

pub(crate) use {shared_main_impl_methods, shared_no_panic_impl_methods, shared_std_impls};
