#!/bin/sh

cargo test --no-default-features --features "simd, metric_lines_unicode" "$@"
