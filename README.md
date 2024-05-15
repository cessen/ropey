# Ropey 2

This is the (very) WIP next major version of Ropey.  DO NOT USE THIS for anything even remotely serious.  This is pre-alpha.

## TODO

- [x] Insertion.
- [x] Removal.
- [x] Change line APIs to take an enum that determines which kind of lines.
- [x] Rope length queries.
- [ ] Tree rebalancing.
- [ ] Chunk fetching functions.
- [x] Try rewriting `RopeBuilder` to be cleaner/faster.
- [x] `RopeSlice`
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
    - [ ] Forward.
    - [ ] Bidirectional.
  - [ ] `Chars`
    - [ ] Forward.
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
- Standard library trait impls:
  - [ ] `From`:
    - [ ] `RopeSlice` -> `String`
    - [ ] `RopeSlice` -> `Option<str>`
    - [ ] `RopeSlice` -> `Cow<str>`
    - [ ] `RopeSlice` -> `Rope` (using extra metadata in the rope to make this trivial--metadata then gets discarded and actual trimming happens on first edit).
    - [ ] `Rope` -> `RopeSlice`
    - [x] `Rope` -> `String`
    - [x] `Rope` -> `Cow<str>`
    - [x] `String` -> `Rope`
    - [x] `str` -> `Rope`
    - [x] `Cow<str>` -> `Rope`
  - [ ] `Hash`
    - [x] `Rope`
    - [ ] `RopeSlice`.
  - [ ] Comparison operators:
    - [ ] `Eq` / `PartialEq`
      - [ ] `Rope` <-> `Rope`
      - [ ] `Rope` <-> `RopeSlice`
      - [x] `Rope` <-> `str`
      - [x] `Rope` <-> `String`
      - [x] `Rope` <-> `Cow<str>`
      - [x] `RopeSlice` <-> `str`
      - [x] `RopeSlice` <-> `String`
      - [x] `RopeSlice` <-> `Cow<str>`
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
