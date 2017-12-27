mod node;
mod node_children;
mod node_text;
mod text_info;

pub(crate) use self::node::Node;
pub(crate) use self::node_children::NodeChildren;
pub(crate) use self::node_text::NodeText;
pub(crate) use self::text_info::TextInfo;

// Once size_of() is a const fn, remove this and use size_of() instead.
#[cfg(all(not(test), target_pointer_width = "64"))]
const PTR_SIZE: usize = 8;
#[cfg(all(not(test), target_pointer_width = "32"))]
const PTR_SIZE: usize = 4;
#[cfg(all(not(test), target_pointer_width = "16"))]
const PTR_SIZE: usize = 2;

// Aim for nodes to be 512 - Arc counters.  This makes it easier for the
// memory allocator to avoid fragmentation.
#[cfg(not(test))]
const TARGET_NODE_SIZE: usize = 512 - (PTR_SIZE * 2);

// Node min/max values.
// For testing, they're set small to trigger deeper trees.  For
// non-testing, they're determined by TARGET_NODE_SIZE, above.
#[cfg(test)]
pub(crate) const MAX_CHILDREN: usize = 5;
#[cfg(not(test))]
pub(crate) const MAX_CHILDREN: usize = (TARGET_NODE_SIZE - 1) / 32;
pub(crate) const MIN_CHILDREN: usize = MAX_CHILDREN - (MAX_CHILDREN / 2);

#[cfg(test)]
pub(crate) const MAX_BYTES: usize = 2;
#[cfg(not(test))]
pub(crate) const MAX_BYTES: usize = TARGET_NODE_SIZE - 1 - (PTR_SIZE * 2);
pub(crate) const MIN_BYTES: usize = MAX_BYTES - (MAX_BYTES / 2);

// Type used for storing tree metadata, such as byte and char length.
pub(crate) type Count = u64;
