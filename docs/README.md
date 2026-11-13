# Table of Contents

[🏠 Home](/README.md)

___

## Introduction

Classeq is an alignment-free phylogenetic placer of sequences. It places
sequences on a reference tree from their k-mer composition, without aligning
them.

It is made to classify many short reads of a protein-coding marker when the
reference does not cover everything in the sample: it runs at the speed and
memory of a k-mer tool, and it refuses sequences from outside the reference
with a threshold it calibrates from the reference itself.
[When to use classeq2](/docs/book/03-place-sequence-cli.md#36-when-to-use-classeq2)
gives the comparison with other tools and the cases where another tool is the
better choice.

The placement process should be done from the command line interface (CLI) or
using the application programming interface (API) server. The CLI is the primary
way to use the tool, but the API server can be used to place sequences in a
distributed way.

- [01. Installation ⚙️](/docs/book/01-installation.md)
- [02. Build Database 🏗️](/docs/book/02-build-db.md): rooting, building, the database format, `cls assess`
- [03. Place Sequences with CLI </>](/docs/book/03-place-sequence-cli.md): output, the evidence rule, calibrated databases, refused queries, when to use classeq2
- [04. Configure the API server 🔧](/docs/book/04-configure-api-server.md)
- [05. Place Sequences using the API 🔗](/docs/book/05-submit-placement-to-api.md)
- [06. Telemetry and Benchmarking Classeq 🕒](/docs/book/06-telemetry-and-benchmark.md)
