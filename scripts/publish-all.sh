#!/usr/bin/bash

ARGS=("$@")

cargo publish -p classeq-core --allow-dirty --dry-run $ARGS
cargo publish -p classeq-cli --allow-dirty --dry-run $ARGS
