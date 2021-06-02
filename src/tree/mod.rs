mod node;
mod node_children;
mod node_text;
mod text_info;

pub(crate) use self::node::Node;
pub(crate) use self::node_children::NodeChildren;
pub(crate) use self::node_text::NodeText;
pub(crate) use self::text_info::TextInfo;

// Type used for storing tree metadata, such as byte and char length.
pub(crate) type Count = u64;

// Real constants used in release builds.
#[cfg(not(test))]
mod constants {
    use super::{Node, TextInfo};
    use smallvec::SmallVec;
    use std::{
        mem::{align_of, size_of},
        sync::Arc,
    };

    const PTR_SIZE: usize = size_of::<&u8>();
    const CHILD_INFO_SIZE: usize = size_of::<Arc<Node>>() + size_of::<TextInfo>();
    const CHILD_INFO_MAX_ALIGN: usize = if align_of::<Arc<Node>>() > align_of::<Arc<TextInfo>>() {
        align_of::<Arc<Node>>()
    } else {
        align_of::<Arc<TextInfo>>()
    };

    // Aim for nodes to be 1024 bytes minus Arc counters.  Keeping the nodes
    // multiples of large powers of two makes it easier for the memory allocator
    // to avoid fragmentation.
    const TARGET_NODE_SIZE: usize = 1024 - (PTR_SIZE * 2);

    // Min/max number of children of an internal node.
    pub(crate) const MAX_CHILDREN: usize = (TARGET_NODE_SIZE
        // In principle we want to subtract NodeChildren's length counter
        // here, which would be one byte.  However, due to that extra byte
        // NodeChildren actually gets padded out to the alignment of
        // Arc/TextInfo.  So we need to subtract that alignment padding
        // instead.
        - CHILD_INFO_MAX_ALIGN
        // Minus Node's enum discriminant.
        - 1)
        / CHILD_INFO_SIZE;
    pub(crate) const MIN_CHILDREN: usize = MAX_CHILDREN / 2;

    // Soft min/max number of bytes of text in a leaf node.
    // Note: MIN_BYTES is little smaller than half MAX_BYTES so that repeated
    // splitting/merging doesn't happen on alternating small insertions and
    // removals.
    pub(crate) const MAX_BYTES: usize = (TARGET_NODE_SIZE
            // Minus NodeText's SmallVec overhead:
            - (size_of::<SmallVec<[u8; 32]>>() - 32)
            // Minus Node's enum discriminant:
            - 1)
        // Round down to NodeText's SmallVec alignment, since NodeText will get
        // padded out otherwise, pushing it over the target node size.
        & !(align_of::<SmallVec<[u8; 32]>>() - 1);
    pub(crate) const MIN_BYTES: usize = (MAX_BYTES / 2) - (MAX_BYTES / 32);

    // These weird expressions are essentially poor-man's compile-time
    // assertions.  They result in a compile-time error if everything
    // isn't the way it needs to be for the combined "Arc counters + Node"
    // size we want.
    // The resulting error messages are cryptic, but they're distinct enough
    // that if people run into them and file an issue it should be obvious what
    // they are.
    const _: [(); 0 - !{
        // Assert alignment.
        const ASSERT: bool = align_of::<Node>() <= align_of::<&u8>();
        ASSERT
    } as usize] = [];
    const _: [(); 0 - !{
        // Assert size.
        const ASSERT: bool = size_of::<Node>() == TARGET_NODE_SIZE;
        ASSERT
    } as usize] = [];
}

// Smaller constants used in debug builds.  These are different from release
// in order to trigger deeper trees without having to use huge text data in
// the tests.
#[cfg(test)]
mod test_constants {
    pub(crate) const MAX_CHILDREN: usize = 5;
    pub(crate) const MIN_CHILDREN: usize = MAX_CHILDREN / 2;

    // MAX_BYTES must be >= 4 to allow for 4-byte utf8 characters.
    pub(crate) const MAX_BYTES: usize = 9; // Note: can't be 8, because 3-byte characters.
    pub(crate) const MIN_BYTES: usize = (MAX_BYTES / 2) - (MAX_BYTES / 32);
}

#[cfg(not(test))]
pub(crate) use self::constants::{MAX_BYTES, MAX_CHILDREN, MIN_BYTES, MIN_CHILDREN};

#[cfg(test)]
pub(crate) use self::test_constants::{MAX_BYTES, MAX_CHILDREN, MIN_BYTES, MIN_CHILDREN};
