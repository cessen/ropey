#!/bin/sh

RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --all-features "$@"
