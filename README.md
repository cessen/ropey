# Ropey

[![CI Build Status][github-ci-img]][github-ci]
[![Latest Release][crates-io-badge]][crates-io-url]
[![Documentation][docs-rs-img]][docs-rs-url]

Ropey is a utf8 text rope for Rust, designed to be the backing text-buffer for
applications such as text editors.  Ropey is fast, robust, and can handle huge
texts and memory-incoherent edits with ease.


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

// Write the file back out to disk.
text.write_to(
    BufWriter::new(File::create("my_great_book.txt")?)
)?;
```

## When Should I Use Ropey?

Ropey is designed and built to be the backing text buffer for applications
such as text editors, and its design trade-offs reflect that.  Ropey is good
at:

- Handling frequent edits to medium-to-large texts.  Even on texts that are
  multiple gigabytes large, edits are measured in single-digit microseconds.
- Handling Unicode correctly.  It is impossible to create invalid utf8 through
  Ropey, and all Unicode line endings are correctly tracked including CRLF.
- Having flat, predictable performance characteristics.  Ropey will never be
  the source of hiccups or stutters in your software.

On the other hand, Ropey is _not_ good at:

- Handling texts smaller than a couple of kilobytes or so.  That is to say,
  Ropey will handle them fine, but Ropey allocates space in kilobyte chunks,
  which introduces unnecessary bloat if your texts are almost always small.
- Handling texts that are larger than available memory.  Ropey is an in-memory
  data structure.
- Getting the best performance for every possible use-case.  Ropey puts work
  into tracking both line endings and unicode scalar values, which is
  performance overhead you may not need depending on your use-case.

Keep this in mind when selecting Ropey for your project.  Ropey is very good
at what it does, but like all software it is designed with certain
applications in mind.


## Features

### Strong Unicode support
Ropey's atomic unit of text is
[Unicode scalar values](https://www.unicode.org/glossary/#unicode_scalar_value)
(or [`char`](https://doc.rust-lang.org/std/primitive.char.html)s in Rust)
encoded as utf8.  All of Ropey's editing and slicing operations are done
in terms of char indices, which prevents accidental creation of invalid
utf8 data.

Ropey also supports converting between scalar value indices and utf16 code unit
indices, for interoperation with external APIs that may still use utf16.

### Line-aware

Ropey knows about line breaks, allowing you to index into and iterate over
lines of text.

The line breaks Ropey recognizes are also configurable at build time via
feature flags.  See Ropey's documentation for details.

### Rope slices

Ropey has rope slices that allow you to work with just parts of a rope, using
all the read-only operations of a full rope including iterators and making
sub-slices.

### Flexible APIs with low-level access

Although Ropey is intentionally limited in scope, it also provides APIs for
efficiently accessing and working with its internal text chunk
representation, allowing additional functionality to be efficiently
implemented by client code with minimal overhead.

### Efficient

Ropey is fast and minimizes memory usage:

- On a recent mobile i7 Intel CPU, Ropey performed over 1.8 million small
  incoherent insertions per second while building up a text roughly 100 MB
  large.  Coherent insertions (i.e. all near the same place in the text) are
  even faster, doing the same task at over 3.3 million insertions per
  second.
- Freshly loading a file from disk only incurs about 10% memory overhead.  For
  example, a 100 MB text file will occupy about 110 MB of memory when loaded
  by Ropey.
- Cloning ropes is _extremely_ cheap.  Rope clones share data, so an initial
  clone only takes 8 bytes of memory.  After that, memory usage will grow
  incrementally as the clones diverge due to edits.

### Thread safe

Ropey ensures that even though clones share memory, everything is thread-safe.
Clones can be sent to other threads for both reading and writing.


## Unsafe code

Ropey uses unsafe code to help achieve some of its space and performance
characteristics.  Although effort has been put into keeping the unsafe code
compartmentalized and making it correct, please be cautious about using Ropey
in software that may face adversarial conditions.

Auditing, fuzzing, etc. of the unsafe code in Ropey is extremely welcome.
If you find any unsoundness, _please_ file an issue!  Also welcome are
recommendations for how to remove any of the unsafe code without introducing
significant space or performance regressions, or how to compartmentalize the
unsafe code even better.


## License

Ropey is licensed under the MIT license (LICENSE.md or http://opensource.org/licenses/MIT)


## Contributing

Contributions are absolutely welcome!  However, please open an issue or email
me to discuss larger changes, to avoid doing a lot of work that may get
rejected.

An overview of Ropey's design can be found [here](https://github.com/cessen/ropey/blob/master/design/design.md).

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in Ropey by you will be licensed as above, without any additional terms or conditions.

[crates-io-badge]: https://img.shields.io/crates/v/ropey.svg
[crates-io-url]: https://crates.io/crates/ropey
[github-ci-img]: https://github.com/cessen/ropey/workflows/ci/badge.svg
[github-ci]: https://github.com/cessen/ropey/actions?query=workflow%3Aci
[docs-rs-img]: https://docs.rs/ropey/badge.svg
[docs-rs-url]: https://docs.rs/ropey
