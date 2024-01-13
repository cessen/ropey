# Ropey 2

This is the (very) WIP next major version of Ropey.  DO NOT USE THIS for anything even remotely serious.  This is pre-alpha.

## TODO

- [x] Insertion.
- [x] Removal.
- [x] Change line APIs to take an enum that determines which kind of lines.
- [x] Rope length queries.
- [ ] Tree rebalancing.
- [ ] Chunk fetching functions.
- [ ] Try rewriting `RopeBuilder` to be cleaner/faster.
- [ ] `RopeSlice`
- Metric conversion functions:
  - [ ] Chars <-> bytes.
  - [ ] UTF16 <-> bytes.
  - [ ] Lines <-> bytes.
- Iterators:
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
- Standard library trait impls:
  - [ ] `From`:
    - [ ] `RopeSlice` -> `String`
    - [ ] `RopeSlice` -> `Option<str>`
    - [ ] `RopeSlice` -> `Cow<str>`
    - [ ] `RopeSlice` -> `Rope`
    - [ ] `Rope` -> `RopeSlice`
    - [x] `Rope` -> `String`
    - [x] `Rope` -> `Cow<str>`
    - [x] `String` -> `Rope`
    - [x] `str` -> `Rope`
    - [x] `Cow<str>` -> `Rope`
  - [ ] `Hash`
    - [ ] `Rope`
    - [ ] `RopeSlice`.
  - [ ] Comparison operators:
    - [ ] `Eq` / `PartialEq`
      - [ ] `Rope` <-> `Rope`
      - [ ] `Rope` <-> `RopeSlice`
      - [x] `Rope` <-> `str`
      - [ ] `Rope` <-> `String`
      - [ ] `Rope` <-> `Cow<str>`
      - [ ] `RopeSlice` <-> `str`
      - [ ] `RopeSlice` <-> `String`
      - [ ] `RopeSlice` <-> `Cow<str>`
    - [ ] `Ord` / `PartialOrd`
      - [ ] `Rope`
      - [ ] `RopeSlice`


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
