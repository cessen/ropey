# Ropey 2

This is the WIP next major version of Ropey.  DO NOT USE THIS for anything serious.  This is alpha software, meaning both that the API is not fully stable yet and that it is likely pretty buggy.


## Differences from Ropey 1.x

There are many breaking API changes in Ropey 2.x.  However, the major ones are:

- With the exception of some line-based APIs, all indexing is now done with **byte indices** rather than char indices.  For example, text insertion, text removal, and slicing are all done with byte indices now.  Even fetching chars is now done by byte index.  If you still want or need to work in terms of char indices, you can do so by using the index conversion functions to convert between byte and char indices as needed.
- The index conversion functions now only convert between byte-to-metric and metric-to-byte rather than metricA-to-metricB.  The latter can still be accomplished by using byte indices as an intermediate.
- The chunk fetching API's have been stripped down.  For example, there are no longer `chunk_at_line()`, etc. functions for fetching chunks based on arbitrary indexing metrics, instead being replaced by just `chunk_at()` which fetches using only byte indices.  Fetching based on arbitrary metrics can still be accomplished by combining `chunk_at()` with the index conversion functions.
- All indexing metrics other than byte index are now behind feature flags, and can be enabled or disabled individually as desired.  Of those, only LF-CR lines are enabled by default.  Notably, this means that the char indexing metric is not enabled by default.
- The line metric feature flags are now properly additive, and multiple line indexing metrics can be tracked simultaneously.  Because of this, all line-based APIs now take a `LineType` parameter that specifies which of the available metrics to use.


## What does this mean for Ropey 1.x?

Ropey 1.x will continue to be maintained for the foreseeable future, but will no longer receive new features. Ropey 1.x is still a good, high-quality rope library that can be depended on, and there is no urgency to move to Ropey 2.x.

If at some point maintenance of Ropey 1.x stops, it will be with plenty of advance warning.


## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.


## Contributing

Contributions are **NOT** currently welcome from anyone outside of the dev team.  All PRs, no matter how good, no matter how seemingly obvious or minor, will be rejected without review.  Issues are also likely to be immediately closed.

Ropey 2 will become open to contributions once it's further along and in a useable state.
