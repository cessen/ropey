//! Utility functions for utf8 string slices.
//!
//! This module provides various utility functions that operate on string
//! slices in ways compatible with Ropey.  They may be useful when building
//! additional functionality on top of Ropey.

// Get the appropriate module (if any) for sse2 types and intrinsics for the
// platform we're compiling for.
#[cfg(target_arch = "x86")]
use std::arch::x86 as sse2;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as sse2;

/// Converts from byte-index to char-index in a string slice.
///
/// If the byte is in the middle of a multi-byte char, returns the index of
/// the char that the byte belongs to.
///
/// Any past-the-end index will return the one-past-the-end char index.
///
/// Runs in O(N) time.
#[inline]
pub fn byte_to_char_idx(text: &str, byte_idx: usize) -> usize {
    let count = count_chars_in_bytes(&text.as_bytes()[0..(byte_idx + 1).min(text.len())]);
    if byte_idx < text.len() {
        count - 1
    } else {
        count
    }
}

/// Converts from byte-index to line-index in a string slice.
///
/// This is equivalent to counting the line endings before the given byte.
///
/// Any past-the-end index will return the last line index.
///
/// Runs in O(N) time.
#[inline]
pub fn byte_to_line_idx(text: &str, byte_idx: usize) -> usize {
    use crate::crlf;
    let mut byte_idx = byte_idx.min(text.len());
    while !text.is_char_boundary(byte_idx) {
        byte_idx -= 1;
    }
    let nl_count = count_line_breaks(&text[..byte_idx]);
    if crlf::is_break(byte_idx, text.as_bytes()) {
        nl_count
    } else {
        nl_count - 1
    }
}

/// Converts from char-index to byte-index in a string slice.
///
/// Any past-the-end index will return the one-past-the-end byte index.
///
/// Runs in O(N) time.
#[inline]
pub fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse2") {
            return char_to_byte_idx_inner::<sse2::__m128i>(text, char_idx);
        }
    }

    // Fallback for non-sse2 platforms.
    char_to_byte_idx_inner::<usize>(text, char_idx)
}

#[inline(always)]
fn char_to_byte_idx_inner<T: ByteChunk>(text: &str, char_idx: usize) -> usize {
    // Get `middle` so we can do more efficient chunk-based counting.
    // We can't use this to get `end`, however, because the start index of
    // `end` actually depends on the accumulating char counts during the
    // counting process.
    let (start, middle, _) = unsafe { text.as_bytes().align_to::<T>() };

    let mut byte_count = 0;
    let mut char_count = 0;

    // Take care of any unaligned bytes at the beginning.
    let mut i = 0;
    while i < start.len() && char_count <= char_idx {
        char_count += ((start[i] & 0xC0) != 0x80) as usize;
        i += 1;
    }
    byte_count += i;

    // Use chunks to count multiple bytes at once, using bit-fiddling magic.
    let mut i = 0;
    let mut acc = T::splat(0);
    let mut acc_i = 0;
    while i < middle.len() && (char_count + (T::size() * (acc_i + 1))) <= char_idx {
        acc = acc.add(middle[i].bitand(T::splat(0xc0)).cmp_eq_byte(0x80));
        acc_i += 1;
        if acc_i == T::max_acc() || (char_count + (T::size() * (acc_i + 1))) >= char_idx {
            char_count += (T::size() * acc_i) - acc.sum_bytes();
            acc_i = 0;
            acc = T::splat(0);
        }
        i += 1;
    }
    char_count += (T::size() * acc_i) - acc.sum_bytes();
    byte_count += i * T::size();

    // Take care of any unaligned bytes at the end.
    let end = &text.as_bytes()[byte_count..];
    let mut i = 0;
    while i < end.len() && char_count <= char_idx {
        char_count += ((end[i] & 0xC0) != 0x80) as usize;
        i += 1;
    }
    byte_count += i;

    // Finish up
    if byte_count == text.len() && char_count <= char_idx {
        byte_count
    } else {
        byte_count - 1
    }
}

/// Converts from char-index to line-index in a string slice.
///
/// This is equivalent to counting the line endings before the given char.
///
/// Any past-the-end index will return the last line index.
///
/// Runs in O(N) time.
#[inline]
pub fn char_to_line_idx(text: &str, char_idx: usize) -> usize {
    byte_to_line_idx(text, char_to_byte_idx(text, char_idx))
}

/// Converts from line-index to byte-index in a string slice.
///
/// More specifically, this returns the index of the first byte of the given
/// line.
///
/// Any past-the-end index will return the one-past-the-end byte index.
///
/// Runs in O(N) time.
#[inline]
pub fn line_to_byte_idx(text: &str, line_idx: usize) -> usize {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse2") {
            return line_to_byte_idx_inner::<sse2::__m128i>(text, line_idx);
        }
    }

    // Fallback for non-sse2 platforms.
    line_to_byte_idx_inner::<usize>(text, line_idx)
}

#[inline(always)]
fn line_to_byte_idx_inner<T: ByteChunk>(text: &str, line_idx: usize) -> usize {
    let mut bytes = text.as_bytes();
    let mut line_break_count = 0;

    // Handle unaligned bytes at the start.
    let aligned_idx = alignment_diff::<T>(bytes);
    if aligned_idx > 0 {
        let result = count_line_breaks_up_to(bytes, aligned_idx, line_idx);
        line_break_count += result.0;
        bytes = &bytes[result.1..];
    }

    // Count line breaks in big chunks.
    if alignment_diff::<T>(bytes) == 0 {
        while bytes.len() >= T::size() {
            // Unsafe because the called function depends on correct alignment.
            let tmp = unsafe { count_line_breaks_in_chunk_from_ptr::<T>(bytes) }.sum_bytes();
            if tmp + line_break_count >= line_idx {
                break;
            }
            line_break_count += tmp;

            bytes = &bytes[T::size()..];
        }
    }

    // Handle unaligned bytes at the end.
    let result = count_line_breaks_up_to(bytes, bytes.len(), line_idx - line_break_count);
    bytes = &bytes[result.1..];

    // Finish up
    let mut byte_idx = text.len() - bytes.len();
    while !text.is_char_boundary(byte_idx) {
        byte_idx += 1;
    }
    byte_idx
}

/// Converts from line-index to char-index in a string slice.
///
/// More specifically, this returns the index of the first char of the given
/// line.
///
/// Any past-the-end index will return the one-past-the-end char index.
///
/// Runs in O(N) time.
#[inline]
pub fn line_to_char_idx(text: &str, line_idx: usize) -> usize {
    byte_to_char_idx(text, line_to_byte_idx(text, line_idx))
}

