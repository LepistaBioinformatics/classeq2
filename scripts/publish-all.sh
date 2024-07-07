#!/usr/bin/bash

ARGS=("$@")

cargo publish -p classeq-core $ARGS
cargo publish -p classeq-ports-lib $ARGS
cargo publish -p classeq-cli $ARGS
cargo publish -p classeq-api $ARGS
cargo publish -p classeq-watcher $ARGS
