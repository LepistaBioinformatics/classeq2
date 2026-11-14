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

`cls build-db` measures three things on the reference and stores them in the
database:

- the mean number of distinct k-mers of a leaf;
- for every internal clade, the lowest share of a leaf's k-mers that the rest
  of the clade also holds, over the clade's leaves (its containment);
- the root crossing: the median share of a leaf's k-mers that leaves under
  another child of the root hold. It is measured on whole leaves and on
  windows of 100, 150, 250, 500 and 1000 bases, so that a read is compared
  with windows of its own length.

Placement uses them as follows.

- **Coverage of long queries.** A query with more k-mers than a typical leaf,
  such as a genome placed on a database of genes, has its coverage measured
  against the leaf's count instead of its own. A query that covers only part
  of the reference loci, such as one scaffold, is underestimated.
- **Default match coverage.** Without `-m`, a query passes when it shares
  with the tree at least as much as the tree's most distant leaf shares with
  it (the root's containment), or as much as the two sides of the root share
  with each other (the root crossing), whichever is lower. The threshold is
  never below a quarter of the root's containment, which is also the
  threshold of a database built before the root crossing existed. This
  refuses sequences of a sister group of the reference; on a deep reference,
  where the two sides of the root share almost nothing, it changes nothing.
  Databases without calibration keep the 0.7 default.
- **Novelty test of the evidence rule.** The descent enters the children of a
  clade only when the query's coverage under that clade reaches
  `--min-containment-ratio` (default 0.25) times the clade's containment.
  Otherwise the query is taken to sit outside the clade's crown, and the
  placement stops at the clade.

## 3.5 Refused queries

A query that classeq2 does not place is `Unclassifiable`. With the evidence
rule on a calibrated database there are three reasons, given in the message:

| Message | Meaning |
|---|---|
| `may not be related to the phylogeny` | No k-mer of the query is in the database. |
| `Insufficient kmers coverage` | The query shares fewer k-mers with the tree than the default match coverage of [3.4](#34-calibrated-databases) asks. This is the test that measures distance. |
| `No decisive evidence among the root children` | The query is related to the tree, but no child of the root holds clearly more k-mers of its own than the others. Sequences of a sister group of the reference often end here. |

Refused queries are the intended answer for sequences from outside the
reference. Use `-m` to set the coverage threshold by hand.

The third reason depends on where the tree is rooted: it refuses a distant
sequence only when the root splits the reference into sides that match it
about equally. Root the tree at the midpoint or with an outgroup
([2.1](/docs/book/02-build-db.md#21-root-the-reference-tree)), not at an arbitrary leaf.

## 3.6 When to use classeq2

classeq2 was compared with EPA-ng, pplacer, APPLES-2, EPIK, App-SpaM and BLAST
on five protein-coding genes of two bacterial groups (the *Bacillus subtilis*
group and *Xanthomonas*), GAPDH of *Colletotrichum* and 18S of eukaryotes.
For 150-base reads of a protein-coding gene, with `--evidence-rule`; cells
with two values are the first and the second bacterial group:

| | classeq2 | EPIK | BLAST | EPA-ng |
|---|---|---|---|---|
| Threshold to choose | none | yes | yes | yes |
| Reads per second, one thread | 19,000 | 39,000 | 258 | 44 |
| Memory | 23 MB | 47 MB | 1.3 GB | 0.3 GB |
| Reads of other genera accepted, reference of a clade | 0% / 4% | 50% / 28% | 0% / 1% | 0% / 0.7% |
| Reads of a sister group at 85% to 90% ANI accepted | 15% / 26% | 76% / 22% | 17% / 14% | 13% / 13% |
| Reads of other genera accepted, reference of the whole genus | 0.8% / 36% | 20% / 92% | 0% / 25% | 0% / 18% |
| False annotations, in-lineage reads | 22% / 31% | 27% / 47% | 1% / 0.2% (with LCA) | 42% / 65% |
| Exact clade, in-lineage reads | 21% / 14% | 35% / 23% | 8% / 2% (with LCA) | 32% / 22% |

The other tools are cut on their own score where they refuse as many
in-lineage reads as classeq2 does.

**Use classeq2** to classify many short reads of a protein-coding marker
when the reference does not cover everything in the sample: it is fast, it
needs no alignment of the reads, and it refuses sequences from outside the
reference without a threshold to set, which EPIK, the other fast
alignment-free placer, cannot do.

**How well it refuses depends on the reference.** The threshold comes from
how much the two sides of the root share with each other. When the reference
holds clades as distant from each other as the outsiders are from it (the
whole genus *Xanthomonas*, whose two clades are under 80% ANI apart), the
threshold is low and reads of neighbouring genera pass; BLAST or a placer
filtered on pendant length then refuse more. A reference of one clade, or
of a group whose members are closer to each other than to anything outside,
refuses as well as they do. For such deep references, set `-m` by hand or
build one reference per clade.

**Use another tool when:**

- the placements must be as deep as possible and the sample is known to be
  in lineage (EPIK, or EPA-ng and pplacer);
- a false annotation costs more than time (BLAST with a lowest common
  ancestor);
- the queries are full-length genes: EPA-ng or pplacer, filtered on pendant
  length, are as safe and reach the exact clade 2.5 to 4 times more often;
- the marker is conserved over a deep, sparsely sampled tree, such as 18S
  across a kingdom.

`cls assess` ([2.6](/docs/book/02-build-db.md#26-check-a-reference-before-building))
tells which case a reference falls in before any query is placed. It
measures in-lineage queries only: it does not tell how the reference will
treat outsiders.

These numbers come from two bacterial groups and two other markers. They
depend on the reference: check yours with `cls assess`, and with sequences
you know to be outside it.

---

[◀️ Prev | Build Classeq Database](/docs/book/02-build-db.md)

[▶️ Next | Configure API Server](/docs/book/04-configure-api-server.md)
