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
