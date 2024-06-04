#!/usr/bin/bash

ARGS=("$@")

cargo publish -p classeq-core $ARGS
cargo publish -p classeq-cli $ARGS
