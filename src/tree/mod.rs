mod node;
mod node_children;
mod node_text;
mod text_info;

#[cfg(not(test))]
use std::{mem::size_of, sync::Arc};

pub(crate) use self::node::Node;
pub(crate) use self::node_children::NodeChildren;
pub(crate) use self::node_text::NodeText;
pub(crate) use self::text_info::TextInfo;

#[cfg(not(test))]
const PTR_SIZE: usize = size_of::<&u8>();
#[cfg(not(test))]
const CHILD_INFO_SIZE: usize = size_of::<Arc<Node>>() + size_of::<TextInfo>();

// Aim for nodes to be 768 bytes - Arc counters.  Keeping the nodes
// multiples of large powers of two makes it easier for the memory allocator
// to avoid fragmentation.
#[cfg(not(test))]
const TARGET_NODE_SIZE: usize = 1024 - (PTR_SIZE * 2);

// Node min/max values.
// For testing, they're set small to trigger deeper trees.  For
// non-testing, they're determined by TARGET_NODE_SIZE, above.
#[cfg(test)]
pub(crate) const MAX_CHILDREN: usize = 5;
#[cfg(not(test))]
pub(crate) const MAX_CHILDREN: usize = (TARGET_NODE_SIZE - 1) / CHILD_INFO_SIZE;
pub(crate) const MIN_CHILDREN: usize = MAX_CHILDREN - (MAX_CHILDREN / 2);

#[cfg(test)]
pub(crate) const MAX_BYTES: usize = 9; // Note: can't be 8, because 3-byte characters.
#[cfg(not(test))]
pub(crate) const MAX_BYTES: usize = TARGET_NODE_SIZE - 1 - (PTR_SIZE * 2);
pub(crate) const MIN_BYTES: usize = MAX_BYTES - (MAX_BYTES / 2);

// Type used for storing tree metadata, such as byte and char length.
pub(crate) type Count = u64;
