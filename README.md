# Ropey 2 (beta)

[![CI Build Status][github-ci-img]][github-ci]
[![Latest Release][crates-io-badge]][crates-io-url]
[![Documentation][docs-rs-img]][docs-rs-url]

Ropey is a utf8 text rope for Rust, designed to be the backing text-buffer for applications such as text editors.  Ropey is fast, robust, and can handle huge texts and memory-incoherent edits with ease.

**Note:** this is the 2.0 version of Ropey, which is currently in beta.  It is not battle-tested like Ropey 1.x, and there may still be some minor breaking API changes before final release, but generally things should be pretty stable at this point.  We encourage you to use Ropey 2 Beta in non-critical projects and provide feedback, report bugs, etc.

For a summary of what's different between Ropey 2.x and Ropey 1.x, please see the [changelog](CHANGELOG.md#200-alpha-1---2024-10-20).


## Example Usage

```rust
use ropey::{Rope, LineType::LF_CR};

// Load a text file.
let mut text = Rope::from_reader(
    BufReader::new(File::open("my_great_book.txt")?)
)?;

// Print the 516th line (zero-indexed) to see the terrible
// writing.
println!("{}", text.line(515, LF_CR));

// Get the start/end byte indices of the line.
let start_idx = text.line_to_byte_idx(515, LF_CR);
let end_idx = text.line_to_byte_idx(516, LF_CR);

// Remove the line...
text.remove(start_idx..end_idx);

// ...and replace it with something better.
text.insert(start_idx, "The flowers are... so... dunno.\n");

// Print the changes, along with the previous few lines for context.
let start_idx = text.line_to_byte_idx(511, LF_CR);
let end_idx = text.line_to_byte_idx(516, LF_CR);
println!("{}", text.slice(start_idx..end_idx));

// Write the file back out to disk.
text.write_to(
    BufWriter::new(File::create("my_great_book.txt")?)
)?;
```


## When Should I Use Ropey?

Ropey is designed and built to be the backing text buffer for applications such as text editors, and its design trade-offs reflect that.  Ropey is good at:

- Handling frequent edits to medium-to-large texts.  Even on texts that are multiple gigabytes large, edits are measured in single-digit microseconds.
- Handling Unicode correctly.  It is impossible to create invalid utf8 through Ropey, and all Unicode line breaks can be correctly tracked including CRLF.
- Having flat, predictable performance characteristics.  Ropey will never be the source of hiccups or stutters in your software.

On the other hand, Ropey is _not_ good at:

- Handling texts smaller than a couple of kilobytes or so.  That is to say, Ropey will handle them fine, but Ropey allocates space in kilobyte chunks, which introduces unnecessary bloat if your texts are almost always small.
- Handling texts that are larger than available memory.  Ropey is an in-memory data structure.
- Directly handling text that is non-unicode, corrupted, or includes chunks of binary data. Ropey only handles utf8 text, so non-utf8 data needs to be converted/sanitized before being passed to Ropey.
- Getting the best performance/memory characteristics for every possible use case.  For example, Ropey puts work into tracking line breaks and other secondary metrics, which introduces overhead you may not need depending on your use case.

Keep this in mind when selecting Ropey for your project.  Ropey is very good at what it does, but like all software it is designed with certain applications in mind.


## With Ropey 2.x soon to be released, what will happen to development of Ropey 1.x?

Ropey 1.x will continue to be maintained for the foreseeable future, but will no longer receive new features.  Ropey 1.x is still a good, high-quality rope library that can be depended on, and you don't need to move to Ropey 2.x if 1.x serves your needs.


## Unsafe code

Ropey uses unsafe code to help achieve some of its space and performance characteristics.  Although effort has been put into keeping the unsafe code minimal, compartmentalized, and correct, please be cautious about using Ropey in software that may face adversarial conditions.


## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.


## Contributing

Contributions are absolutely welcome!  However, please open an issue to discuss larger changes, to avoid doing a lot of work that may get rejected.  Also note that PRs that add dependencies are very likely to be rejected (Ropey aims to have minimal dependencies).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Ropey by you will be licensed as above, without any additional terms or conditions.

[crates-io-badge]: https://img.shields.io/crates/v/ropey.svg
[crates-io-url]: https://crates.io/crates/ropey
[github-ci-img]: https://github.com/cessen/ropey/workflows/ci/badge.svg
[github-ci]: https://github.com/cessen/ropey/actions?query=workflow%3Aci
[docs-rs-img]: https://docs.rs/ropey/badge.svg
[docs-rs-url]: https://docs.rs/ropey