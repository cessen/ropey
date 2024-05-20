#!/bin/sh

echo -e "\n\n\n\n\n\n\n\nMETRICS: none\n" \
&& cargo build --no-default-features --features "simd" \
\
&& echo -e "\n\n\n\n\n\n\n\nMETRICS: metric_chars\n" \
&& cargo build --no-default-features --features "simd, metric_chars" \
\
&& echo -e "\n\n\n\n\n\n\n\nMETRICS: metric_utf16\n" \
&& cargo build --no-default-features --features "simd, metric_utf16" \
\
&& echo -e "\n\n\n\n\n\n\n\nMETRICS: metric_lines_lf\n" \
&& cargo build --no-default-features --features "simd, metric_lines_lf" \
\
&& echo -e "\n\n\n\n\n\n\n\nMETRICS: metric_lines_cr_lf\n" \
&& cargo build --no-default-features --features "simd, metric_lines_cr_lf" \
\
&& echo -e "\n\n\n\n\n\n\n\nMETRICS: metric_lines_unicode\n" \
&& cargo build --no-default-features --features "simd, metric_lines_unicode" \
\
&& echo -e "\n\n\n\n\n\n\n\nMETRICS: metric_chars, metric_utf16, metric_lines_lf, metric_lines_cr_lf, metric_lines_unicode\n" \
&& cargo build --no-default-features --features "simd, metric_chars, metric_utf16, metric_lines_lf, metric_lines_cr_lf, metric_lines_unicode"