// /// Counts the utf16 surrogate pairs that would be in `text` if it were encoded
// /// as utf16.
// pub(crate) fn count_utf16_surrogates_slow(text: &str) -> usize {
//     let mut utf16_surrogate_count = 0;
//
//     for byte in text.bytes() {
//         utf16_surrogate_count += ((byte & 0xf0) == 0xf0) as usize;
//     }
//
//     utf16_surrogate_count
// }

/// Counts the utf16 surrogate pairs that would be in `text` if it were encoded
/// as utf16.
#[inline]
pub(crate) fn count_utf16_surrogates(text: &str) -> usize {
    count_utf16_surrogates_in_bytes(text.as_bytes())
}

#[inline]
pub(crate) fn count_utf16_surrogates_in_bytes(text: &[u8]) -> usize {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse2") {
            return count_utf16_surrogates_internal::<sse2::__m128i>(text);
        }
    }

    // Fallback for non-sse2 platforms.
    count_utf16_surrogates_internal::<usize>(text)
}

#[inline(always)]
fn count_utf16_surrogates_internal<T: ByteChunk>(text: &[u8]) -> usize {
    // Get `middle` for more efficient chunk-based counting.
    let (start, middle, end) = unsafe { text.align_to::<T>() };

    let mut utf16_surrogate_count = 0;

    // Take care of unaligned bytes at the beginning.
    for byte in start.iter() {
        utf16_surrogate_count += ((byte & 0xf0) == 0xf0) as usize;
    }

    // Take care of the middle bytes in big chunks.
    let mut i = 0;
    let mut acc = T::splat(0);
    for chunk in middle.iter() {
        let tmp = chunk.bitand(T::splat(0xf0)).cmp_eq_byte(0xf0);
        acc = acc.add(tmp);
        i += 1;
        if i == T::max_acc() {
            i = 0;
            utf16_surrogate_count += acc.sum_bytes();
            acc = T::splat(0);
        }
    }
    utf16_surrogate_count += acc.sum_bytes();

    // Take care of unaligned bytes at the end.
    for byte in end.iter() {
        utf16_surrogate_count += ((byte & 0xf0) == 0xf0) as usize;
    }

    utf16_surrogate_count
}

#[inline(always)]
pub(crate) fn byte_to_utf16_surrogate_idx(text: &str, byte_idx: usize) -> usize {
    count_utf16_surrogates(&text[..byte_idx])
}

#[inline(always)]
pub(crate) fn utf16_code_unit_to_char_idx(text: &str, utf16_idx: usize) -> usize {
    // TODO: optimized version.  This is pretty slow.  It isn't expected to be
    // used in performance critical functionality, so this isn't urgent.  But
    // might as well make it faster when we get the chance.
    let mut char_i = 0;
    let mut utf16_i = 0;
    for c in text.chars() {
        if utf16_idx <= utf16_i {
            break;
        }
        char_i += 1;
        utf16_i += c.len_utf16();
    }

    if utf16_idx < utf16_i {
        char_i -= 1;
    }

    char_i
}

//===========================================================================
// Internal
//===========================================================================

/// Returns the byte position just after the second-to-last line break
/// in `text`, or zero of there is no second-to-last line break.
///
/// This function is narrow in scope, only being used for iterating
/// backwards over the lines of a `str`.
pub(crate) fn prev_line_end_char_idx(text: &str) -> usize {
    let mut itr = text.bytes().enumerate().rev();

    let first_byte = if let Some((_, byte)) = itr.next() {
        byte
    } else {
        return 0;
    };

    while let Some((idx, byte)) = itr.next() {
        match byte {
            0x0A | 0x0B | 0x0C => {
                return idx + 1;
            }
            0x0D => {
                if first_byte != 0x0A {
                    return idx + 1;
                }
            }
            0x85 => {
                if let Some((_, 0xC2)) = itr.next() {
                    return idx + 1;
                }
            }
            0xA8 | 0xA9 => {
                if let Some((_, 0x80)) = itr.next() {
                    if let Some((_, 0xE2)) = itr.next() {
                        return idx + 1;
                    }
                }
            }
            _ => {}
        }
    }

    return 0;
}

/// Returns whether the given string ends in a line break or not.
#[inline]
pub(crate) fn ends_with_line_break(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    // Find the starting boundary of the last codepoint.
    let mut i = text.len() - 1;
    while !text.is_char_boundary(i) {
        i -= 1;
    }

    // Check if the last codepoint is a line break.
    match &text[i..] {
        "\u{000A}" | "\u{000B}" | "\u{000C}" | "\u{000D}" | "\u{0085}" | "\u{2028}"
        | "\u{2029}" => true,
        _ => false,
    }
}

/// Uses bit-fiddling magic to count utf8 chars really quickly.
/// We actually count the number of non-starting utf8 bytes, since
/// they have a consistent starting two-bit pattern.  We then
/// subtract from the byte length of the text to get the final
/// count.
#[inline]
pub(crate) fn count_chars(text: &str) -> usize {
    count_chars_in_bytes(text.as_bytes())
}

#[inline]
pub(crate) fn count_chars_in_bytes(text: &[u8]) -> usize {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse2") {
            return count_chars_internal::<sse2::__m128i>(text);
        }
    }

    // Fallback for non-sse2 platforms.
    count_chars_internal::<usize>(text)
}

#[inline(always)]
fn count_chars_internal<T: ByteChunk>(text: &[u8]) -> usize {
    // Get `middle` for more efficient chunk-based counting.
    let (start, middle, end) = unsafe { text.align_to::<T>() };

    let mut inv_count = 0;

    // Take care of unaligned bytes at the beginning.
    for byte in start.iter() {
        inv_count += ((byte & 0xC0) == 0x80) as usize;
    }

    // Take care of the middle bytes in big chunks.
    let mut i = 0;
    let mut acc = T::splat(0);
    for chunk in middle.iter() {
        let tmp = chunk.bitand(T::splat(0xc0)).cmp_eq_byte(0x80);
        acc = acc.add(tmp);
        i += 1;
        if i == T::max_acc() {
            i = 0;
            inv_count += acc.sum_bytes();
            acc = T::splat(0);
        }
    }
    inv_count += acc.sum_bytes();

    // Take care of unaligned bytes at the end.
    for byte in end.iter() {
        inv_count += ((byte & 0xC0) == 0x80) as usize;
    }

    text.len() - inv_count
}

/// Uses bit-fiddling magic to count line breaks really quickly.
///
/// The following unicode sequences are considered newlines by this function:
/// - u{000A}        (Line Feed)
/// - u{000B}        (Vertical Tab)
/// - u{000C}        (Form Feed)
/// - u{000D}        (Carriage Return)
/// - u{000D}u{000A} (Carriage Return + Line Feed)
/// - u{0085}        (Next Line)
/// - u{2028}        (Line Separator)
/// - u{2029}        (Paragraph Separator)
#[inline]
pub(crate) fn count_line_breaks(text: &str) -> usize {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse2") {
            return count_line_breaks_internal::<sse2::__m128i>(text);
        }
    }

    // Fallback for non-sse2 platforms.
    count_line_breaks_internal::<usize>(text)
}

