# Ropey [![Build Status][trav-ci-img]][trav-ci] [![crates.io badge][crates-io-badge]][crates-io-url]

Ropey is a utf8 text buffer for Rust, designed to be the backing text buffer
for applications such as text editors.  Ropey is fast, Unicode-safe, has low
memory overhead, and can handle huge texts and memory-incoherent edits
without trouble.

Internally it's implemented as a b-tree
[rope](https://en.wikipedia.org/wiki/Rope_(data_structure)).

## Features

### Efficient

Ropey is fast and minimizes memory usage:

- On a recent mobile i7 Intel CPU, Ropey performed over 1.1 million small
  incoherent insertions per second while building up a text roughly 100 MB
  large.  Coherent insertions (i.e. all near the same place in the text) are
  even faster, doing the same task at over 1.7 million insertions per
  second.
- Freshly loading a file from disk only incurs about 30% memory overhead.  For
  example, a 100 MB text file will occupy about 130 MB of memory when loaded
  by Ropey.
- Cloning ropes is _extremely_ cheap.  Rope clones share data, so an initial
  clone only takes 8 bytes of memory.  After that, memory usage will grow
  incrementally as the clones diverge due to edits.


### Strong Unicode support
Ropey treats Unicode scalar values (`char`s in Rust) as the atomic unit of
text.  Indexing and edits are all done in terms of Unicode scalar values,
making the APIs intuitive and making it impossible to accidentally create
invalid utf8 data.

Ropey also ensures that [grapheme clusters](https://www.unicode.org/reports/tr29/#Grapheme_Cluster_Boundaries)
are never split in its internal representation, and thus can always be
accessed as `&str` slices.  This is particularly helpful when printing text
to screen because consuming code doesn't have to worry about finding split
graphemes that should be printed as single visual characters. Moreover, Ropey
provides APIs for iterating over graphemes and querying about grapheme
boundaries.


### Line-aware

Ropey knows about line breaks, allowing you to index into and iterate over
lines of text.

Ropey also recognizes all eight Unicode-specified line breaks:
line feed, carriage return, carriage return + line feed, vertical tab,
form feed, next line, line separator, and paragraph separator.


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