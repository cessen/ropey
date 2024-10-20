#!/bin/sh

# Run external tests with small chunks.  Cargo doesn't compile the
# main crate (Ropey) with "test" enabled for external tests, so we
# need to do that manually.
cargo test --features "internal_dev_small_chunks, simd, metric_chars, metric_utf16, metric_lines_lf, metric_lines_lf_cr, metric_lines_unicode" --test '*' "$@"
