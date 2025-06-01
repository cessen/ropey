# Changelog


## [Unreleased]

- Move esoteric functionality to new `extra` module.
- Add function to check if two ropes are instances of each other.
- Allow constructing `RopeSlice`s directly from `&str` slices.
- "Owning slices" can now be edited, which implicitly converts them to normal ropes.
- Improved error reporting.
- Misc documentation improvements.
- Misc bug fixes.


## [2.0.0 alpha 2] - 2024-10-21

### Bug fixes

- Fixed `chunks_at()`.


## [2.0.0 alpha 1] - 2024-10-20

- Major rewrite of Ropey, with new APIs designed based on what we've learned from years of using Ropey 1.x.

### Main changes from Ropey 1.x

- **Indexing is now byte based** rather than char based.  For example, text insertion, text removal, and slicing are all done with byte indices now.  Even fetching chars is now done by byte index.  If you still want or need to work in terms of char indices, you can do so by using the index conversion functions to convert between byte and char indices as needed.
- **The index conversion functions are now byte-to-metric and metric-to-byte** rather than metricA-to-metricB.  The latter can still be accomplished by using byte indices as an intermediate.
- **The chunk fetching API's have been stripped down.**  For example, there are no longer `chunk_at_line()`, etc. functions for fetching chunks based on arbitrary indexing metrics, instead being replaced by just `chunk_at()` which fetches using only byte indices.  Fetching based on arbitrary metrics can still be accomplished by combining `chunk_at()` with the index conversion functions.
- **All indexing metrics other than byte index are now behind feature flags**, and can be enabled or disabled individually as desired.  Of those, only LF-CR lines are enabled by default.  Notably, this means that the char indexing metric is not enabled by default.
- **The line metric feature flags are now properly additive,** and multiple line indexing metrics can be tracked simultaneously.  Because of this, all line-based APIs now take a `LineType` parameter that specifies which of the available metrics to use.


[Unreleased]: https://github.com/cessen/ropey/compare/v2.0.0-alpha.2...HEAD
[2.0.0 alpha 2]: https://github.com/cessen/ropey/compare/v2.0.0-alpha.1...v2.0.0-alpha.2
[2.0.0 alpha 1]: https://github.com/cessen/ropey/releases/tag/v2.0.0-alpha.1
