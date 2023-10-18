# Changelog


## [Unreleased]


## [1.6.1] - 2023-10-18

- Fixed test code that was incorrect on some platforms / with some configurations.
- Minor documentation improvements.


## [1.6.0] - 2023-02-01

### New features
- Added `is_instance()` method, which checks if two ropes are same-memory instances of each other.

### Bug fixes
- Ropey would panic when trying to create a `Lines` iterator for an empty rope or rope slice.


## [1.5.1] - 2023-01-01

### Performance
- A much faster `Lines` iterator, thanks to @pascalkuthe (PR #70).

### Bug fixes
- Ropey's `Hash` impl was incorrect, due to making incorrect assumptions about the guaranteed behavior of `Hasher`s.  This didn't cause any problems with Rust's default hasher, but was incorrect in the general case.
- Comparing ropes for equality would panic when the two ropes had chunk boundaries that weren't mutually aligned at char boundaries.
- `len_lines()` could give incorrect counts on `RopeSlice`s that split CRLF pairs.
- Ropey's internal B-Tree representation could (rarely) end up in a state that violated some invariants.  This didn't affect anything in practice, because no code currently depends on the violated invariant.  But future code might.


## 1.5.1-alpha - 2022-11-27

- Special early release, mainly to accomodate the Helix project.  It is not recommended to use this release outside of Helix.


## [1.5.0] - 2022-05-29

### New features
- Added a `reversed()` method for Ropey's iterators.  This is the same as `reverse()` except instead of mutating in-place, it consumes the iterator and returns it reversed.  This is more convenient when chaining iterator method calls.
- Added a `simd` cargo feature flag.  It's enabled by default, but can be disabled to use only scalar code (no simd intrinsics).

### Bug fixes
- Fix a theoretical memory safety issue found via running Ropey's tests through miri.  Thanks to @Nilstrieb!
- Fix (unintentionally) depending on Rust memory layout to achieve precise node sizes in memory.  We now use `repr(C)`.


## [1.4.1] - 2022-03-16

### Bug fixes
- Fix a stupid copy/paste typo in the previous line break feature flag implementation that caused the wrong line break code to be used.


## [1.4.0] - 2022-03-16

### New features
- Added `byte_slice()` and `get_byte_slice()` methods to `Rope` and `RopeSlice` to slice by byte index instead of char index.  This can allow optimizations in client code in some cases.
- Added `cr_lines` and `unicode_lines` feature flags to the crate, to manage what line endings are recognized and tracked.  This allows, for example, building Ropey to only recognize line feed as a line break.  `unicode_lines` is on by default, and corresponds to the original behavior.
- Implemented `std::hash::Hash` for `Rope` and `RopeSlice`.

### Misc
- Split `str_utils` module out into a separate crate, `str_indices`.  The `str_utils` module still exists, but is now mostly just a re-export of the new crate.


## [1.3.2] - 2021-12-30

### Bug fixes
- Relax the lifetime requirements of various `RopeSlice` methods.  They were unintentionally strict.


## [1.3.1] - 2021-06-22

### Bug fixes
- Fix unnecessary rope fragmentation when using `Rope::append()` to append many small ropes together.
- Fix contiguous `RopeSlices` occasionally failing to convert to a `&str` with `RopeSlice::as_str()`.


## [1.3.0] - 2021-06-16

### New features
- Added non-panicking versions of all methods on `Rope` and `RopeSlice`.
- All iterators can now be reversed, swapping the beheavior of `prev()` and `next()`.

### Bug fixes
- The in-memory node size wasn't being computed properly, potentially resulting in unecessary memory fragmentation.


## [1.2.0] - 2020-06-14

### New features
- `Rope` and `RopeSlice` can now convert between char indices and utf16 code unit indices.  This useful when interacting with external APIs that use utf16 code units as their text indexing scheme.

### Dependencies
- Updated smallvec to minimum version 1.0.


## [1.1.0] - 2019-09-01

### New features
- Iterators can now be created directly to start at any position in the `Rope` or `RopeSlice`.
- All iterators can now iterate backwards via a new `prev()` method.
- All iterators now implement `Clone` and `Debug` traits.
- `Bytes`, `Chars`, and `Lines` iterators now implement `ExactSizeIterator`.

### Changes
- The `Chunks` iterator no longer yields empty chunks, for example if the `Rope` or `RopeSlice` it was created from is also empty.


## [1.0.1] - 2019-05-01

### Other
- Converted a lot of unsafe code to safe code, with minimal performance impact.


## [1.0.0] - 2019-01-03

### New features
- Implemented `Eq`, `Ord`, and `PartialOrd` traits for `Rope` and `RopeSlice`.


## [0.9.2] - 2018-10-04

### Bug fixes
- Turns out the previous Line iterator bug fix introduced a different bug.  Fixed!


## [0.9.1] - 2018-10-03

### Bug fixes
- The Lines iterator would sometimes emit an extra blank line when created from a small rope slice.
- The `write_to()` convenience method could potentially write only part of the rope, without any error indication.


## [0.9.0] - 2018-09-04

### Performance improvements
- Minor performance improvements to a few methods on `Rope` and `RopeSlice`.

### New features
- Added `Rope::byte()` for fetching individual bytes by index.
- Added more conversion functions for `Rope` and `RopeSlice`, in the form of `From` impls.

### Breaking changes
- Removed `Rope::to_string()`, `RopeSlice::from_str()`, `RopeSlice::to_string()`, and `RopeSlice::to_rope()` in favor of `From` impls that do the same thing.


## [0.8.4] - 2018-07-28

### Performance improvements
- Minor across-the-board speedups by using SIMD better.
- Significant speedups for Rope::insert()/remove() by being more clever about node info updates.
- Further significant speedup to Rope::remove() due to a (performance-only) bug fix.

### Bug fixes
- Ropey wouldn't compile on non-x86/64 platforms after the introduction of SSE2 optimizations in v0.8.3.  They are now wrapped properly so that Ropey again compiles on other platforms as well.


## [0.8.3] - 2018-07-26

### Performance improvements
- Significant speedups across the board by using SIMD for index conversions.
- Loading texts from files or creating Ropes from strings is now significantly faster.

### Memory usage improvements
- Memory overhead reduced from 17% to 10% for freshly loaded text.

### Bug fixes
- The low-level line -> byte conversion function would sometimes return a byte index in the middle of the line break for multi-byte line break characters.


## [0.8.2] - 2018-07-22

### Performance improvements
- File loading is slightly faster.

### Bug fixes
- The low-level line break counting functions could return an incorrect count under certain circumstances.  This also affected the higher-level methods in Ropey, although it was somewhat difficult to trigger in practice.


## [0.8.1] - 2018-07-20

### Performance improvements
- Increased Rope::insert() speed by roughly 1.4x for small insertion strings.
- Increased Rope::remove() speed by roughly 1.75x.

### Other
- General documentation improvements, based on feedback.


## [0.8.0] - 2018-07-14

### Performance improvements
- Building new ropes via RopeBuilder or Rope::from_str() is now about 15% faster.
- Slicing is now almost twice as fast.
- Fetching lines is now almost twice as fast.
- Significant speedups for byte/char -> line index conversion methods.
- Significant speedups for line -> byte/char index conversion methods.

### New features
- Chunk fetching can now be done by line break index as well as byte/char index.
- Some previously-internal utility functions for working with string slices are now part of Ropey's public API.
- Added Rope::write_to() convenience function for writing a Rope's data to a writer.

### Breaking changes
- Conversion from byte/char indices to line indices has been changed to be more intuitive.  It is now equivalent to counting the line endings before the given byte/char index.
- Chunk fetching now returns the starting byte/char/line of the chunk, which is generally easier to work with.


## [0.7.1] - 2018-07-09

### Bug fixes
- The chunk fetching methods on slices returned bogus starting char indices.


## [0.7.0] - 2018-07-05

### Performance improvements
- `RopeSlice`s have been given a major speed boost for small slices: for contiguous slices of text in memory, they will simply point at the text without any tree structure.  This makes it feasible to use `RopeSlice`s to yield e.g. graphemes or words, even in tight inner loops, while maintaining performance.

### New features
- You can now fetch contiguous chunks of text directly from `Rope`s and `RopeSlice`s, via byte or char index.  The chunk containing the given byte or char will be returned along with offset information.
- Added more index conversion methods.  For both `Rope`s and `RopeSlice`s, you can now convert between any of: byte, char, and line indices.
- Added a method to directly create `RopeSlice`s from string slices.  This isn't terribly useful when using Ropey's standard API's, but it allows for much more efficient implementations of things like custom iterators.
- Added a method to directly access a `RopeSlice`s text as a contiguous string slice when possible.  This is useful for client code to be able to make a fast-path branch for small slices that happen to be contiguous.  Like the above item, this can result in significant performance gains for certain use-cases.

### API breaking-changes
- All grapheme related APIs have been removed.  However, new APIs have been added that allow the efficient implementation of those same APIs on top of Ropey.  See the grapheme examples in the `examples` directory of the repo for working implementations.


## [0.6.3] - 2018-01-28

### Features
- Added a new `Rope::insert_char()` convenience method for inserting a single Unicode scalar value.

### Documentation
- Updated the Chunks iterator docs to accurately reflect the new segmentation API in 0.6.x.


## [0.6.2] - 2018-01-11

### Fixes
- 0.6.0 and 0.6.1 had an API regression where you now had to specify the
  segmenter in the type parameters of RopeSlice and the various iterators.


## [0.6.1] - 2018-01-11

- No functional changes.  Just updated the readme to render properly on crates.io.


## [0.6.0] - 2018-01-11

### New features
- Grapheme segmentation can now be customized if needed.

### API changes
- `Rope::remove()`, `Rope::slice()`, and `RopeSlice::slice()` now take range syntax to specify
  their ranges.


## [0.5.6] - 2018-01-05

### Documenation
- Added a design overview document to the repo, explaining Ropey's design.  Mainly targeted at potential contributors.
- Added a more integrated example of usage to the front page of the library docs.

### Features
- Fleshed out the `PartialEq` impls.  `Rope` and `RopeSlice` can now be compared for equality with not just `&str`, but also `String` and `Cow<str>`.

### Performance
- `Rope::char()`, which fetches a single Unicode scalar value as a `char`, is now several times faster.

### Misc
- This changelog had the wrong year on some of its dates.  Heh...


## [0.5.5] - 2017-12-30

### Bug fixes
- Comparing two empty ropes for equality would panic.

### New features
- Added Rope::capacity() and Rope::shrink_to_fit() methods.  Although these are probably of limited use, they may be useful in especially memory-constrained environments.


## [0.5.4] - 2017-12-30

### Bug fixes
- Rope::remove() didn't always merge graphemes between chunks properly.

### Performance and memory
- Inserting large texts into a rope now degrades in performance more gracefully as the insertion text becomes larger, rather than hitting a sudden performance cliff.
- `Rope::remove()` got a nice speed boost.
- Memory overhead has been reduced across the board.  Freshly loaded files now only have ~17% overhead, and the worst-case (built up from lots of small random-location inserts) is now ~60% overhead.

### Misc
- 100% unit test coverage of public APIs.
- Added randomized testing via [QuickCheck](https://crates.io/crates/quickcheck).
- Added benchmarks to the library.


## [0.5.3] - 2017-12-28

### Performance and memory
- Massive speed boost for small insertions: between %40 - %50 faster.
- `Rope::from_str()` now only uses stack memory for strings smaller than ~3MB. (Aside from the resulting Rope itself, of course.)

### Misc
- Better unit test coverage of public APIs.  Still not 100%, but getting there!


## [0.5.2] - 2017-12-25

### Bug fixes
- There were ocassionally unnecessary heap allocations that took up a small amount of extra space in the rope.

### Misc
- Memory overhead has been significantly reduced for ropes built up by many small coherent insertions.


## [0.5.1] - 2017-12-24

### Bug fixes
- Calling `Rope::line_to_char()` with a line index one-past-the-end would panic.  This wasn't consistent with other indexing, and has been fixed and now returns the one-past-the-end char index.
- Had accidentally left some asserts in the `Rope::remove()` code that were put in during debugging.  They were causing significant slow downs for removes.

### Misc
- Added a changelog file.


[Unreleased]: https://github.com/cessen/ropey/compare/v1.6.1...HEAD
[1.6.1]: https://github.com/cessen/ropey/compare/v1.6.0...v1.6.1
[1.6.0]: https://github.com/cessen/ropey/compare/v1.5.1...v1.6.0
[1.5.1]: https://github.com/cessen/ropey/compare/v1.5.0...v1.5.1
[1.5.0]: https://github.com/cessen/ropey/compare/v1.4.1...v1.5.0
[1.4.1]: https://github.com/cessen/ropey/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/cessen/ropey/compare/v1.3.2...v1.4.0
[1.3.2]: https://github.com/cessen/ropey/compare/v1.3.1...v1.3.2
[1.3.1]: https://github.com/cessen/ropey/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/cessen/ropey/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/cessen/ropey/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/cessen/ropey/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/cessen/ropey/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/cessen/ropey/compare/v0.9.2...v1.0.0
[0.9.2]: https://github.com/cessen/ropey/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/cessen/ropey/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/cessen/ropey/compare/v0.8.4...v0.9.0
[0.8.4]: https://github.com/cessen/ropey/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/cessen/ropey/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/cessen/ropey/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/cessen/ropey/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/cessen/ropey/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/cessen/ropey/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/cessen/ropey/compare/v0.6.3...v0.7.0
[0.6.3]: https://github.com/cessen/ropey/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/cessen/ropey/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/cessen/ropey/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/cessen/ropey/compare/v0.5.6...v0.6.0
[0.5.6]: https://github.com/cessen/ropey/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/cessen/ropey/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/cessen/ropey/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/cessen/ropey/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/cessen/ropey/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/cessen/ropey/releases/tag/v0.5.1
