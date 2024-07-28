# 3. Place Sequences with CLI

[🏠 Home](/README.md)

[📋 Summary](/docs/README.md)

---

## 3.1 Build the Database

After building the database, you can place sequences using the command line
interface (CLI). The CLI is the primary way to use the tool, but the API server
can be used to place sequences in a distributed way.

The CLI is a simple tool that reads a set of sequences from a file and places
them on the reference tree. The CLI requires the database system filepath and
the sequences to be placed. The CLI can be used as follows:

```bash
cls place \ 
    -d cls-database-name \ 
    -o placed_sequences.yaml \ 
    sequences.fasta
```

Alternatively, you can pipe the sequences to the CLI using the standard input.
The following example shows how to place sequences using the standard input:

```bash
cat sequences.fasta | cls place \ 
    -d cls-database-name \ 
    -o placed_sequences.yaml
```

The help command can be used to show the available options and arguments for the
`cls place` command.

## 3.2 Output format

The default output format of the CLI is a YAML file containing the placed
sequences. As an example, the output file can be as follows:

```yaml
# Example of fully classified sequence
#
# Occurs when the query sequence is placed at the terminal node of the reference
# tree. The placement is considered fully resolved.
---
query: NC_014639_Bacillus_atrophaeus_1942_complete
code: IdentityFound
placement:
  clade:
    id: 61
    kind: NODE
    support: 72.0
    length: 0.000619153
    children:
    - id: 62
      name: NZ_JALAVT010000005_Bacillus_atrophaeus_strain_TP10S1NI5
      kind: LEAF
      length: 1e-6
    - id: 63
      name: NZ_JALAPU010000003_Bacillus_atrophaeus_strain_TP8F7aA3
      kind: LEAF
      length: 0.00123804
  oneLen: 3754
  restLen: 0
  restAvg: 0.0
  restMax: 0

# Example of partially classified sequence
#
# Occurs when the sequence is placed in a internal node of the reference tree.
---
query: NC_000964_Bacillus_subtilis_subsp_subtilis
code: 'MaxResolutionReached: LCA Accepted'
placement: 149

# Example of unclassified sequence
#
# Occurs when more than one internal node is proposed as the placement of the
# query sequence.
---
query: NC_000964_Bacillus_subtilis_subsp_subtilis
code: 'Inconclusive'
placement:
  - id: 95
    kind: NODE
    support: 100.0
    length: 0.173095
    children:
    - id: 96
      kind: NODE
      support: 89.0
      length: 0.0377908
    - id: 97
      kind: NODE
      support: 88.0
      length: 0.00273586

# Example of Unclassifiable sequence
#
# Occurs when the query sequence has no overlapping kmers with the reference
# tree OR the minimum number of overlapping kmers is not reached (controlled by 
# the -m option).
---
query: NC_000964_Bacillus_subtilis_subsp_subtilis
code: 'Unclassifiable: Query sequence has no overlapping kmers with the reference tree'
```

---

[◀️ Prev | Build Classeq Database](/docs/book/02-build-db.md)

[▶️ Next | Configure API Server](/docs/book/04-configure-api-server.md)
