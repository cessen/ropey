#[inline]
pub fn is_break(byte_idx: usize, text: &str) -> bool {
    debug_assert!(byte_idx <= text.len());

    let bytes = text.as_bytes();
    (bytes[byte_idx - 1] != 0x0D) | (bytes[byte_idx] != 0x0A)
}

#[inline]
pub fn seam_is_break(left: &str, right: &str) -> bool {
    (left.as_bytes()[left.len() - 1] != 0x0D) | (right.as_bytes()[0] != 0x0A)
}

/// Makes sure that special cases are handled correctly.
#[inline]
pub fn is_break_checked(byte_idx: usize, text: &str) -> bool {
    if !text.is_char_boundary(byte_idx) {
        false
    } else if byte_idx == 0 || byte_idx == text.len() {
        true
    } else {
        is_break(byte_idx, text)
    }
}

/// Makes sure that special cases are handled correctly.
#[inline]
pub fn seam_is_break_checked(left: &str, right: &str) -> bool {
    debug_assert!(!left.is_empty() && !right.is_empty());
    seam_is_break(left, right)
}

/// Returns the segment break before (but not including) the given byte
/// boundary.
///
/// This will return back the passed byte boundary if it is at the start
/// of the string.
#[inline]
pub fn prev_break(byte_idx: usize, text: &str) -> usize {
    // Bounds check
    debug_assert!(byte_idx <= text.len());

    let mut boundary_idx = byte_idx;
    while boundary_idx > 0 {
        // Find previous codepoint boundary
        boundary_idx -= 1;
        while !text.is_char_boundary(boundary_idx) {
            boundary_idx -= 1;
        }

        // Check if it's a segment break
        if is_break_checked(boundary_idx, text) {
            break;
        }
    }

    boundary_idx
}

/// Returns the segment break after (but not including) the given byte
/// boundary.
///
/// This will return back the passed byte boundary if it is at the end of
/// the string.
#[inline]
pub fn next_break(byte_idx: usize, text: &str) -> usize {
    // Bounds check
    debug_assert!(byte_idx <= text.len());

    let mut boundary_idx = byte_idx;
    while boundary_idx < text.len() {
        // Find next codepoint boundary
        boundary_idx += 1;
        while !text.is_char_boundary(boundary_idx) {
            boundary_idx += 1;
        }

        // Check if it's a segment break
        if is_break_checked(boundary_idx, text) {
            break;
        }
    }

    boundary_idx
}

/// Finds the segment break nearest to the given byte that is not the
/// left or right edge of the text.
///
/// There is only one circumstance where the left or right edge will be
/// returned: if the entire text is a single unbroken segment, then the
/// right edge of the text is returned.
#[inline]
pub fn nearest_internal_break(byte_idx: usize, text: &str) -> usize {
    // Bounds check
    debug_assert!(byte_idx <= text.len());

    // Find codepoint boundary
    let mut boundary_idx = byte_idx;
    while !text.is_char_boundary(boundary_idx) {
        boundary_idx -= 1;
    }

    // Find the two nearest segment boundaries
    let left = if is_break_checked(boundary_idx, text) && boundary_idx != text.len() {
        boundary_idx
    } else {
        prev_break(boundary_idx, text)
    };
    let right = next_break(boundary_idx, text);

    // Otherwise, return the closest of left and right that isn't the
    // start or end of the string
    if left == 0 || (right != text.len() && (byte_idx - left) >= (right - byte_idx)) {
        return right;
    } else {
        return left;
    }
}

#[inline]
pub fn find_good_split(byte_idx: usize, text: &str, bias_left: bool) -> usize {
    if is_break_checked(byte_idx, text) {
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
        let text = "Hello world!\r\nHow's it going?";

        assert!(is_break_checked(0, ""));
        assert!(is_break_checked(0, text));
        assert!(is_break_checked(12, text));
        assert!(!is_break_checked(13, text));
        assert!(is_break_checked(14, text));
        assert!(is_break_checked(19, text));
    }

    #[test]
    fn crlf_segmenter_02() {
        let l = "Hello world!\r";
        let r = "\nHow's it going?";

        assert!(!seam_is_break_checked(l, r));
        assert!(!seam_is_break_checked(l, "\n"));
        assert!(!seam_is_break_checked("\r", r));
        assert!(!seam_is_break_checked("\r", "\n"));
        assert!(seam_is_break_checked(r, l));
        assert!(seam_is_break_checked("\n", "\r"));
    }

    #[test]
    fn nearest_internal_break_01() {
        let text = "Hello world!";
        assert_eq!(1, nearest_internal_break(0, text));
        assert_eq!(6, nearest_internal_break(6, text));
        assert_eq!(11, nearest_internal_break(12, text));
    }

    #[test]
    fn nearest_internal_break_02() {
        let text = "Hello\r\n world!";
        assert_eq!(5, nearest_internal_break(5, text));
        assert_eq!(7, nearest_internal_break(6, text));
        assert_eq!(7, nearest_internal_break(7, text));
    }

    #[test]
    fn nearest_internal_break_03() {
        let text = "\r\nHello world!\r\n";
        assert_eq!(2, nearest_internal_break(0, text));
        assert_eq!(2, nearest_internal_break(1, text));
        assert_eq!(2, nearest_internal_break(2, text));
        assert_eq!(14, nearest_internal_break(14, text));
        assert_eq!(14, nearest_internal_break(15, text));
        assert_eq!(14, nearest_internal_break(16, text));
    }

    #[test]
    fn nearest_internal_break_04() {
        let text = "\r\n";
        assert_eq!(2, nearest_internal_break(0, text));
        assert_eq!(2, nearest_internal_break(1, text));
        assert_eq!(2, nearest_internal_break(2, text));
    }

    #[test]
    fn is_break_01() {
        let text = "\n\r\n\r\n\r\n\r\n\r\n\r";

        assert!(is_break_checked(0, text));
        assert!(is_break_checked(12, text));
        assert!(is_break_checked(3, text));
        assert!(!is_break_checked(6, text));
    }

    #[test]
    fn seam_is_break_01() {
        let text1 = "\r\n\r\n\r\n";
        let text2 = "\r\n\r\n";

        assert!(seam_is_break(text1, text2));
    }

    #[test]
    fn seam_is_break_02() {
        let text1 = "\r\n\r\n\r";
        let text2 = "\n\r\n\r\n";

        assert!(!seam_is_break(text1, text2));
    }
}
