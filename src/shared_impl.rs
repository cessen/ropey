macro_rules! impl_shared_methods {
    () => {
        //---------------------------------------------------------
        // Queries.

        pub fn len_bytes(&self) -> usize {
            self.get_byte_range()[1] - self.get_byte_range()[0]
        }

        #[cfg(feature = "metric_chars")]
        pub fn len_chars(&self) -> usize {
            if let Some(info) = self.get_full_info() {
                info.chars
            } else {
                let char_start_idx = self._byte_to_char(self.get_byte_range()[0]);
                let char_end_idx = self._byte_to_char(self.get_byte_range()[1]);
                char_end_idx - char_start_idx
            }
        }

        #[cfg(feature = "metric_utf16")]
        pub fn len_utf16(&self) -> usize {
            if let Some(info) = self.get_full_info() {
                info.chars + info.utf16_surrogates
            } else {
                let utf16_start_idx = self._byte_to_utf16(self.get_byte_range()[0]);
                let utf16_end_idx = self._byte_to_utf16(self.get_byte_range()[1]);
                utf16_end_idx - utf16_start_idx
            }
        }

        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_cr_lf",
            feature = "metric_lines_unicode"
        ))]
        pub fn len_lines(&self, line_type: LineType) -> usize {
            if let Some(info) = self.get_full_info() {
                info.line_breaks(line_type) + 1
            } else {
                let line_start_idx = self._byte_to_line(self.get_byte_range()[0], line_type);
                let line_end_idx = self._byte_to_line(self.get_byte_range()[1], line_type);
                let ends_with_crlf_split =
                    self.is_relevant_crlf_split(self.get_byte_range()[1], line_type);

                line_end_idx - line_start_idx + 1 + ends_with_crlf_split as usize
            }
        }

        pub fn is_char_boundary(&self, byte_idx: usize) -> bool {
            assert!(byte_idx <= self.len_bytes());

            let byte_idx = byte_idx + self.get_byte_range()[0];
            self.get_root().is_char_boundary(byte_idx)
        }

        //---------------------------------------------------------
        // Metric conversions.

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

        #[cfg(feature = "metric_chars")]
        pub fn char_to_byte(&self, char_idx: usize) -> usize {
            assert!(char_idx <= self.len_chars());
            if self.get_full_info().is_some() {
                self._char_to_byte(char_idx)
            } else {
                let char_start_idx = self._byte_to_char(self.get_byte_range()[0]);
                self._char_to_byte(char_start_idx + char_idx) - self.get_byte_range()[0]
            }
        }

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

        #[cfg(feature = "metric_utf16")]
        pub fn utf16_to_byte(&self, utf16_idx: usize) -> usize {
            assert!(utf16_idx <= self.len_utf16());
            if self.get_full_info().is_some() {
                self._utf16_to_byte(utf16_idx)
            } else {
                let utf16_start_idx = self._byte_to_utf16(self.get_byte_range()[0]);
                self._utf16_to_byte(utf16_start_idx + utf16_idx) - self.get_byte_range()[0]
            }
        }

        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_cr_lf",
            feature = "metric_lines_unicode"
        ))]
        #[inline]
        pub fn byte_to_line(&self, byte_idx: usize, line_type: LineType) -> usize {
            assert!(byte_idx <= self.len_bytes());
            if self.get_full_info().is_some() {
                self._byte_to_line(byte_idx, line_type)
            } else {
                let crlf_split = if byte_idx == self.get_byte_range()[1] {
                    self.is_relevant_crlf_split(self.get_byte_range()[1], line_type)
                } else {
                    false
                };

                self._byte_to_line(self.get_byte_range()[0] + byte_idx, line_type)
                    - self._byte_to_line(self.get_byte_range()[0], line_type)
                    + crlf_split as usize
            }
        }

        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_cr_lf",
            feature = "metric_lines_unicode"
        ))]
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

        //---------------------------------------------------------
        // Internal utility methods.

        #[cfg(feature = "metric_chars")]
        fn _byte_to_char(&self, byte_idx: usize) -> usize {
            let (start_info, text, _) = self
                .get_root()
                .get_text_at_byte(byte_idx, self.get_root_info());
            start_info.chars + text.byte_to_char(byte_idx - start_info.bytes)
        }

        #[cfg(feature = "metric_chars")]
        fn _char_to_byte(&self, char_idx: usize) -> usize {
            let (start_info, text, _) = self
                .get_root()
                .get_text_at_char(char_idx, self.get_root_info());
            start_info.bytes + text.char_to_byte(char_idx - start_info.chars)
        }

        #[cfg(feature = "metric_utf16")]
        fn _byte_to_utf16(&self, byte_idx: usize) -> usize {
            let (start_info, text, _) = self
                .get_root()
                .get_text_at_byte(byte_idx, self.get_root_info());
            start_info.chars
                + start_info.utf16_surrogates
                + text.byte_to_utf16(byte_idx - start_info.bytes)
        }

        #[cfg(feature = "metric_utf16")]
        fn _utf16_to_byte(&self, utf16_idx: usize) -> usize {
            let (start_info, text, _) = self
                .get_root()
                .get_text_at_utf16(utf16_idx, self.get_root_info());
            start_info.bytes
                + text.utf16_to_byte(utf16_idx - (start_info.chars + start_info.utf16_surrogates))
        }

        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_cr_lf",
            feature = "metric_lines_unicode"
        ))]
        fn _byte_to_line(&self, byte_idx: usize, line_type: LineType) -> usize {
            let (start_info, text, _) = self
                .get_root()
                .get_text_at_byte(byte_idx, self.get_root_info());

            start_info.line_breaks(line_type)
                + text.byte_to_line(byte_idx - start_info.bytes, line_type)
        }

        #[cfg(any(
            feature = "metric_lines_lf",
            feature = "metric_lines_cr_lf",
            feature = "metric_lines_unicode"
        ))]
        fn _line_to_byte(&self, line_idx: usize, line_type: LineType) -> usize {
            let (start_info, text, _) =
                self.get_root()
                    .get_text_at_line_break(line_idx, self.get_root_info(), line_type);

            start_info.bytes
                + text.line_to_byte(line_idx - start_info.line_breaks(line_type), line_type)
        }

        //---------------------------------------------------------
        // Iterators.

        pub fn bytes(&self) -> Bytes<'_> {
            Bytes::new(
                &self.get_root(),
                self.get_byte_range(),
                self.get_byte_range()[0],
            )
        }

        pub fn chunks(&self) -> Chunks<'_> {
            Chunks::new(
                &self.get_root(),
                self.get_byte_range(),
                self.get_byte_range()[0],
            )
            .0
        }
    };
}

pub(crate) use impl_shared_methods;
