# Ropey [![Build Status][trav-ci-img]][trav-ci] [![crates.io badge][crates-io-badge]][crates-io-url]

Ropey is a utf8 text buffer for Rust, implemented as a text rope and designed
for efficient editing and manipulation of large texts.

Note: this repository is currently a WIP for the new implementation of Ropey,
and is not yet published to crates.io.  The currently published versions can be
found at https://github.com/cessen/ropey_old


## Features

### Efficient

Ropey is fast and minimizes memory usage:

- On a recent mobile i7 Intel CPU, Ropey performed over 1 million small
  incoherent insertions per second while building up a text roughly 100 MB
  large.  Coherent insertions (i.e. all near the same place in the text) are
  even faster, doing the same task at over 1.3 million insertions per
  second.
- Freshly loading a file from disk only incurs about 30% memory overhead.  For
  example, a 100 MB text file will occupy about 130 MB of memory when loaded
  by Ropey.
- Cloning ropes is _extremely_ cheap.  Rope clones share data, so an initial
  clone only takes 8 bytes of memory.  After that, memory usage will grow
  incrementally as the clones diverge due to edits.


### Strong Unicode support
Ropey treats Unicode code points (`char`s in Rust) as the atomic unit of text.
Indexing and edits are all done in terms of code points, making the APIs
intuitive and making it impossible to accidentally create invalid utf8 data
through edits.

Ropey also ensures that [grapheme clusters](https://www.unicode.org/reports/tr29/#Grapheme_Cluster_Boundaries)
are never split in its internal representation, and thus can always be accessed
as coherent `&str` slices.  Moreover, Ropey provides APIs for iterating over
graphemes and querying about grapheme boundaries.


### Line-aware

Ropey knows about line breaks, allowing you to index into and iterate over lines
of text.


### Slicing

Ropey has rope slices that allow you to work with just parts of a rope, using
any of the read-only operations of a full rope including iterators and making
sub-slices.


### Streaming loading and saving

Ropey provides APIs for efficiently streaming text data to and from ropes.  This
is primarily intended for efficiently saving and loading text data from disk, but
the APIs are flexible, and can be used for whatever you like.


### Thread safe

Ropey ensures that even though clones share memory, everything is thread-safe.
Clones can be sent to other threads for both reading and writing.


## Things to still investigate

### Memory overhead

Although text loaded from a file has low memory overhead, the same size text
created from scratch by many small inserts has a lot more overhead.  In the
worst cases it can be almost 140%.  This likely isn't a problem for the primary
intended application of Ropey (e.g. as the backing storage for text editors)
since text typed out in a single session is unlikely to be in the megabytes, but
it would be good to address this anyway.

The most likely solution is to be more aggressive about merging leaf nodes when
possible, even during insertion.  The tricky part will be doing that without
harming performance too much.

### Grapheme indexing

The previous version of Ropey had grapheme indexing.  Unfortunately, keeping the
grapheme metadata up-to-date introduced a lot of overhead (5-10x slower).

In practice, it's not clear how useful that feature really is.  The way
graphemes are usually worked with in practice doesn't (I think) need direct
indexing.  Nevertheless, it would be cool to get that feature back.  But not
at such a significant cost to performance.


## License

Ropey is licensed under the MIT license (LICENSE.md or http://opensource.org/licenses/MIT)


## Contributing

Contributions are absolutely welcome!  However, please open an issue or email me
to discuss larger changes, to avoid doing a lot of work that may get rejected.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Ropey by you will be licensed as above, without any additional
terms or conditions.

[crates-io-badge]: https://img.shields.io/crates/v/ropey.svg
[crates-io-url]: https://crates.io/crates/ropey
[trav-ci-img]: https://travis-ci.org/cessen/ropey.svg?branch=master
[trav-ci]: https://travis-ci.org/cessen/ropey