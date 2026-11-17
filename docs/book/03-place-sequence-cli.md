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

### Choosing `--min-evidence`

`--min-evidence` sets how sure the descent must be before it enters a child.
With a higher value, a doubtful step is not taken: the query stays at the
parent clade, with `MaxResolutionReached: Evidence below threshold`. A
placement that would have been false, in a sister species for example, becomes
a right one at a higher level. The price is resolution: some placements that
were right at the deepest clade also stay one level up.

The default of 2 keeps the most resolution. The table shows what higher values
do to queries from lineages the reference does not hold, which is where false
placements come from. "False" is the share of placed queries that end inside a
wrong clade; "right" is the share of all queries placed at the clade their
lineage attaches to or above it; "exact" is the share placed at that clade.

| `--min-evidence` | 150 bases: false | right | exact | 250 bases: false | right | exact | Full gene: false | right | exact |
|---|---|---|---|---|---|---|---|---|---|
| 2 (default) | 32% | 29% | 8.3% | 29% | 30% | 9.0% | 17% | 36% | 12.1% |
| 16 | 26% | 31% | 7.8% | 25% | 32% | 8.6% | 16% | 37% | 12.2% |
| 32 | 21% | 32% | 7.2% | 21% | 33% | 8.2% | 13% | 38% | 12.4% |
| 64 | 14% | 32% | 5.6% | 14% | 34% | 7.0% | 11% | 39% | 12.5% |
| 96 | 10% | 29% | 3.5% | 9% | 34% | 4.9% | 8% | 40% | 11.4% |
| 128 | 11% | 25% | 2.4% | 8% | 32% | 4.0% | 8% | 40% | 11.0% |

When to raise it:

- **Reads from a sample that may hold lineages the reference lacks**, such as
  an environmental sample or a product under quality control, when a false
  call at the species is worse than a right call at the genus. Use 32 to 64
  for reads of 150 bases and 64 to 96 for reads of 250 bases. This halves the
  false placements or better, and the share of right placements does not
  fall.
- **Full-length genes or longer queries**, with the same concern: 64 to 128.

When to leave it at 2:

- **Queries expected to belong to species the reference holds**, when the
  deepest clade is what you want, as when confirming the identity of an
  isolate.
- **Queries shorter than about 100 bases.** The value is a count of k-mers, so
  it must stay well under what the query has. A query of `L` bases has
  `2 × (L − k + 1)` k-mers, both strands counted: 232 for 150 bases at the
  default `k` of 35. Keep `--min-evidence` under a quarter of that. At 256,
  every read of 150 bases is refused.

What it does not do: it does not refuse sequences from outside the reference
clade. That is the job of the calibrated coverage (section 3.4), and the share
of outside sequences placed is the same at every value in the table.

To see the effect on your own reference before placing anything, run
`cls assess` with the same option (section 2.6 of the `build-db` chapter): it
reports the exact, under, false and refused shares for a missing leaf and for
a missing clade.

The numbers come from 1,249 tests on 25 references of five bacterial groups
(five marker genes each of the *Bacillus subtilis* group, *Bacteroides*,
*Burkholderia*, *Streptomyces* and *Xanthomonas*). In each test a clade was
removed from the reference and its sequences were placed. Every value from 8
to 64 lowered the false placements in all five groups, least in
*Burkholderia* (43% to 25% at 64, reads of 150 bases). Other groups and other
kinds of marker were not tested.

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
rule on a calibrated database there are four reasons, given in the message:

