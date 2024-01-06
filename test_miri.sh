#!/bin/sh

# None of the features are relevant to unsafe code in Ropey, so we disable them to make things run faster.
cargo +nightly miri test --no-default-features
