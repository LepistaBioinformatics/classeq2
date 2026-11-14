# Classeq: A phylogenetic placer of sequences

Classeq is an alignment-free phylogenetic placer of sequences. It places
sequences on a reference tree from their k-mer composition, without aligning
them.

It is made to classify many short reads of a protein-coding marker when the
reference does not cover everything in the sample: it runs at the speed and
memory of a k-mer tool, and it refuses sequences from outside the reference
with a threshold it calibrates from the reference itself, with no setting to
choose.
[When to use classeq2](/docs/book/03-place-sequence-cli.md#36-when-to-use-classeq2)
gives the comparison with other tools and the cases where another tool is the
better choice.

> **_NOTE:_** This tool are in development, don't use in production.

## Visit the Classeq book

The Classeq book contains the documentation of the tool, including installation,
usage, and API documentation. This book is a work in progress, and we are
constantly updating it.

[▶️ 📚 Book](/docs/README.md)

## License

Classeq is licensed under the [Apache License, Version 2.0](LICENSE).
