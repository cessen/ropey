/// Returns whether the given byte index in `text` is a valid
/// splitting point.  Valid splitting point in this case means
/// that it _is_ a utf8 code point boundary and _is not_ the
/// middle of a CRLF pair.
#[inline]
pub fn is_break(byte_idx: usize, text: &[u8]) -> bool {
    debug_assert!(byte_idx <= text.len());

    if byte_idx == 0 || byte_idx == text.len() {
        true
    } else {
        (text[byte_idx] >> 6 != 0b10) && ((text[byte_idx - 1] != 0x0D) | (text[byte_idx] != 0x0A))
    }
}

/// Returns whether the seam between `left` and `right` is a valid
/// splitting point.  Valid splitting point in this case means
/// that it _is_ a utf8 code point boundary and _is not_ the middle
/// of a CRLF pair.
#[inline]
pub fn seam_is_break(left: &[u8], right: &[u8]) -> bool {
    debug_assert!(!left.is_empty() && !right.is_empty());
    (right[0] >> 6 != 0b10) && ((left[left.len() - 1] != 0x0D) | (right[0] != 0x0A))
}

/// Returns the segment break before (but not including) the given byte
/// boundary.
///
/// This will return back the passed byte boundary if it is at the start
/// of the string.
#[inline]
pub fn prev_break(byte_idx: usize, text: &[u8]) -> usize {
    // Bounds check
    debug_assert!(byte_idx <= text.len());

    if byte_idx == 0 {
        0
    } else {
        let mut boundary_idx = byte_idx - 1;
        while !is_break(boundary_idx, text) {
            boundary_idx -= 1;
        }
        boundary_idx
    }
}

/// Returns the segment break after (but not including) the given byte
/// boundary.
///
/// This will return back the passed byte boundary if it is at the end of
/// the string.
#[inline]
pub fn next_break(byte_idx: usize, text: &[u8]) -> usize {
    // Bounds check
    debug_assert!(byte_idx <= text.len());

    if byte_idx == text.len() {
        text.len()
    } else {
        let mut boundary_idx = byte_idx + 1;
        while !is_break(boundary_idx, text) {
            boundary_idx += 1;
        }
        boundary_idx
    }
}

/// Finds the segment break nearest to the given byte that is not the
/// left or right edge of the text.
///
/// There is only one circumstance where the left or right edge will be
/// returned: if the entire text is a single unbroken segment, then the
/// right edge of the text is returned.
#[inline]
pub fn nearest_internal_break(byte_idx: usize, text: &[u8]) -> usize {
    // Bounds check
    debug_assert!(byte_idx <= text.len());

    // Find the two nearest segment boundaries
    let left = if is_break(byte_idx, text) && byte_idx != text.len() {
        byte_idx
    } else {
        prev_break(byte_idx, text)
    };
    let right = next_break(byte_idx, text);

    // Otherwise, return the closest of left and right that isn't the
    // start or end of the string
    if left == 0 || (right != text.len() && (byte_idx - left) >= (right - byte_idx)) {
        return right;
    } else {
        return left;
    }
}

#[inline]
pub fn find_good_split(byte_idx: usize, text: &[u8], bias_left: bool) -> usize {
    // Bounds check
    debug_assert!(byte_idx <= text.len());

    if is_break(byte_idx, text) {
        byte_idx
    } else {
        let prev = prev_break(byte_idx, text);
        let next = next_break(byte_idx, text);
        if bias_left {
            if prev > 0 {
                prev
            } else {
                next
            }
        } else {
            #[allow(clippy::collapsible_if)] // More readable this way
            if next < text.len() {
                next
            } else {
                prev
            }
        }
    }
}

//===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_segmenter_01() {
        let text = b"Hello world!\r\nHow's it going?";

        assert!(is_break(0, b""));
        assert!(is_break(0, text));
        assert!(is_break(12, text));
        assert!(!is_break(13, text));
        assert!(is_break(14, text));
        assert!(is_break(19, text));
    }

    #[test]
    fn crlf_segmenter_02() {
        let l = b"Hello world!\r";
        let r = b"\nHow's it going?";

        assert!(!seam_is_break(l, r));
        assert!(!seam_is_break(l, b"\n"));
        assert!(!seam_is_break(b"\r", r));
        assert!(!seam_is_break(b"\r", b"\n"));
        assert!(seam_is_break(r, l));
        assert!(seam_is_break(b"\n", b"\r"));
    }

    #[test]
    fn nearest_internal_break_01() {
        let text = b"Hello world!";
        assert_eq!(1, nearest_internal_break(0, text));
        assert_eq!(6, nearest_internal_break(6, text));
        assert_eq!(11, nearest_internal_break(12, text));
    }

    #[test]
    fn nearest_internal_break_02() {
        let text = b"Hello\r\n world!";
        assert_eq!(5, nearest_internal_break(5, text));
        assert_eq!(7, nearest_internal_break(6, text));
        assert_eq!(7, nearest_internal_break(7, text));
    }

    #[test]
    fn nearest_internal_break_03() {
        let text = b"\r\nHello world!\r\n";
        assert_eq!(2, nearest_internal_break(0, text));
        assert_eq!(2, nearest_internal_break(1, text));
        assert_eq!(2, nearest_internal_break(2, text));
        assert_eq!(14, nearest_internal_break(14, text));
        assert_eq!(14, nearest_internal_break(15, text));
        assert_eq!(14, nearest_internal_break(16, text));
    }

    #[test]
    fn nearest_internal_break_04() {
        let text = b"\r\n";
        assert_eq!(2, nearest_internal_break(0, text));
        assert_eq!(2, nearest_internal_break(1, text));
        assert_eq!(2, nearest_internal_break(2, text));
    }

    #[test]
    fn is_break_01() {
        let text = b"\n\r\n\r\n\r\n\r\n\r\n\r";

        assert!(is_break(0, text));
        assert!(is_break(12, text));
        assert!(is_break(3, text));
        assert!(!is_break(6, text));
    }

    #[test]
    fn seam_is_break_01() {
        let text1 = b"\r\n\r\n\r\n";
        let text2 = b"\r\n\r\n";

        assert!(seam_is_break(text1, text2));
    }

    #[test]
    fn seam_is_break_02() {
        let text1 = b"\r\n\r\n\r";
        let text2 = b"\n\r\n\r\n";

        assert!(!seam_is_break(text1, text2));
    }
}
