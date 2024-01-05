#!/bin/sh

# Run integration tests with small chunks.  Cargo doesn't compile the
# main crate (Ropey) with "test" enabled for integration tests, so we
# need to do that manually.
cargo test --features "small_chunks, simd, metric_chars, metric_utf16, metric_lines_lf, metric_lines_cr_lf, metric_lines_unicode" --test '*'
