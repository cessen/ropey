mod node;
mod node_children;
mod node_text;
mod text_info;

pub(crate) use self::node::Node;
pub(crate) use self::node_children::NodeChildren;
pub(crate) use self::node_text::NodeText;
pub(crate) use self::text_info::TextInfo;

// Internal node min/max values.
pub(crate) const MAX_CHILDREN: usize = 17;
pub(crate) const MIN_CHILDREN: usize = MAX_CHILDREN - (MAX_CHILDREN / 2);

// Leaf node min/max values.
pub(crate) const MAX_BYTES: usize = 335;
pub(crate) const MIN_BYTES: usize = MAX_BYTES - (MAX_BYTES / 2);

// Type used for storing tree metadata, such as byte and char length.
pub(crate) type Count = u32;

/// Largest possible Rope length in bytes.  (Roughly 4 GB.)
pub const MAX_ROPE_LEN: usize = (!(0 as Count) - 2) as usize;

pub fn add_exceeds_max_rope_size(a: Count, b: Count) -> bool {
    const HIGH_BIT: Count = !((!0) >> 1);
    if ((a & HIGH_BIT) | (b & HIGH_BIT)) == 0 {
        // Neither has high bit set
        false
    } else if ((a & HIGH_BIT) ^ (b & HIGH_BIT)) == 0 {
        // Both have high bit set
        true
    } else if ((a & !HIGH_BIT) + (b & !HIGH_BIT)) & HIGH_BIT != 0 {
        // Only one has high bit set, but lower bits overflow
        true
    } else {
        // No overflow, but exceeds max length
        (a + b) > MAX_ROPE_LEN as Count
    }
}
