Ropey is a utf8-text rope library for Rust, designed for efficient editing and
manipulation of large texts.

Note: this repository is currently a WIP for the new implementation of Ropey,
and is not yet published to crates.io.  The currently published versions can be
found at https://github.com/cessen/ropey_old


# Features

## Strong Unicode support
Ropey treats Unicode code points as the base unit of text.  In other words,
you can index into, slice by, and iterate over a rope by `char` index.

Ropey also ensures that [grapheme clusters](https://www.unicode.org/reports/tr29/)
are never split in its internal representation, and thus can always be accessed
as coherent `&str` slices.  Moreover, Ropey provides APIs for iterating over
graphemes and querying about grapheme boundaries.

## Slicing

Ropey has rope slices that allows you to work with just parts of a rope, using
any of the read-only operations of a full rope including iterators and making
sub-slices.


## Line-aware

Ropey knows about line breaks, allowing you to index into and iterate over lines
of text.


## Streaming loading and saving

Ropey provides APIs for efficiently streaming text data to and from ropes.  This
is primarily intended for efficiently saving a rope's text to disk and
efficiently loading text from disk into a new rope.  But the APIs are flexible,
and can be used for whatever you like.


## Efficient

Ropey is fast and minimizes memory usage:

- On a recent mobile i7 Intel CPU, Ropey was able to perform over 700,000 small
  incoherent insertions per second, building up a text roughly 100 MB large.
  Coherent insertions (i.e. all near the same place in the text) are even
  faster, doing the same task at over 1.1 million insertions per second.
- Cloning ropes is _extremely_ cheap.  Rope clones share data, so an initial
  clone only takes 8 bytes of memory.  After that, memory usage will grow
  incrementally as the clones diverge due to edits.
- Freshly loading a file from disk incurs roughly 30% memory overhead.  For
  example, a 100 MB text file will occupy about 130 MB of memory when loaded
  by Ropey.  Memory overhead will increase slowly with edits, but is bounded
  (fragmentation from memory allocators aside).


## Thread safe

Ropey ensures that even though clones share memory, everything is thread-safe.
Clones can be sent to other threads for both reading and writing.


# Things to still investigate

Although text loaded from a file has low memory overhead, the same size text
created from scratch by many small inserts has a lot more overhead.  In the
worst cases it can be almost 140%.  This likely isn't a problem for the primary
intended application of Ropey (e.g. as the backing storage for text editors)
since text typed out in a single session is unlikely to be in the megabytes, but
it would be good to address this anyway.

The most likely solution is to be more aggressive about merging leaf nodes when
possible, even during insertion.  The tricky part will be doing that without
harming performance too much.

# Contributing

Contributions are absolutely welcome!  However, I do have a feeling for how I
want Ropey to be structured and work, so please open an issue or email me to
discuss larger changes, to avoid doing a lot of work for nothing.

By submitting a pull request to this repository, you implicitly license your
code under the same license (MIT, see LICENSE.md) as Ropey.