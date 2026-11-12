# 3. Place Sequences with CLI </>

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

Placement descends the tree from the root. At each node, every child, leaves
included, is scored by the number of query k-mers indexed under it (with
`--remove-intersection`, only the k-mers that no sibling shares). A unique best
child is selected and the descent continues below it. In each placement record,
`one` is the score of the selected clade and `rest` is the score of its
strongest sibling.

```yaml
# Ids and k-mer counts below are illustrative.
# Every placement is an object tagged by `status`: IdentityFound,
# MaxResolutionReached, Inconclusive or Unclassifiable.

# Example of fully classified sequence
#
# Occurs when the descent ends at a leaf of the reference tree: at every level
# one child scored strictly higher than all of its siblings.
---
query: NC_014639_Bacillus_atrophaeus_1942_complete
code: IdentityFound
placement:
  status: IdentityFound
  clade:
    id: 62
    parent: 61
    kind: LEAF
    name: NZ_JALAVT010000005_Bacillus_atrophaeus_strain_TP10S1NI5
    length: 1e-6
  one: 3754
  rest: 3101

# Example of partially classified sequence
#
# Occurs when no child of the current node carries any evidence for the query,
# which in practice happens only with --remove-intersection, when every
# matching k-mer is shared between siblings. The sequence is placed at that
# internal node.
---
query: NC_000964_Bacillus_subtilis_subsp_subtilis
code: 'MaxResolutionReached: LCA Accepted'
placement:
  status: MaxResolutionReached
  clade: 149
  message: LCA Accepted

# Example of inconclusive sequence
#
# Occurs when two or more children of an internal node tie for the best score.
# Each proposal carries the clade id and its k-mer counts; for tied proposals
# `one` equals `rest`. The parent of the proposals is the deepest node the
# query can be placed at with confidence.
---
query: NC_000964_Bacillus_subtilis_subsp_subtilis
code: 'Inconclusive: Multiple proposals'
placement:
  status: Inconclusive
  proposals:
  - clade: 96
    one: 1520
    rest: 1520
  - clade: 97
    one: 1520
    rest: 1520
  message: Multiple proposals

# Example of Unclassifiable sequence
#
# Occurs when the query sequence has no overlapping kmers with the reference
# tree, when the fraction of the query k-mers found in the tree is below the
# minimum match coverage (controlled by the -m option, default 0.7), or when
# the children of the root tie for the best score. The placement field is
# omitted and the reason is reported in `code`.
---
query: NC_000964_Bacillus_subtilis_subsp_subtilis
code: 'Unclassifiable: Query sequence has no overlapping kmers with the reference tree'
```

## 3.3 The evidence rule

By default the descent follows the child with the most query k-mers, by any
margin. A query from a lineage that the reference lacks then ends inside a
sibling's clade, for example in a sister species.

With `--evidence-rule` (`-e`), the descent enters a child only on k-mers
specific to it: indexed under that child, under none of its siblings, and under
no clade left behind on the way from the root. The child must hold at least
`--min-evidence` of them (default 2), and its strongest sibling at most
`--max-rival-ratio` times as many (default 0.2; lower is stricter). Otherwise
the placement stops at the current clade with `MaxResolutionReached`. A tie
never happens, so no record is `Inconclusive`.

Each placed record then carries a `scores` object:

```yaml
---
query: NC_000964_Bacillus_subtilis_subsp_subtilis
code: 'MaxResolutionReached: Evidence below threshold'
placement:
  status: MaxResolutionReached
  clade: 149
  message: Evidence below threshold
scores:
  # Share of the query's distinct k-mers indexed under the placed clade
  coverage: 0.14
  # Specific k-mers of the last clade the descent entered, and of that
  # clade's strongest sibling
  evidence: 16
  rival: 0
```

For `IdentityFound`, `one` and `rest` hold the same two counts as `evidence`
and `rival`.

## 3.4 Calibrated databases

`cls build-db` measures two things on the reference and stores them in the
database:

- the mean number of distinct k-mers of a leaf;
- for every internal clade, the lowest share of a leaf's k-mers that the rest
  of the clade also holds, over the clade's leaves (its containment).

Placement uses them as follows.

- **Coverage of long queries.** A query with more k-mers than a typical leaf,
  such as a genome placed on a database of genes, has its coverage measured
  against the leaf's count instead of its own. A query that covers only part
  of the reference loci, such as one scaffold, is underestimated.
- **Default match coverage.** Without `-m`, the threshold is a quarter of the
  root's containment: a query passes when it shares with the tree at least a
  quarter of what the tree's most distant leaf shares with it. Databases without calibration
  keep the 0.7 default.
- **Novelty test of the evidence rule.** The descent enters the children of a
  clade only when the query's coverage under that clade reaches
  `--min-containment-ratio` (default 0.25) times the clade's containment.
  Otherwise the query is taken to sit outside the clade's crown, and the
  placement stops at the clade.

---

[◀️ Prev | Build Classeq Database](/docs/book/02-build-db.md)

[▶️ Next | Configure API Server](/docs/book/04-configure-api-server.md)