#[inline(always)]
fn count_line_breaks_internal<T: ByteChunk>(text: &str) -> usize {
    let mut bytes = text.as_bytes();
    let mut count = 0;

    // Handle unaligned bytes at the start.
    let aligned_idx = alignment_diff::<T>(bytes);
    if aligned_idx > 0 {
        let result = count_line_breaks_up_to(bytes, aligned_idx, bytes.len());
        count += result.0;
        bytes = &bytes[result.1..];
    }

    // Count line breaks in big chunks.
    let mut i = 0;
    let mut acc = T::splat(0);
    while bytes.len() >= T::size() {
        // Unsafe because the called function depends on correct alignment.
        acc = acc.add(unsafe { count_line_breaks_in_chunk_from_ptr::<T>(bytes) });
        i += 1;
        if i == T::max_acc() {
            i = 0;
            count += acc.sum_bytes();
            acc = T::splat(0);
        }
        bytes = &bytes[T::size()..];
    }
    count += acc.sum_bytes();

    // Handle unaligned bytes at the end.
    count += count_line_breaks_up_to(bytes, bytes.len(), bytes.len()).0;

    count
}

/// Used internally in the line-break counting functions.
///
/// Counts line breaks a byte at a time up to a maximum number of bytes and
/// line breaks, and returns the counted lines and how many bytes were processed.
#[inline(always)]
#[allow(clippy::if_same_then_else)]
fn count_line_breaks_up_to(bytes: &[u8], max_bytes: usize, max_breaks: usize) -> (usize, usize) {
    let mut ptr = 0;
    let mut count = 0;
    while ptr < max_bytes && count < max_breaks {
        let byte = bytes[ptr];

        // Handle u{000A}, u{000B}, u{000C}, and u{000D}
        if (byte <= 0x0D) && (byte >= 0x0A) {
            count += 1;

            // Check for CRLF and and subtract 1 if it is,
            // since it will be caught in the next iteration
            // with the LF.
            if byte == 0x0D && (ptr + 1) < bytes.len() && bytes[ptr + 1] == 0x0A {
                count -= 1;
            }
        }
        // Handle u{0085}
        else if byte == 0xC2 && (ptr + 1) < bytes.len() && bytes[ptr + 1] == 0x85 {
            count += 1;
        }
        // Handle u{2028} and u{2029}
        else if byte == 0xE2
            && (ptr + 2) < bytes.len()
            && bytes[ptr + 1] == 0x80
            && (bytes[ptr + 2] >> 1) == 0x54
        {
            count += 1;
        }

        ptr += 1;
    }

    (count, ptr)
}

/// Used internally in the line-break counting functions.
///
/// The start of `bytes` MUST be aligned as type T, and `bytes` MUST be at
/// least as large (in bytes) as T.  If these invariants are not met, bad
/// things could potentially happen.  Hence why this function is unsafe.
#[inline(always)]
unsafe fn count_line_breaks_in_chunk_from_ptr<T: ByteChunk>(bytes: &[u8]) -> T {
    let c = {
        // The only unsafe bits of the function are in this block.
        debug_assert_eq!(bytes.align_to::<T>().0.len(), 0);
        debug_assert!(bytes.len() >= T::size());
        // This unsafe cast is for performance reasons: going through e.g.
        // `align_to()` results in a significant drop in performance.
        *(bytes.as_ptr() as *const T)
    };
    let end_i = T::size();

    let mut acc = T::splat(0);

    // Calculate the flags we're going to be working with.
    let nl_1_flags = c.cmp_eq_byte(0xC2);
    let sp_1_flags = c.cmp_eq_byte(0xE2);
    let all_flags = c.bytes_between_127(0x09, 0x0E);
    let cr_flags = c.cmp_eq_byte(0x0D);

    // Next Line: u{0085}
    if !nl_1_flags.is_zero() {
        let nl_2_flags = c.cmp_eq_byte(0x85).shift_back_lex(1);
        let flags = nl_1_flags.bitand(nl_2_flags);
        acc = acc.add(flags);

        // Handle ending boundary
        if bytes.len() > end_i && bytes[end_i - 1] == 0xC2 && bytes[end_i] == 0x85 {
            acc = acc.inc_nth_from_end_lex_byte(0);
        }
    }

    // Line Separator:      u{2028}
    // Paragraph Separator: u{2029}
    if !sp_1_flags.is_zero() {
        let sp_2_flags = c.cmp_eq_byte(0x80).shift_back_lex(1).bitand(sp_1_flags);
        if !sp_2_flags.is_zero() {
            let sp_3_flags = c
                .shr(1)
                .bitand(T::splat(!0x80))
                .cmp_eq_byte(0x54)
                .shift_back_lex(2);
            let sp_flags = sp_2_flags.bitand(sp_3_flags);
            acc = acc.add(sp_flags);
        }

        // Handle ending boundary
        if bytes.len() > end_i
            && bytes[end_i - 2] == 0xE2
            && bytes[end_i - 1] == 0x80
            && (bytes[end_i] >> 1) == 0x54
        {
            acc = acc.inc_nth_from_end_lex_byte(1);
        } else if bytes.len() > (end_i + 1)
            && bytes[end_i - 1] == 0xE2
            && bytes[end_i] == 0x80
            && (bytes[end_i + 1] >> 1) == 0x54
        {
            acc = acc.inc_nth_from_end_lex_byte(0);
        }
    }

    // Line Feed:                   u{000A}
    // Vertical Tab:                u{000B}
    // Form Feed:                   u{000C}
    // Carriage Return:             u{000D}
    // Carriage Return + Line Feed: u{000D}u{000A}
    acc = acc.add(all_flags);
    if !cr_flags.is_zero() {
        // Handle CRLF
        let lf_flags = c.cmp_eq_byte(0x0A);
        let crlf_flags = cr_flags.bitand(lf_flags.shift_back_lex(1));
        acc = acc.sub(crlf_flags);
        if bytes.len() > end_i && bytes[end_i - 1] == 0x0D && bytes[end_i] == 0x0A {
            acc = acc.dec_last_lex_byte();
        }
    }

    acc
}

/// Returns the alignment difference between the start of `bytes` and the
/// type `T`.
///
/// Or put differently: returns how many bytes into `bytes` you need to walk
/// to reach the alignment of `T` in memory.
///
/// Will return 0 if already aligned at the start, and will return the length
/// of `bytes` if alignment is beyond the end of `bytes`.
#[inline(always)]
fn alignment_diff<T>(bytes: &[u8]) -> usize {
    let alignment = std::mem::align_of::<T>();
    let ptr = bytes.as_ptr() as usize;
    (alignment - ((ptr - 1) & (alignment - 1)) - 1).min(bytes.len())
}

