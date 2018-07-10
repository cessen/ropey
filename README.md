# Ropey

[![Travis CI Build Status][trav-ci-img]][trav-ci] [![][crates-io-badge]][crates-io-url]

Ropey is a utf8 text buffer for Rust, designed to be the backing text buffer
for applications such as text editors.  Ropey is fast, Unicode-safe, has low
memory overhead, and can handle huge texts and memory-incoherent edits
without trouble.

Internally it's implemented as a b-tree
[rope](https://en.wikipedia.org/wiki/Rope_(data_structure)).

## Example Usage

```rust
// Load a text file.
let mut text = ropey::Rope::from_reader(
    File::open("my_great_book.txt")?
)?;

// Print the 516th line (zero-indexed).
println!("{}", text.line(515));

// Get the start/end char indices of the line.
let start_idx = text.line_to_char(515);
let end_idx = text.line_to_char(516);

// Remove the line...
text.remove(start_idx..end_idx);

// ...and replace it with something better.
text.insert(start_idx, "The flowers are... so... dunno.\n");

// Print the changes, along with the previous few lines for context.
let start_idx = text.line_to_char(511);
let end_idx = text.line_to_char(516);
println!("{}", text.slice(start_idx..end_idx));

// Write the file back out to disk using the `Chunks` iterator
// for efficiency.
let mut file = File::create("my_great_book.txt")?;
for chunk in text.chunks() {
    file.write(chunk.as_bytes())?;
}
```

## Features

### Efficient

Ropey is fast and minimizes memory usage:

- On a recent mobile i7 Intel CPU, Ropey performed over 1.5 million small
  incoherent insertions per second while building up a text roughly 100 MB
  large.  Coherent insertions (i.e. all near the same place in the text) are
  even faster, doing the same task at over 2.5 million insertions per
  second.
- Freshly loading a file from disk only incurs about 17% memory overhead.  For
  example, a 100 MB text file will occupy about 117 MB of memory when loaded
  by Ropey.
- Cloning ropes is _extremely_ cheap.  Rope clones share data, so an initial
  clone only takes 8 bytes of memory.  After that, memory usage will grow
  incrementally as the clones diverge due to edits.


### Strong Unicode support
Ropey treats [Unicode scalar values](https://www.unicode.org/glossary/#unicode_scalar_value)
([`char`](https://doc.rust-lang.org/std/primitive.char.html)s in Rust)
encoded in utf8 as the atomic unit of text.  Indexing and edits are all done
in terms of Unicode scalar values, making the APIs intuitive and making it
impossible to accidentally create invalid utf8 data.


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


### Flexible APIs With Low Level Access

Although Ropey is intentionally limited in scope, it also provides APIs for
efficiently accessing and working with its internal text chunk
representation, allowing additional functionality to be efficiently
implemented by client code with minimal overhead.


### Thread safe

Ropey ensures that even though clones share memory, everything is thread-safe.
Clones can be sent to other threads for both reading and writing.


## Note about unsafe code:

Ropey does use unsafe code to help acheive some of its space and performance
characteristics.  Although a lot of effort has been put into keeping unsafe
code compartmentalized and making it correct, it is nevertheless _not_
recommended to use Ropey in software that may face adversarial conditions.

Auditing, fuzzing, etc. of the unsafe code in Ropey is extremely welcome.
If any unsoundness is found, _please_ file an issue!  Also welcome are
recommendations for how to remove any of the unsafe code from Ropey without
introducing significant space or performance regressions, or how to
compartmentalize the unsafe code even better.


## License

Ropey is licensed under the MIT license (LICENSE.md or http://opensource.org/licenses/MIT)


## Contributing

Contributions are absolutely welcome!  However, please open an issue or email me
to discuss larger changes, to avoid doing a lot of work that may get rejected.

An overview of Ropey's design can be found [here](https://github.com/cessen/ropey/blob/master/design/design.md).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Ropey by you will be licensed as above, without any additional
terms or conditions.

[crates-io-badge]: https://img.shields.io/crates/v/ropey.svg
[crates-io-url]: https://crates.io/crates/ropey
[trav-ci-img]: https://travis-ci.org/cessen/ropey.svg?branch=master
[trav-ci]: https://travis-ci.org/cessen/ropey