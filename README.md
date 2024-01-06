# Ropey 2

This is the (very) WIP next major version of Ropey.  DO NOT USE THIS for anything even remotely serious.  This is pre-alpha.

## TODO

- [x] Insertion.
- [x] Removal.
- [ ] Tree rebalancing.
- [x] Change line APIs to take an enum that determines which kind of lines.
- [ ] `RopeSlice`
- [ ] Iterators:
  - [ ] `Chunks`
    - [x] Forward.
    - [ ] Bidirectional.
    - [ ] TextInfo querying.
  - [ ] `Bytes`
  - [ ] `Chars`
  - [ ] `Lines`:
    - [ ] LF
    - [ ] LF + CR
    - [ ] Full Unicode
  - [ ] Creating iterators at a specific offset.
- [x] Rope length queries.
- [ ] Metric conversion functions:
  - [ ] Chars <-> bytes.
  - [ ] UTF16 <-> bytes.
  - [ ] Lines <-> bytes.
- [ ] Chunk fetching functions.
- [ ] Try rewriting `RopeBuilder` to be cleaner.
- [ ] Conversion implementations:
  - [ ] `RopeSlice` -> `String`
  - [ ] `RopeSlice` -> `Option<str>`
  - [ ] `RopeSlice` -> `Cow<str>`
  - [ ] `RopeSlice` -> `Rope`
- [ ] Implement `Hash` for `Rope` and `RopeSlice`.
- [ ] Comparison operators:
  - [ ] `Eq` / `PartialEq`
  - [ ] `Ord` / `PartialOrd`


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