//======================================================================

/// Interface for working with chunks of bytes at a time, providing the
/// operations needed for the functionality in str_utils.
trait ByteChunk: Copy + Clone + std::fmt::Debug {
    /// Returns the size of the chunk in bytes.
    fn size() -> usize;

    /// Returns the maximum number of iterations the chunk can accumulate
    /// before sum_bytes() becomes inaccurate.
    fn max_acc() -> usize;

    /// Creates a new chunk with all bytes set to n.
    fn splat(n: u8) -> Self;

    /// Returns whether all bytes are zero or not.
    fn is_zero(&self) -> bool;

    /// Shifts bytes back lexographically by n bytes.
    fn shift_back_lex(&self, n: usize) -> Self;

    /// Shifts bits to the right by n bits.
    fn shr(&self, n: usize) -> Self;

    /// Compares bytes for equality with the given byte.
    ///
    /// Bytes that are equal are set to 1, bytes that are not
    /// are set to 0.
    fn cmp_eq_byte(&self, byte: u8) -> Self;

    /// Compares bytes to see if they're in the non-inclusive range (a, b),
    /// where a < b <= 127.
    ///
    /// Bytes in the range are set to 1, bytes not in the range are set to 0.
    fn bytes_between_127(&self, a: u8, b: u8) -> Self;

    /// Performs a bitwise and on two chunks.
    fn bitand(&self, other: Self) -> Self;

    /// Adds the bytes of two chunks together.
    fn add(&self, other: Self) -> Self;

    /// Subtracts other's bytes from this chunk.
    fn sub(&self, other: Self) -> Self;

    /// Increments the nth-from-last lexographic byte by 1.
    fn inc_nth_from_end_lex_byte(&self, n: usize) -> Self;

    /// Decrements the last lexographic byte by 1.
    fn dec_last_lex_byte(&self) -> Self;

    /// Returns the sum of all bytes in the chunk.
    fn sum_bytes(&self) -> usize;
}

impl ByteChunk for usize {
    #[inline(always)]
    fn size() -> usize {
        std::mem::size_of::<usize>()
    }

    #[inline(always)]
    fn max_acc() -> usize {
        (256 / std::mem::size_of::<usize>()) - 1
    }

    #[inline(always)]
    fn splat(n: u8) -> Self {
        const ONES: usize = std::usize::MAX / 0xFF;
        ONES * n as usize
    }

    #[inline(always)]
    fn is_zero(&self) -> bool {
        *self == 0
    }

    #[inline(always)]
    fn shift_back_lex(&self, n: usize) -> Self {
        if cfg!(target_endian = "little") {
            *self >> (n * 8)
        } else {
            *self << (n * 8)
        }
    }

    #[inline(always)]
    fn shr(&self, n: usize) -> Self {
        *self >> n
    }

    #[inline(always)]
    fn cmp_eq_byte(&self, byte: u8) -> Self {
        const ONES: usize = std::usize::MAX / 0xFF;
        const ONES_HIGH: usize = ONES << 7;
        let word = *self ^ (byte as usize * ONES);
        (!(((word & !ONES_HIGH) + !ONES_HIGH) | word) & ONES_HIGH) >> 7
    }

    #[inline(always)]
    fn bytes_between_127(&self, a: u8, b: u8) -> Self {
        const ONES: usize = std::usize::MAX / 0xFF;
        const ONES_HIGH: usize = ONES << 7;
        let tmp = *self & (ONES * 127);
        (((ONES * (127 + b as usize) - tmp) & !*self & (tmp + (ONES * (127 - a as usize))))
            & ONES_HIGH)
            >> 7
    }

    #[inline(always)]
    fn bitand(&self, other: Self) -> Self {
        *self & other
    }

    #[inline(always)]
    fn add(&self, other: Self) -> Self {
        *self + other
    }

    #[inline(always)]
    fn sub(&self, other: Self) -> Self {
        *self - other
    }

    #[inline(always)]
    fn inc_nth_from_end_lex_byte(&self, n: usize) -> Self {
        if cfg!(target_endian = "little") {
            *self + (1 << ((Self::size() - 1 - n) * 8))
        } else {
            *self + (1 << (n * 8))
        }
    }

    #[inline(always)]
    fn dec_last_lex_byte(&self) -> Self {
        if cfg!(target_endian = "little") {
            *self - (1 << ((Self::size() - 1) * 8))
        } else {
            *self - 1
        }
    }

