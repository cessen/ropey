#!/bin/sh

cargo test --no-default-features --features "simd, metric_chars, metric_utf16, metric_lines_lf, metric_lines_cr_lf, metric_lines_unicode" "$@"
