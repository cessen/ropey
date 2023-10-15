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
#[cfg(not(any(test, feature = "small_chunks")))]
mod constants {
    use super::{Node, TextInfo};
    use smallvec::SmallVec;
    use std::{
        mem::{align_of, size_of},
        sync::Arc,
    };

    // Because stdlib's max is not const for some reason.
    // TODO: replace with stdlib max once it's const.
    const fn cmax(a: usize, b: usize) -> usize {
        if a > b {
            a
        } else {
            b
        }
    }

    // Aim for Node + Arc counters to be 1024 bytes.  Keeping the nodes
    // multiples of large powers of two makes it easier for the memory
    // allocator to avoid fragmentation.
    const TARGET_TOTAL_SIZE: usize = 1024;

    // Space that the strong and weak Arc counters take up in `ArcInner`.
    const ARC_COUNTERS_SIZE: usize = size_of::<std::sync::atomic::AtomicUsize>() * 2;

    // Misc useful info that we need below.
    const NODE_CHILDREN_ALIGN: usize = cmax(align_of::<Arc<u8>>(), align_of::<TextInfo>());
    const NODE_TEXT_ALIGN: usize = align_of::<SmallVec<[u8; 16]>>();
    const START_OFFSET: usize = {
        const NODE_INNER_ALIGN: usize = cmax(NODE_CHILDREN_ALIGN, NODE_TEXT_ALIGN);
        // The +NODE_INNER_ALIGN is because of Node's enum discriminant.
        ARC_COUNTERS_SIZE + NODE_INNER_ALIGN
    };

    // Node maximums.
    #[doc(hidden)] // NOT PART OF THE PUBLIC API!
    pub const MAX_CHILDREN: usize = {
        let node_list_align = align_of::<Arc<u8>>();
        let info_list_align = align_of::<TextInfo>();
        let field_gap = if node_list_align >= info_list_align {
            0
        } else {
            // This is over-conservative, because in reality it depends
            // on the number of elements.  But handling that is probably
            // more complexity than it's worth.
            info_list_align - node_list_align
        };

        // The -NODE_CHILDREN_ALIGN is for the `len` field in `NodeChildrenInternal`.
        let target_size = TARGET_TOTAL_SIZE - START_OFFSET - NODE_CHILDREN_ALIGN - field_gap;

        target_size / (size_of::<Arc<u8>>() + size_of::<TextInfo>())
    };
    #[doc(hidden)] // NOT PART OF THE PUBLIC API!
    pub const MAX_BYTES: usize = {
        let smallvec_overhead = size_of::<SmallVec<[u8; 16]>>() - 16;
        TARGET_TOTAL_SIZE - START_OFFSET - smallvec_overhead
    };

    // Node minimums.
    // Note: MIN_BYTES is intentionally a little smaller than half
    // MAX_BYTES, to give a little wiggle room when on the edge of
    // merging/splitting.
    #[doc(hidden)] // NOT PART OF THE PUBLIC API!
    pub const MIN_CHILDREN: usize = MAX_CHILDREN / 2;
    #[doc(hidden)] // NOT PART OF THE PUBLIC API!
    pub const MIN_BYTES: usize = (MAX_BYTES / 2) - (MAX_BYTES / 32);

    // Compile-time assertion.
    const _: () = {
        assert!(
            (ARC_COUNTERS_SIZE + size_of::<Node>()) == TARGET_TOTAL_SIZE,
            "`Node` is not the target size in memory.",
        );
    };
}

// Smaller constants used in debug builds.  These are different from release
// in order to trigger deeper trees without having to use huge text data in
// the tests.
#[cfg(any(test, feature = "small_chunks"))]
mod test_constants {
    #[doc(hidden)] // NOT PART OF THE PUBLIC API!
    pub const MAX_CHILDREN: usize = 5;
    #[doc(hidden)] // NOT PART OF THE PUBLIC API!
    pub const MIN_CHILDREN: usize = MAX_CHILDREN / 2;

    // MAX_BYTES must be >= 4 to allow for 4-byte utf8 characters.
    #[doc(hidden)] // NOT PART OF THE PUBLIC API!
    pub const MAX_BYTES: usize = 9; // Note: can't be 8, because 3-byte characters.
    #[doc(hidden)] // NOT PART OF THE PUBLIC API!
    pub const MIN_BYTES: usize = (MAX_BYTES / 2) - (MAX_BYTES / 32);
}

#[cfg(not(any(test, feature = "small_chunks")))]
pub use self::constants::{MAX_BYTES, MAX_CHILDREN, MIN_BYTES, MIN_CHILDREN};

#[cfg(any(test, feature = "small_chunks"))]
pub use self::test_constants::{MAX_BYTES, MAX_CHILDREN, MIN_BYTES, MIN_CHILDREN};
