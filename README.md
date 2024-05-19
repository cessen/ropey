# Ropey 2

This is the (very) WIP next major version of Ropey.  DO NOT USE THIS for anything even remotely serious.  This is pre-alpha.

## TODO

- [x] Insertion.
- [x] Removal.
- [x] Change line APIs to take an enum that determines which kind of lines.
- [x] Rope length queries.
- [x] Tree rebalancing.
- [x] Chunk fetching function.
- [x] Try rewriting `RopeBuilder` to be cleaner/faster.
- [x] `RopeSlice`
- [ ] "Owned slices": full Ropes but that store meta data about a sliced range, so that owned slices (that don't depend on the lifetime of the original rope) can be created.
- Metric conversion functions:
  - [x] Chars <-> bytes.
  - [x] UTF16 <-> bytes.
  - [x] Lines <-> bytes.
- Iterators:
  - [ ] `Chunks`
    - [x] Forward.
    - [ ] Bidirectional.
    - [ ] TextInfo querying.
  - [ ] `Bytes`
    - [x] Forward.
    - [ ] Bidirectional.
  - [ ] `Chars`
    - [x] Forward.
    - [ ] Bidirectional.
  - [ ] `Lines`:
    - [ ] LF
      - [ ] Forward.
      - [ ] Bidirectional.
    - [ ] LF + CR
      - [ ] Forward.
      - [ ] Bidirectional.
    - [ ] Full Unicode
      - [ ] Forward.
      - [ ] Bidirectional.
  - [ ] Creating iterators at a specific offset.
- [x] Standard library trait impls:
  - [x] `From`:
    - [x] `RopeSlice` -> `String`
    - [x] `RopeSlice` -> `Option<str>`
    - [x] `RopeSlice` -> `Cow<str>`
    - [x] `RopeSlice` -> `Rope`
    - [x] `Rope` -> `RopeSlice`
    - [x] `Rope` -> `String`
    - [x] `Rope` -> `Option<str>`
    - [x] `Rope` -> `Cow<str>`
    - [x] `String` -> `Rope`
    - [x] `str` -> `Rope`
    - [x] `Cow<str>` -> `Rope`
  - [x] `Hash`
    - [x] `Rope`
    - [x] `RopeSlice`.
  - [x] Comparison operators:
    - [x] `Eq` / `PartialEq`
      - [x] `Rope` <-> `Rope`
      - [x] `Rope` <-> `RopeSlice`
      - [x] `Rope` <-> `str`
      - [x] `Rope` <-> `String`
      - [x] `Rope` <-> `Cow<str>`
      - [x] `RopeSlice` <-> `RopeSlice`
      - [x] `RopeSlice` <-> `str`
      - [x] `RopeSlice` <-> `String`
      - [x] `RopeSlice` <-> `Cow<str>`
    - [x] `Ord` / `PartialOrd`
      - [x] `Rope`
      - [x] `RopeSlice`


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