| Message | Meaning |
|---|---|
| `may not be related to the phylogeny` | No k-mer of the query is in the database. |
| `Insufficient kmers coverage` | The query shares fewer k-mers with the tree than the default match coverage of [3.4](#34-calibrated-databases) asks. This is the test that measures distance. |
| `Insufficient kmers coverage under the root child` | The reference is deep: the two sides of its root share with each other under a quarter of what a side shares within itself. The query shares less with the side it enters than nearly all of that side's own leaves do (their 5th percentile, measured at `build-db`). Sequences of neighbouring genera end here on a reference of a whole genus, and so do many species the reference does not hold. |
| `No decisive evidence among the root children` | The query is related to the tree, but no child of the root holds clearly more k-mers of its own than the others. Sequences of a sister group of the reference often end here. |

Refused queries are the intended answer for sequences from outside the
reference. Use `-m` to set the coverage threshold by hand.

The third reason depends on where the tree is rooted: it refuses a distant
sequence only when the root splits the reference into sides that match it
about equally. Root the tree at the midpoint or with an outgroup
([2.1](/docs/book/02-build-db.md#21-root-the-reference-tree)), not at an arbitrary leaf.

## 3.6 When to use classeq2

classeq2 was compared with EPA-ng, pplacer, APPLES-2, EPIK, App-SpaM and BLAST
on five protein-coding genes of five bacterial groups (the *Bacillus subtilis*
group, *Xanthomonas*, *Bacteroides*, *Burkholderia* and *Streptomyces*), GAPDH
of *Colletotrichum* and 18S of eukaryotes. For 150-base reads of a
protein-coding gene, with `--evidence-rule`; cells with a range are the median
of the five groups, with the lowest and the highest group:

| | classeq2 | EPIK | BLAST | EPA-ng |
|---|---|---|---|---|
| Threshold to choose | none | yes | yes | yes |
| Reads per second, one thread | 19,000 | 39,000 | 258 | 44 |
| Memory | 23 MB | 47 MB | 1.3 GB | 0.3 GB |
| Reads of other genera accepted, reference of a clade | 2% (0 to 7%) | 26% (10 to 43%) | 0% (0 to 3%) | 0.4% (0 to 3%) |
| Reads of a sister group at 85% to 90% ANI accepted | 19% (15 to 29%) | 33% (13 to 63%) | 11% (3 to 20%) | 10% (8 to 21%) |
| Reads of other genera accepted, reference of the whole genus | 4% (0 to 13%) | 40% (10 to 48%) | 1% (0 to 9%) | 2% (0 to 9%) |
| False annotations, in-lineage reads | 16% (6 to 27%) | 32% (9 to 36%) | 0.1% (0.1 to 0.8%), with LCA | 39% (13 to 51%) |
| Exact clade, in-lineage reads | 12% (8 to 19%) | 21% (17 to 32%) | 4% (2 to 11%), with LCA | 20% (15 to 32%) |

The in-lineage reads are those of a clade taken out of the reference;
classeq2 refuses 38% of them (23% to 53%), and the other tools are cut on
their own score where they refuse as many.

**Use classeq2** to classify many short reads of a protein-coding marker
when the reference does not cover everything in the sample: it is fast, it
needs no alignment of the reads, and it refuses sequences from outside the
reference without a threshold to set, which EPIK, the other fast
alignment-free placer, cannot do.

**What the refusal costs.** A read of a strain the reference does not hold
is usually placed (6% to 28% refused); a read of a species it does not hold
is refused about half of the time (48% to 69%). When the two sides of the
root share little with each other (a whole genus such as *Xanthomonas* or
*Streptomyces*, with clades under 80% ANI apart), the query must share with
the side it enters as much as the leaves of that side share with each other
(the fourth reason above); most full-length genes of a new species are then
refused. BLAST or a placer filtered on pendant length still refuse reads of
other genera a little better on such references. To accept more, set `-m`
by hand, or build one reference per clade.

**Use another tool when:**

- the placements must be as deep as possible and the sample is known to be
  in lineage (EPIK, or EPA-ng and pplacer);
- a false annotation costs more than time (BLAST with a lowest common
  ancestor);
- the queries are full-length genes: EPA-ng or pplacer, filtered on pendant
  length, are as safe and reach the exact clade 2.5 times more often;
- the marker is conserved over a deep, sparsely sampled tree, such as 18S
  across a kingdom.

`cls assess` ([2.6](/docs/book/02-build-db.md#26-check-a-reference-before-building))
tells which case a reference falls in before any query is placed. It
measures in-lineage queries only: it does not tell how the reference will
treat outsiders.

These numbers come from five bacterial groups and two other markers. They
depend on the reference: check yours with `cls assess`, and with sequences
you know to be outside it.

## 3.7 Naming the clades: annotations (`-a`)

A placement gives a clade by its number. With an annotation file, `cls place`
also reports what that clade is: its name, its rank, its taxon identifier.

```bash
cls place \
    -d bsub-gyrb.cls \
    -a annotations.yaml \
    -e \
    -o placed_sequences \
    sequences.fasta
```

The file is a YAML list. Each item names a clade of the database tree by its
number and gives it tags:

```yaml
- clade: 0
  meta:
  - !Taxid 653685
  - !SciName Bacillus subtilis group
  - !Rank species group
  - !Gene gyrB
  - !InferenceMethod Maximum likelihood
  - !Note A free text about the reference.

- clade: 70
  meta:
  - !Taxid 1938374
  - !SciName Bacillus amyloliquefaciens group
  - !Rank species group

- clade: 7
  meta:
  - !Taxid 1423
  - !SciName Bacillus subtilis
  - !Rank species
```

| Tag | Value |
|---|---|
| `Taxid` | A whole number, for example an NCBI taxon identifier |
| `SciName` | The scientific name |
| `Rank` | The rank, as free text (`species`, `species group`, `genus`) |
| `Gene` | The gene of the reference |
| `InferenceMethod` | How the tree was inferred |
| `Note` | Free text |

Every tag is optional, and a clade without an item has no annotation.

**Finding the clade numbers.** The numbers are those of the database, not of
the Newick file. Write the tree of the database out and read them there; each
clade has its `id`, its `parent` and, for a leaf, its `name`:

```bash
cls convert database -d bsub-gyrb.cls --only-tree -f json -o tree
```

Branches under `--min-branch-support` are removed when the database is built,
so annotate the tree of the database you will place on. A database built again
with another support, or from another tree, numbers its clades anew.

**What is reported.** A placed query carries the annotations of every
annotated clade between its own clade and the root, sorted by clade number.
Clades are numbered from the root down, so the last item is the deepest name:

```yaml
---
query: read_4
code: "MaxResolutionReached: Evidence below threshold"
annotations:
- clade: 0
  meta:
  - Taxid: 653685
  - SciName: Bacillus subtilis group
  - Rank: species group
  - Gene: gyrB
- clade: 70
  meta:
  - Taxid: 1938374
  - SciName: Bacillus amyloliquefaciens group
  - Rank: species group
placement:
  status: MaxResolutionReached
  clade: 77
  message: Evidence below threshold
scores:
  coverage: 1.0
  evidence: 190
  rival: 0
```

Here the query stopped at clade 77, an unnamed clade inside the *Bacillus
amyloliquefaciens* group, so the group is the deepest name it can be given. A
query that stops at a species clade, or below one, carries the species as its
last item. A refused query has no placement and no annotations.

**Which clades to annotate.** Annotate a species on the clade that holds all
its sequences, and only when that clade holds no sequence of another species.
A gene tree does not resolve every species; a species whose sequences are
mixed with those of another is better left without a clade of its own, and its
queries are then reported at the group above. Annotating the root with the
group and the gene makes every placed query say which reference it was placed
on.

## 3.8 Ready-made databases (`cls db`)

Some databases are published with classeq2, each with the names of its species
annotated on the tree ([3.7](#37-naming-the-clades-annotations--a)). `cls db`
lists them and downloads one.

```bash
cls db list
```

```
name       version  latest  date        cls     size    title
bsub-gyrb  1        yes     2026-11-17  0.12.0  3.0 MB  Bacillus subtilis group, gyrB
xan-gyrb   1        yes     2026-11-17  0.12.0  5.0 MB  Xanthomonas, gyrB
bur-reca   1        yes     2026-11-17  0.12.0  1.5 MB  Burkholderia, recA
```

The output is tab-separated, one line per version. `cls` is the version of
classeq2 that built the database, and `size` is that of all its files.

```bash
cls db fetch bsub-gyrb
cls place -d bsub-gyrb/bsub-gyrb.cls -a bsub-gyrb/annotations.yaml -e \
    -o placed_sequences sequences.fasta
```

`fetch` writes into a directory named after the database, inside the current
directory or the one given with `-o`. It downloads the database, its
annotation file and its README.

| Option | Effect |
|---|---|
| `--version N` | Download version `N` and not the latest |
| `--all` | Also download the alignment and the tree the database was built from, the report of `cls assess`, and an example sample with its expected report |
| `-o DIR` | Write into `DIR/<name>/` |
| `--force` | Overwrite files that already exist |
| `--index LOCATION` | Read another index, at an `http(s)` address or a local path |

Every file is checked against the SHA-256 recorded in the index. If a file
fails the check, nothing is written. If the database was built by a classeq2
newer than the one running, a warning says so: update classeq2, or build the
database again from its alignment and tree (`--all`).

**Versions.** Each database is versioned by itself, with whole numbers. A
database gets a new version only when one of its files changed, so a new
release of classeq2 does not by itself make new versions of the databases.
Give `--version` in a script or a pipeline whose results must stay the same.

**Where the files are.** Each version is a release of the classeq2 repository
on GitHub, tagged `db-<name>-v<N>`, with the files as its assets; they can be
downloaded from the release page without `cls`. The list read by `cls db` is
`index.json`, in the release tagged `databases`.

**A mirror.** A machine without access to GitHub can use a copy. Keep the
layout of the releases, `<root>/databases/index.json` and
`<root>/db-<name>-v<N>/<file>`, and give the index:

```bash
cls db list --index /data/classeq/databases/index.json
cls db fetch bsub-gyrb --index /data/classeq/databases/index.json
```

---

[◀️ Prev | Build Classeq Database](/docs/book/02-build-db.md)

[▶️ Next | Configure API Server](/docs/book/04-configure-api-server.md)