    #[inline(always)]
    fn sum_bytes(&self) -> usize {
        const ONES: usize = std::usize::MAX / 0xFF;
        self.wrapping_mul(ONES) >> ((Self::size() - 1) * 8)
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl ByteChunk for sse2::__m128i {
    #[inline(always)]
    fn size() -> usize {
        std::mem::size_of::<sse2::__m128i>()
    }

    #[inline(always)]
    fn max_acc() -> usize {
        (256 / 8) - 1
    }

    #[inline(always)]
    fn splat(n: u8) -> Self {
        unsafe { sse2::_mm_set1_epi8(n as i8) }
    }

    #[inline(always)]
    fn is_zero(&self) -> bool {
        let tmp = unsafe { std::mem::transmute::<Self, (u64, u64)>(*self) };
        tmp.0 == 0 && tmp.1 == 0
    }

    #[inline(always)]
    fn shift_back_lex(&self, n: usize) -> Self {
        match n {
            0 => *self,
            1 => unsafe { sse2::_mm_srli_si128(*self, 1) },
            2 => unsafe { sse2::_mm_srli_si128(*self, 2) },
            3 => unsafe { sse2::_mm_srli_si128(*self, 3) },
            4 => unsafe { sse2::_mm_srli_si128(*self, 4) },
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn shr(&self, n: usize) -> Self {
        match n {
            0 => *self,
            1 => unsafe { sse2::_mm_srli_epi64(*self, 1) },
            2 => unsafe { sse2::_mm_srli_epi64(*self, 2) },
            3 => unsafe { sse2::_mm_srli_epi64(*self, 3) },
            4 => unsafe { sse2::_mm_srli_epi64(*self, 4) },
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn cmp_eq_byte(&self, byte: u8) -> Self {
        let tmp = unsafe { sse2::_mm_cmpeq_epi8(*self, Self::splat(byte)) };
        unsafe { sse2::_mm_and_si128(tmp, Self::splat(1)) }
    }

    #[inline(always)]
    fn bytes_between_127(&self, a: u8, b: u8) -> Self {
        let tmp1 = unsafe { sse2::_mm_cmpgt_epi8(*self, Self::splat(a)) };
        let tmp2 = unsafe { sse2::_mm_cmplt_epi8(*self, Self::splat(b)) };
        let tmp3 = unsafe { sse2::_mm_and_si128(tmp1, tmp2) };
        unsafe { sse2::_mm_and_si128(tmp3, Self::splat(1)) }
    }

    #[inline(always)]
    fn bitand(&self, other: Self) -> Self {
        unsafe { sse2::_mm_and_si128(*self, other) }
    }

    #[inline(always)]
    fn add(&self, other: Self) -> Self {
        unsafe { sse2::_mm_add_epi8(*self, other) }
    }

    #[inline(always)]
    fn sub(&self, other: Self) -> Self {
        unsafe { sse2::_mm_sub_epi8(*self, other) }
    }

    #[inline(always)]
    fn inc_nth_from_end_lex_byte(&self, n: usize) -> Self {
        let mut tmp = unsafe { std::mem::transmute::<Self, [u8; 16]>(*self) };
        tmp[15 - n] += 1;
        unsafe { std::mem::transmute::<[u8; 16], Self>(tmp) }
    }

    #[inline(always)]
    fn dec_last_lex_byte(&self) -> Self {
        let mut tmp = unsafe { std::mem::transmute::<Self, [u8; 16]>(*self) };
        tmp[15] -= 1;
        unsafe { std::mem::transmute::<[u8; 16], Self>(tmp) }
    }

    #[inline(always)]
    fn sum_bytes(&self) -> usize {
        const ONES: u64 = std::u64::MAX / 0xFF;
        let tmp = unsafe { std::mem::transmute::<Self, (u64, u64)>(*self) };
        let a = tmp.0.wrapping_mul(ONES) >> (7 * 8);
        let b = tmp.1.wrapping_mul(ONES) >> (7 * 8);
        (a + b) as usize
    }
}

// AVX2, currently unused because it actually runs slower than SSE2 for most
// of the things we're doing, oddly.
// impl ByteChunk for x86_64::__m256i {
//     #[inline(always)]
//     fn size() -> usize {
//         std::mem::size_of::<x86_64::__m256i>()
//     }

//     #[inline(always)]
//     fn max_acc() -> usize {
//         (256 / 8) - 1
//     }

//     #[inline(always)]
//     fn splat(n: u8) -> Self {
//         unsafe { x86_64::_mm256_set1_epi8(n as i8) }
//     }

//     #[inline(always)]
//     fn is_zero(&self) -> bool {
//         let tmp = unsafe { std::mem::transmute::<Self, (u64, u64, u64, u64)>(*self) };
//         tmp.0 == 0 && tmp.1 == 0 && tmp.2 == 0 && tmp.3 == 0
//     }

//     #[inline(always)]
//     fn shift_back_lex(&self, n: usize) -> Self {
//         let mut tmp1;
//         let tmp2 = unsafe { std::mem::transmute::<Self, [u8; 32]>(*self) };
//         match n {
//             0 => return *self,
//             1 => {
//                 tmp1 = unsafe {
//                     std::mem::transmute::<Self, [u8; 32]>(x86_64::_mm256_srli_si256(*self, 1))
//                 };
//                 tmp1[15] = tmp2[16];
//             }
//             2 => {
//                 tmp1 = unsafe {
//                     std::mem::transmute::<Self, [u8; 32]>(x86_64::_mm256_srli_si256(*self, 2))
//                 };
//                 tmp1[15] = tmp2[17];
//                 tmp1[14] = tmp2[16];
//             }
//             _ => unreachable!(),
//         }
//         unsafe { std::mem::transmute::<[u8; 32], Self>(tmp1) }
//     }

//     #[inline(always)]
//     fn shr(&self, n: usize) -> Self {
//         match n {
//             0 => *self,
//             1 => unsafe { x86_64::_mm256_srli_epi64(*self, 1) },
//             2 => unsafe { x86_64::_mm256_srli_epi64(*self, 2) },
//             3 => unsafe { x86_64::_mm256_srli_epi64(*self, 3) },
//             4 => unsafe { x86_64::_mm256_srli_epi64(*self, 4) },
//             _ => unreachable!(),
//         }
//     }

//     #[inline(always)]
//     fn cmp_eq_byte(&self, byte: u8) -> Self {
//         let tmp = unsafe { x86_64::_mm256_cmpeq_epi8(*self, Self::splat(byte)) };
//         unsafe { x86_64::_mm256_and_si256(tmp, Self::splat(1)) }
//     }

//     #[inline(always)]
//     fn bytes_between_127(&self, a: u8, b: u8) -> Self {
//         let tmp2 = unsafe { x86_64::_mm256_cmpgt_epi8(*self, Self::splat(a)) };
//         let tmp1 = {
//             let tmp = unsafe { x86_64::_mm256_cmpgt_epi8(*self, Self::splat(b + 1)) };
//             unsafe { x86_64::_mm256_andnot_si256(tmp, Self::splat(0xff)) }
//         };
//         let tmp3 = unsafe { x86_64::_mm256_and_si256(tmp1, tmp2) };
//         unsafe { x86_64::_mm256_and_si256(tmp3, Self::splat(1)) }
//     }

//     #[inline(always)]
//     fn bitand(&self, other: Self) -> Self {
//         unsafe { x86_64::_mm256_and_si256(*self, other) }
//     }

//     #[inline(always)]
//     fn add(&self, other: Self) -> Self {
//         unsafe { x86_64::_mm256_add_epi8(*self, other) }
//     }

//     #[inline(always)]
//     fn sub(&self, other: Self) -> Self {
//         unsafe { x86_64::_mm256_sub_epi8(*self, other) }
//     }

//     #[inline(always)]
//     fn inc_nth_from_end_lex_byte(&self, n: usize) -> Self {
//         let mut tmp = unsafe { std::mem::transmute::<Self, [u8; 32]>(*self) };
//         tmp[31 - n] += 1;
//         unsafe { std::mem::transmute::<[u8; 32], Self>(tmp) }
//     }

//     #[inline(always)]
//     fn dec_last_lex_byte(&self) -> Self {
//         let mut tmp = unsafe { std::mem::transmute::<Self, [u8; 32]>(*self) };
//         tmp[31] -= 1;
//         unsafe { std::mem::transmute::<[u8; 32], Self>(tmp) }
//     }

//     #[inline(always)]
//     fn sum_bytes(&self) -> usize {
//         const ONES: u64 = std::u64::MAX / 0xFF;
//         let tmp = unsafe { std::mem::transmute::<Self, (u64, u64, u64, u64)>(*self) };
//         let a = tmp.0.wrapping_mul(ONES) >> (7 * 8);
//         let b = tmp.1.wrapping_mul(ONES) >> (7 * 8);
//         let c = tmp.2.wrapping_mul(ONES) >> (7 * 8);
//         let d = tmp.3.wrapping_mul(ONES) >> (7 * 8);
//         (a + b + c + d) as usize
//     }
// }

//======================================================================

/// An iterator that yields the byte indices of line breaks in a string.
/// A line break in this case is the point immediately *after* a newline
/// character.
///
/// The following unicode sequences are considered newlines by this function:
/// - u{000A}        (Line Feed)
/// - u{000B}        (Vertical Tab)
/// - u{000C}        (Form Feed)
/// - u{000D}        (Carriage Return)
/// - u{000D}u{000A} (Carriage Return + Line Feed)
/// - u{0085}        (Next Line)
/// - u{2028}        (Line Separator)
/// - u{2029}        (Paragraph Separator)
#[allow(unused)] // Used in tests, as reference solution.
struct LineBreakIter<'a> {
    byte_itr: std::str::Bytes<'a>,
    byte_idx: usize,
}

#[allow(unused)]
impl<'a> LineBreakIter<'a> {
    #[inline]
    fn new(text: &str) -> LineBreakIter {
        LineBreakIter {
            byte_itr: text.bytes(),
            byte_idx: 0,
        }
    }
}

impl<'a> Iterator for LineBreakIter<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        while let Some(byte) = self.byte_itr.next() {
            self.byte_idx += 1;
            // Handle u{000A}, u{000B}, u{000C}, and u{000D}
            if (byte <= 0x0D) && (byte >= 0x0A) {
                if byte == 0x0D {
                    // We're basically "peeking" here.
                    if let Some(0x0A) = self.byte_itr.clone().next() {
                        self.byte_itr.next();
                        self.byte_idx += 1;
                    }
                }
                return Some(self.byte_idx);
            }
            // Handle u{0085}
            else if byte == 0xC2 {
                self.byte_idx += 1;
                if let Some(0x85) = self.byte_itr.next() {
                    return Some(self.byte_idx);
                }
            }
            // Handle u{2028} and u{2029}
            else if byte == 0xE2 {
                self.byte_idx += 2;
                let byte2 = self.byte_itr.next().unwrap();
                let byte3 = self.byte_itr.next().unwrap() >> 1;
                if byte2 == 0x80 && byte3 == 0x54 {
                    return Some(self.byte_idx);
                }
            }
        }

        return None;
    }
}

//======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    #[test]
    fn count_chars_01() {
        let text = "Hello せかい! Hello せかい! Hello せかい! Hello せかい! Hello せかい!";

        assert_eq!(54, count_chars(text));
    }

    #[test]
    fn count_chars_02() {
        assert_eq!(100, count_chars(TEXT_LINES));
    }

    #[test]
    fn line_breaks_iter_01() {
        let text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                    There\u{2028}is something.\u{2029}";
        let mut itr = LineBreakIter::new(text);
        assert_eq!(48, text.len());
        assert_eq!(Some(1), itr.next());
        assert_eq!(Some(8), itr.next());
        assert_eq!(Some(9), itr.next());
        assert_eq!(Some(13), itr.next());
        assert_eq!(Some(17), itr.next());
        assert_eq!(Some(22), itr.next());
        assert_eq!(Some(32), itr.next());
        assert_eq!(Some(48), itr.next());
        assert_eq!(None, itr.next());
    }

    #[test]
    fn prev_line_end_char_idx_01() {
        let mut text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                        There\u{2028}is something.\u{2029}";

        assert_eq!(48, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(32, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(22, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(17, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(13, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(9, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(8, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(1, text.len());
        text = &text[..prev_line_end_char_idx(text)];
        assert_eq!(0, text.len());
    }

    #[test]
    fn count_line_breaks_01() {
        let text = "\u{000A}Hello\u{000D}\u{000A}\u{000D}せ\u{000B}か\u{000C}い\u{0085}. \
                    There\u{2028}is something.\u{2029}";
        assert_eq!(48, text.len());
        assert_eq!(8, count_line_breaks(text));
    }

    #[test]
    fn count_line_breaks_02() {
        let text = "\u{000A}Hello world!  This is a longer text.\u{000D}\u{000A}\u{000D}To better test that skipping by usize doesn't mess things up.\u{000B}Hello せかい!\u{000C}\u{0085}Yet more text.  How boring.\u{2028}Hi.\u{2029}\u{000A}Hello world!  This is a longer text.\u{000D}\u{000A}\u{000D}To better test that skipping by usize doesn't mess things up.\u{000B}Hello せかい!\u{000C}\u{0085}Yet more text.  How boring.\u{2028}Hi.\u{2029}\u{000A}Hello world!  This is a longer text.\u{000D}\u{000A}\u{000D}To better test that skipping by usize doesn't mess things up.\u{000B}Hello せかい!\u{000C}\u{0085}Yet more text.  How boring.\u{2028}Hi.\u{2029}\u{000A}Hello world!  This is a longer text.\u{000D}\u{000A}\u{000D}To better test that skipping by usize doesn't mess things up.\u{000B}Hello せかい!\u{000C}\u{0085}Yet more text.  How boring.\u{2028}Hi.\u{2029}";
        assert_eq!(count_line_breaks(text), LineBreakIter::new(text).count());
    }

    #[test]
    fn byte_to_char_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(1, byte_to_char_idx(text, 1));
        assert_eq!(6, byte_to_char_idx(text, 6));
        assert_eq!(6, byte_to_char_idx(text, 7));
        assert_eq!(6, byte_to_char_idx(text, 8));
        assert_eq!(7, byte_to_char_idx(text, 9));
        assert_eq!(7, byte_to_char_idx(text, 10));
        assert_eq!(7, byte_to_char_idx(text, 11));
        assert_eq!(8, byte_to_char_idx(text, 12));
        assert_eq!(8, byte_to_char_idx(text, 13));
        assert_eq!(8, byte_to_char_idx(text, 14));
        assert_eq!(9, byte_to_char_idx(text, 15));
        assert_eq!(10, byte_to_char_idx(text, 16));
        assert_eq!(10, byte_to_char_idx(text, 17));
        assert_eq!(10, byte_to_char_idx(text, 18));
        assert_eq!(10, byte_to_char_idx(text, 19));
    }

    #[test]
    fn byte_to_char_idx_02() {
        let text = "";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(0, byte_to_char_idx(text, 1));

        let text = "h";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(1, byte_to_char_idx(text, 1));
        assert_eq!(1, byte_to_char_idx(text, 2));

        let text = "hi";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(1, byte_to_char_idx(text, 1));
        assert_eq!(2, byte_to_char_idx(text, 2));
        assert_eq!(2, byte_to_char_idx(text, 3));
    }

    #[test]
    fn byte_to_char_idx_03() {
        let text = "せかい";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(0, byte_to_char_idx(text, 1));
        assert_eq!(0, byte_to_char_idx(text, 2));
        assert_eq!(1, byte_to_char_idx(text, 3));
        assert_eq!(1, byte_to_char_idx(text, 4));
        assert_eq!(1, byte_to_char_idx(text, 5));
        assert_eq!(2, byte_to_char_idx(text, 6));
        assert_eq!(2, byte_to_char_idx(text, 7));
        assert_eq!(2, byte_to_char_idx(text, 8));
        assert_eq!(3, byte_to_char_idx(text, 9));
        assert_eq!(3, byte_to_char_idx(text, 10));
        assert_eq!(3, byte_to_char_idx(text, 11));
        assert_eq!(3, byte_to_char_idx(text, 12));
    }

    #[test]
    fn byte_to_char_idx_04() {
        // Ascii range
        for i in 0..88 {
            assert_eq!(i, byte_to_char_idx(TEXT_LINES, i));
        }

        // Hiragana characters
        for i in 88..125 {
            assert_eq!(88 + ((i - 88) / 3), byte_to_char_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 125..130 {
            assert_eq!(100, byte_to_char_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn byte_to_line_idx_01() {
        let text = "Here\nare\nsome\nwords";
        assert_eq!(0, byte_to_line_idx(text, 0));
        assert_eq!(0, byte_to_line_idx(text, 4));
        assert_eq!(1, byte_to_line_idx(text, 5));
        assert_eq!(1, byte_to_line_idx(text, 8));
        assert_eq!(2, byte_to_line_idx(text, 9));
        assert_eq!(2, byte_to_line_idx(text, 13));
        assert_eq!(3, byte_to_line_idx(text, 14));
        assert_eq!(3, byte_to_line_idx(text, 19));
    }

    #[test]
    fn byte_to_line_idx_02() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(0, byte_to_line_idx(text, 0));
        assert_eq!(1, byte_to_line_idx(text, 1));
        assert_eq!(1, byte_to_line_idx(text, 5));
        assert_eq!(2, byte_to_line_idx(text, 6));
        assert_eq!(2, byte_to_line_idx(text, 9));
        assert_eq!(3, byte_to_line_idx(text, 10));
        assert_eq!(3, byte_to_line_idx(text, 14));
        assert_eq!(4, byte_to_line_idx(text, 15));
        assert_eq!(4, byte_to_line_idx(text, 20));
        assert_eq!(5, byte_to_line_idx(text, 21));
    }

    #[test]
    fn byte_to_line_idx_03() {
        let text = "Here\r\nare\r\nsome\r\nwords";
        assert_eq!(0, byte_to_line_idx(text, 0));
        assert_eq!(0, byte_to_line_idx(text, 4));
        assert_eq!(0, byte_to_line_idx(text, 5));
        assert_eq!(1, byte_to_line_idx(text, 6));
        assert_eq!(1, byte_to_line_idx(text, 9));
        assert_eq!(1, byte_to_line_idx(text, 10));
        assert_eq!(2, byte_to_line_idx(text, 11));
        assert_eq!(2, byte_to_line_idx(text, 15));
        assert_eq!(2, byte_to_line_idx(text, 16));
        assert_eq!(3, byte_to_line_idx(text, 17));
    }

    #[test]
    fn byte_to_line_idx_04() {
        // Line 0
        for i in 0..32 {
            assert_eq!(0, byte_to_line_idx(TEXT_LINES, i));
        }

        // Line 1
        for i in 32..59 {
            assert_eq!(1, byte_to_line_idx(TEXT_LINES, i));
        }

        // Line 2
        for i in 59..88 {
            assert_eq!(2, byte_to_line_idx(TEXT_LINES, i));
        }

        // Line 3
        for i in 88..125 {
            assert_eq!(3, byte_to_line_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 125..130 {
            assert_eq!(3, byte_to_line_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn char_to_byte_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(1, char_to_byte_idx(text, 1));
        assert_eq!(2, char_to_byte_idx(text, 2));
        assert_eq!(5, char_to_byte_idx(text, 5));
        assert_eq!(6, char_to_byte_idx(text, 6));
        assert_eq!(12, char_to_byte_idx(text, 8));
        assert_eq!(15, char_to_byte_idx(text, 9));
        assert_eq!(16, char_to_byte_idx(text, 10));
    }

    #[test]
    fn char_to_byte_idx_02() {
        let text = "せかい";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(3, char_to_byte_idx(text, 1));
        assert_eq!(6, char_to_byte_idx(text, 2));
        assert_eq!(9, char_to_byte_idx(text, 3));
    }

    #[test]
    fn char_to_byte_idx_03() {
        let text = "Hello world!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(1, char_to_byte_idx(text, 1));
        assert_eq!(8, char_to_byte_idx(text, 8));
        assert_eq!(11, char_to_byte_idx(text, 11));
        assert_eq!(12, char_to_byte_idx(text, 12));
    }

    #[test]
    fn char_to_byte_idx_04() {
        let text = "Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(30, char_to_byte_idx(text, 24));
        assert_eq!(60, char_to_byte_idx(text, 48));
        assert_eq!(90, char_to_byte_idx(text, 72));
        assert_eq!(115, char_to_byte_idx(text, 93));
        assert_eq!(120, char_to_byte_idx(text, 96));
        assert_eq!(150, char_to_byte_idx(text, 120));
        assert_eq!(180, char_to_byte_idx(text, 144));
        assert_eq!(210, char_to_byte_idx(text, 168));
        assert_eq!(239, char_to_byte_idx(text, 191));
    }

    #[test]
    fn char_to_byte_idx_05() {
        // Ascii range
        for i in 0..88 {
            assert_eq!(i, char_to_byte_idx(TEXT_LINES, i));
        }

        // Hiragana characters
        for i in 88..100 {
            assert_eq!(88 + ((i - 88) * 3), char_to_byte_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 100..110 {
            assert_eq!(124, char_to_byte_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn char_to_line_idx_01() {
        let text = "Hello せ\nか\nい!";
        assert_eq!(0, char_to_line_idx(text, 0));
        assert_eq!(0, char_to_line_idx(text, 7));
        assert_eq!(1, char_to_line_idx(text, 8));
        assert_eq!(1, char_to_line_idx(text, 9));
        assert_eq!(2, char_to_line_idx(text, 10));
    }

    #[test]
    fn char_to_line_idx_02() {
        // Line 0
        for i in 0..32 {
            assert_eq!(0, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 1
        for i in 32..59 {
            assert_eq!(1, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 2
        for i in 59..88 {
            assert_eq!(2, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 3
        for i in 88..100 {
            assert_eq!(3, char_to_line_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 100..110 {
            assert_eq!(3, char_to_line_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn line_to_byte_idx_01() {
        let text = "Here\r\nare\r\nsome\r\nwords";
        assert_eq!(0, line_to_byte_idx(text, 0));
        assert_eq!(6, line_to_byte_idx(text, 1));
        assert_eq!(11, line_to_byte_idx(text, 2));
        assert_eq!(17, line_to_byte_idx(text, 3));
    }

    #[test]
    fn line_to_byte_idx_02() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(0, line_to_byte_idx(text, 0));
        assert_eq!(1, line_to_byte_idx(text, 1));
        assert_eq!(6, line_to_byte_idx(text, 2));
        assert_eq!(10, line_to_byte_idx(text, 3));
        assert_eq!(15, line_to_byte_idx(text, 4));
        assert_eq!(21, line_to_byte_idx(text, 5));
    }

    #[test]
    fn line_to_byte_idx_03() {
        assert_eq!(0, line_to_byte_idx(TEXT_LINES, 0));
        assert_eq!(32, line_to_byte_idx(TEXT_LINES, 1));
        assert_eq!(59, line_to_byte_idx(TEXT_LINES, 2));
        assert_eq!(88, line_to_byte_idx(TEXT_LINES, 3));

        // Past end
        assert_eq!(124, line_to_byte_idx(TEXT_LINES, 4));
        assert_eq!(124, line_to_byte_idx(TEXT_LINES, 5));
        assert_eq!(124, line_to_byte_idx(TEXT_LINES, 6));
    }

    #[test]
    fn line_to_char_idx_01() {
        let text = "Hello せ\nか\nい!";
        assert_eq!(0, line_to_char_idx(text, 0));
        assert_eq!(8, line_to_char_idx(text, 1));
        assert_eq!(10, line_to_char_idx(text, 2));
    }

    #[test]
    fn line_to_char_idx_02() {
        assert_eq!(0, line_to_char_idx(TEXT_LINES, 0));
        assert_eq!(32, line_to_char_idx(TEXT_LINES, 1));
        assert_eq!(59, line_to_char_idx(TEXT_LINES, 2));
        assert_eq!(88, line_to_char_idx(TEXT_LINES, 3));

        // Past end
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 4));
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 5));
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 6));
    }

    #[test]
    fn line_byte_round_trip() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(6, line_to_byte_idx(text, byte_to_line_idx(text, 6)));
        assert_eq!(2, byte_to_line_idx(text, line_to_byte_idx(text, 2)));

        assert_eq!(0, line_to_byte_idx(text, byte_to_line_idx(text, 0)));
        assert_eq!(0, byte_to_line_idx(text, line_to_byte_idx(text, 0)));

        assert_eq!(21, line_to_byte_idx(text, byte_to_line_idx(text, 21)));
        assert_eq!(5, byte_to_line_idx(text, line_to_byte_idx(text, 5)));
    }

    #[test]
    fn line_char_round_trip() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(6, line_to_char_idx(text, char_to_line_idx(text, 6)));
        assert_eq!(2, char_to_line_idx(text, line_to_char_idx(text, 2)));

        assert_eq!(0, line_to_char_idx(text, char_to_line_idx(text, 0)));
        assert_eq!(0, char_to_line_idx(text, line_to_char_idx(text, 0)));

        assert_eq!(21, line_to_char_idx(text, char_to_line_idx(text, 21)));
        assert_eq!(5, char_to_line_idx(text, line_to_char_idx(text, 5)));
    }

    #[test]
    fn usize_flag_bytes_01() {
        let v: usize = 0xE2_09_08_A6_E2_A6_E2_09;
        assert_eq!(0x00_00_00_00_00_00_00_00, v.cmp_eq_byte(0x07));
        assert_eq!(0x00_00_01_00_00_00_00_00, v.cmp_eq_byte(0x08));
        assert_eq!(0x00_01_00_00_00_00_00_01, v.cmp_eq_byte(0x09));
        assert_eq!(0x00_00_00_01_00_01_00_00, v.cmp_eq_byte(0xA6));
        assert_eq!(0x01_00_00_00_01_00_01_00, v.cmp_eq_byte(0xE2));
    }

    #[test]
    fn usize_bytes_between_127_01() {
        let v: usize = 0x7E_09_00_A6_FF_7F_08_07;
        assert_eq!(0x01_01_00_00_00_00_01_01, v.bytes_between_127(0x00, 0x7F));
        assert_eq!(0x00_01_00_00_00_00_01_00, v.bytes_between_127(0x07, 0x7E));
        assert_eq!(0x00_01_00_00_00_00_00_00, v.bytes_between_127(0x08, 0x7E));
    }

    #[test]
    fn ends_with_line_break_01() {
        assert_eq!(true, ends_with_line_break("\n"));
        assert_eq!(true, ends_with_line_break("\r"));
        assert_eq!(true, ends_with_line_break("\u{000A}"));
        assert_eq!(true, ends_with_line_break("\u{000B}"));
        assert_eq!(true, ends_with_line_break("\u{000C}"));
        assert_eq!(true, ends_with_line_break("\u{000D}"));
        assert_eq!(true, ends_with_line_break("\u{0085}"));
        assert_eq!(true, ends_with_line_break("\u{2028}"));
        assert_eq!(true, ends_with_line_break("\u{2029}"));
    }

    #[test]
    fn ends_with_line_break_02() {
        assert_eq!(true, ends_with_line_break("Hi there!\n"));
        assert_eq!(true, ends_with_line_break("Hi there!\r"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{000A}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{000B}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{000C}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{000D}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{0085}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{2028}"));
        assert_eq!(true, ends_with_line_break("Hi there!\u{2029}"));
    }

    #[test]
    fn ends_with_line_break_03() {
        assert_eq!(false, ends_with_line_break(""));
        assert_eq!(false, ends_with_line_break("a"));
        assert_eq!(false, ends_with_line_break("Hi there!"));
    }

    #[test]
    fn ends_with_line_break_04() {
        assert_eq!(false, ends_with_line_break("\na"));
        assert_eq!(false, ends_with_line_break("\ra"));
        assert_eq!(false, ends_with_line_break("\u{000A}a"));
        assert_eq!(false, ends_with_line_break("\u{000B}a"));
        assert_eq!(false, ends_with_line_break("\u{000C}a"));
        assert_eq!(false, ends_with_line_break("\u{000D}a"));
        assert_eq!(false, ends_with_line_break("\u{0085}a"));
        assert_eq!(false, ends_with_line_break("\u{2028}a"));
        assert_eq!(false, ends_with_line_break("\u{2029}a"));
    }
}
