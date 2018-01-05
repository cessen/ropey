# Ropey's Design

This document explains Ropey's technical design.  It is primarily targeted at potential contributors, to help them understand the codebase.  But it may also be of interest to consumers of the library and the generally-curious.


## Directory Structure

Public-facing:

- `src/rope.rs`: the high-level implementation of `Rope`.
- `src/slice.rs`: implementation of `RopeSlice`.
- `src/iter.rs`: implementations of all the iterators.
- `src/rope_builder.rs`: implementation of `RopeBuilder`.

Internal-only:
- `src/tree/`: the low-level implementation of `Rope`'s internals, where most of the meat of the b-tree rope is.
- `src/str_utils.rs`: utility functions that operate on `&str` slices.  For example, functions to count chars and line endings.


## B-tree Rope

The core data structure in Ropey is a [b-tree](https://en.wikipedia.org/wiki/B-tree) [Rope](https://en.wikipedia.org/wiki/Rope_(data_structure)).  This data-structure was chosen for several reasons:

- It has good random-access editing performance.  [Gap buffers](https://en.wikipedia.org/wiki/Gap_buffer) are another popular choice of data structure for text editing, but they perform poorly on random-access edits.  Multiple-cursor support is common in code editors at this point, and being able to efficiently edit at many locations simultaneously is therefore important.
- It can naturally track `char` indices and line endings.  Most other data structures require additional external data structures to be maintained to track such information.
- B-trees minimize pointer indirection and make traversal patterns more coherent when properly implemented, which is important for performing well with modern memory architectures and large data sets.

The nodes of Ropey's b-tree are primarily implemented in three files:

- `src/tree/node_text.rs`: a tailor-made small string implementation that stores text at the leaf nodes of the tree.
- `src/tree/node_children.rs`: a tailor-made fixed-capacity vec implementation that stores the child meta-data and child pointers at the internal nodes of the tree.
- `src/tree/node.rs`: the main `Node` implementation, which uses the types defined in the above two files as the leaf and internal variants in an `enum`.

Most of the logic for traversing and modifying the tree is implemented in `node.rs`.  I've tried to limit the code in `node_text.rs` and `node_children.rs` to things that only involve the immediate node, and not any kind of tree traversal.

The four main methods in `node.rs` are:

- `Node::edit_char_range()`
- `Node::split()`
- `Node::prepend_at_depth()`
- `Node::append_at_depth()`.

These are by far the most complex code in Ropey, and are the core editing operations which the `Rope` type uses to implement its own editing operations.  Be very careful when modifying them and their helper methods, as there are many invariants that must be held for everything to work properly.  Ropey has a lot of unit tests, and running `cargo test` is a useful way to help minimize the chances that you break something, but don't depend on that entirely.

Aside from a handful of additional helper-methods for the above editing methods, the rest of the methods are based on pretty straight-forward tree traversals.


## Tree Invariants

The invariants of the tree that must hold true for the tree to operate correctly are:

- The standard b-tree invariants:
    - All leaf nodes must be at the same depth in the tree.
    - Internal nodes must have no more than `MAX_CHILDREN` children and no fewer than `MIN_CHILDREN` children (except the root node&mdash;see next point).  These constants are defined in `src/tree/mod.rs`.
    - When the root node is an internal node it may have fewer than `MIN_CHILDREN`, but it still must have at least two children.
- All child meta-data at the internal nodes must be accurate.  (The meta-data is stored in an array of `TextInfo` structs in the `NodeChildren` type in `src/tree/node_children.rs`).
- Leaf nodes must never be empty, except for the root node when it is a leaf.
- The constituent code points in a grapheme cluster must never be separated by a leaf node boundary.  For example, if '\r' and '\n' are next to each other but split by a leaf boundary, the code for counting line endings won't work properly.  And more generally the methods for iterating over graphemes won't work correctly if any graphemes are split.

There are some hidden-from-documentation methods on `Rope` that check for and assert these invariants:

- `Rope:assert_integrity()`: checks for basic child meta-data integrity.  This is _the most important_ check, as things will break in crazy ways if this isn't true.  If you get really strange behavior from the tree, this is the first thing to check.
- `Rope::assert_invariants()`: checks that the rest of the invariants listed above hold true.  If you get panics or weird performance degradation, this is the second thing to check.

There is one final "invariant" that should _generally_ hold true, but doesn't strictly need to for correct operation and _may_ be violated under some circumstances:

- Leaf nodes should _generally_ not contain less than `MIN_BYTES` of text or more than `MAX_BYTES` of text.

For any text that's not really crazy, this invariant should hold true.  But theoretically, for example, there may be a single grapheme that is larger than `MAX_BYTES`.  In such a case the leaf node must exceed `MAX_BYTES` to ensure that the don't-split-graphemes invariant holds.  Similarly, there may be two such huge graphemes with a very small amount of text sandwiched between them, in which case that small text may be put in its own node, violating `MIN_BYTES`.

In practice, these cases are vanishingly unlikely to ever happen in real (and non-broken) text, but they nevertheless need to be handled correctly by all code.


## Memory Layout

The structures in Ropey's rope implementation have been carefully designed to:

1. Minimize pointer chasing.
2. Make it easy for memory allocators to compactly store nodes.
3. Make it easy for memory allocators to compactly _re-use_ space from previously freed nodes for new nodes.

These goals are the reason for the seemingly strange designs of `NodeChildren` and `NodeText`, as well as the strange way that the `MIN_*`/`MAX_*` constants are calculated in `src/tree/mod.rs`.

To motivate the design a bit, let's explore what a naive implementation of a b-tree rope might look like.  And for simplicity, let's only track bytes in the rope, not chars or line endings.  It could be implemented as a single enum, like this:

``` Rust
enum Node {
    Internal {
        byte_count: usize,
        Vec<Node>,
    },
    Leaf(String),
}
```

This is actually pretty good!  And in memory, it looks something like this:

![Diagram of naive rope](images/naive.png)

There are a couple of things about this that aren't great:

- Although the children of a node are all stored next to each other in memory (good for memory locality), their number varies.  Moreover, the memory allocated for leaf text also varies in size.  This can lead to memory fragmentation because the space allocated for children or leaf text can't necessarily be reused for other children or leaf text later on if their size is larger.
- The leaf nodes all have an extra level of indirection to get to their actual data.

Having said that, this is actually a pretty decent design!  The fragmentation is (fingers crossed) unlikely to be a major issue with a decent allocator.  And it's only a single additional level of indirection that only happens at the bottom of the tree, which is likely to cause only minor performance degradation.

But one of Ropey's design goals is also to share data between Rope clones, to make cloning cheap.  This means that every child needs to be kept under something like a reference-counted smart pointer, so it can be shared between trees.  So what happens if we do that?

``` Rust
enum Node {
    Internal {
        byte_count: usize,
        Vec<Rc<Node>>,
    },
    Leaf(String),
}
```

![Diagram of bad rope](images/bad.png)

Suddenly things are a lot worse off:

- We no longer have good memory locality: children and their `byte_count` metadata are all over the place.
- We have an extra level of memory indirection _at every level of the tree_, and _two_ levels of indirection at the leaf nodes.

For performance this is pretty bad.  For example, even just choosing which child to traverse into requires checking the children's metadata, which means jumping all over the place in memory:

![Diagram of bad jumping](images/bad_jumping.png)

Ropey addresses this in two ways.  First, it stores child meta-data in the _parent_ node, in a coherent array.  This allows fast scanning to decide which child to traverse into.  And second, it in-lines the leaf's string data into the enum.  A simplified (and not-quite-actually-workable) version of that approach might look like this:

``` Rust
enum Node {
    Internal {
        child_count: u8,
        child_byte_counts: [usize; 4],
        children: [Rc<Node>; 4],
    },
    Leaf {
        len: usize,
        [u8; 100],
    },
}
```

![Diagram of good rope](images/good.png)

Now we've eliminated all unnecessary indirection and we've kept all metadata coherent in memory for fast scanning.  And (bonus!) all nodes are exactly the same size as each other, including leaf text data, which makes things really easy for the memory allocator.

This is essentially Ropey's design, but implemented with a bit more sophistication.  The main differences are:

- Ropey tracks char count and line-ending count in addition to byte count.
- Instead of a plain byte array for text storage, Ropey uses a "small string" implementation (`NodeText`).  This is both for API convenience (all the rest of Ropey's code can interact with leaf text as normal strings), and also because if a leaf happens to exceed the inlined byte count, it can spill out into a heap-allocated string (this should be extremely, vanishingly rare--it's really just a precaution against weird pathological cases).
- Instead of directly using raw arrays for storing child pointers and metadata, Ropey wraps all of that in a separate type (`NodeChildren`) that does all of the fiddly unsafe book-keeping and exposes a safe API.

Other than that, Ropey's memory layout is pretty much designed exactly like the code snippet above.

One final piece of the puzzle: since the inlined leaf text and the child pointers/metadata are both crammed into the same enum, they should both be sized to take up roughly the same amount of space, to minimize unused bytes.  Moreover, allocators work best with sizes in the multiples of large-ish powers of two.  That is what all the weird calculations for the `MAX_*` constants are doing.  Note also that with `Rc` and `Arc`, they take up two words of space for the strong and weak reference counts, so that is also accounted for, so that the actual allocated chunk of memory ends up being a multiple of 256 bytes.

Phew!  Hopefully that all made sense.

[TODO: the rest of this section.]


## Rope Clones and Thread Safety

[TODO]


## Unsafe Code

[TODO]
