# 2. Build Database 🏗️

[🏠 Home](/README.md)

[📋 Summary](/docs/README.md)

---

Naturally as a phylogenetic placer, Classeq requires a reference tree and a
reference multi sequence FASTA file to place sequences. The reference tree
should be rooted and in Newick format. The reference multi sequence FASTA file
should contain all sequences that are used to build the reference tree.

## 2.1 Root the Reference Tree

Classeq places sequences by descending the reference tree from its root, so
the tree must be rooted. Most tree inference programs (PhyML, RAxML, IQ-TREE,
FastTree) write unrooted trees, with three branches at the top level. `cls
build-db` rejects such a tree with an error that points to the command below.

Root the tree with `cls root-tree`. It keeps leaf names, branch lengths and
branch supports, and moves each support with its branch when the root changes.

```bash
# Midpoint rooting (default): the root is placed at the middle of the longest
# leaf-to-leaf path.
cls root-tree reference_tree.nwk -o rooted_reference_tree.nwk

# Outgroup rooting: the root is placed on the branch that separates these
# leaves from all the others. The outgroup must form a clade.
cls root-tree reference_tree.nwk -o rooted_reference_tree.nwk \
    --outgroup Outgroup_1,Outgroup_2
```

Without `-o` the rooted tree is written to stdout, and the input can be read
from stdin with `-`. Use `--keep-if-rooted` to leave trees that are already
rooted unchanged.

Alternatively, `cls build-db` can root the tree in memory with
`--root midpoint` or `--root outgroup --outgroup <LEAVES>`. A tree that is
already rooted is then used as is.

If you prefer to use Docker:

```bash
docker run \
    -it --rm \
    -u $( id -u ${USER} ):$( id -g ${USER} ) \
    -v ${PWD}:/data \
    -w /data \
    sgelias/classeq-cli \
    root-tree reference_tree.nwk -o rooted_reference_tree.nwk
```

## 2.2 Build the Database

After rooting the reference tree, you can build the database using the CLI
command `cls build-db`. The command requires the rooted reference tree and the
reference multi sequence FASTA file as input. The command uses the reference
tree and the reference sequences to build the database. The command can be used
as follows:

```bash
cls build-db \ 
    rooted_reference_tree.nwk \ 
    reference_sequences.fasta \ 
    -o cls-database-name
```

The first two arguments are positional and required. The first argument is the
rooted reference tree, and the second argument is the reference multi sequence
FASTA file. The `-o` option is optional and defines file name of the database
output. Case the option is not provided, the output file name is
`classeq-database.cls`. The database is written in the binary format described
in [2.5](#25-binary-database-layout).

### Additional options

**K-mer and Minimizer sizes**: As default classeq build the database using kmers
of size 35 and a minimizer size of 4. You can change these values using the `-k`
and `-m` options, respectively.

**Tree sanitization**: During the database building, the tree is sanitized to
remove branches with low phylogenetic signal. The `-s` option allows you to
change the threshold used to remove branches. The default value is 70.

**Database format**: `--format binary` (the default) writes the binary
database. `--format legacy` writes the
[Zstandard](https://github.com/facebook/zstd)-compressed YAML database of
earlier versions. Use it only when an older `cls` must read the database:
older versions cannot read the binary format, while the current version reads
both.

## 2.3 Database conversion and description

The database is stored in a binary file with the `.cls` extension. The database
can be converted to YAML, a human-readable format, using the CLI command `cls
convert database`. The command reads a database in any format and writes it in
the format given by `-f`: `yaml` (the default), `json`, `zstd` (the legacy
format) or `binary`. The command can be used as follows:

```bash
cls convert database \ 
    -d cls-database-name.cls \ 
    -f yaml \ 
    -o cls-database-name
```

Use `-f binary` to convert a legacy database to the binary format. Converting
a legacy database and building the same tree and sequences with `build-db`
give byte-identical files.

In addition, you can describe the database using the CLI command `cls
describe-db`. The command requires the database file as input and outputs the
database description. The output format should be JSON, YAML, or TSV. The
default output format is TSV and the option `-f` allows you them to change. The
command can be used as follows:

```bash
cls describe-db \ 
    cls-database-name.cls
```

The database description contains the following information:

```yaml
# The unique identifier of the database
ID: ce47d8bc-2885-3d2c-8247-5b8c8b28fefe
# The name of tree file used to build the database
Name: bsub-rootedmid.nwk
# The lowest branch support used to sanitize the tree
MinBranchSupport: '70'
# The k-mer size
KmerSize: '35'
# The total number of k-mers in the database
kmerCount: '101542'
# The minimizer size
MinimizerSize: '4'
# The total number of minimizers in the database
MinimizerCount: '256'
# The number of kmers contained at the largest minimizer
LargestMinimizer: '1082'
# The number of kmers contained at the smallest minimizer
SmallestMinimizer: '86'
# The average number of kmers contained at the minimizers
MinimizerAvgKmers: '396'
```

## 2.4 YAML format

When converted to YAML, the database is a standard YAML file. The legacy
`.cls` format is the same YAML compressed with Zstandard. The YAML file
contains the following information:

```yaml
# The unique identifier of the database
id: ce47d8bc-2885-3d2c-8247-5b8c8b28fefe
# The name of tree file used to build the database
name: bsub-rootedmid.nwk
# The lowest branch support used to sanitize the tree
minBranchSupport: 70.0
# The sanitized tree in a yaml format
root:
  id: 0
  kind: ROOT
  length: 0.0
  children:
  - id: 1
    kind: NODE
    support: 100.0
    length: 0.197182
    children:
    - id: 2
      kind: NODE
      support: 100.0
      length: 0.0442934
      children:
      - id: 3
        name: NZ_AOFM01000009_Bacillus_sonorensis_L12_contig9
        kind: LEAF
        length: 1e-6
      - id: 4
        name: _R_NZ_QRLM01000005_Bacillus_sonorensis_strain_AM1257B
        kind: LEAF
        length: 0.000619744
# The kmers map in a yaml format
kmersMap:
  # The kmer size
  kSize: 35
  # The minimizer size
  mSize: 4
  # The kmers map itself
  map:
    # A minimizer
    10631256518523097406:
      # A hashed kmer
      15277858258350234285:
      # A tree node id
      - 0
      - 65
      - ...
      16738842834674266113:
      - 0
      - 78
      - ...
    10517626403121597142:
      13045666331085307167:
      - 0
      - 310
      - ...
```

The most important parts of the database are the `root` and `kmersMap` fields.
The `root` field contains the sanitized reference tree in a YAML format. The
`kmersMap` field include all mapped minimizers and kmers from the reference
tree, used during predictions.

## 2.5 Binary database layout

The binary database holds the same content as the YAML file, laid out so that
`cls` maps the file into memory instead of parsing it, in the spirit of the
NCBI BLAST and Kraken 2 databases. Opening a database therefore takes
milliseconds whatever its size, and the operating system loads only the
pages that placement reads. **Do not modify a database file while `cls` reads
it.**

All integers are little-endian, and every section starts at a multiple of 8
bytes.

| Section | Content |
|---|---|
| Header (128 bytes) | The magic `CLSQIDX\x02`, the layout version, the widths of the node ids (2 or 4 bytes) and of the posting offsets (4 or 8 bytes), the k-mer and minimizer sizes, the counts, the section offsets and a CRC32 of the tree section. |
| Tree | The tree as JSON, without the k-mers. |
| Buckets | One `(minimizer hash, first k-mer)` pair per minimizer, sorted by hash, plus a final pair whose first k-mer is the k-mer count. |
| K-mers | The k-mer hashes, sorted within each minimizer. |
| Posting offsets | Where the node ids of each k-mer start, plus a final offset equal to the number of node ids. |
| Postings | The tree nodes of each k-mer, sorted. Node ids take 2 bytes when the tree has fewer than 65,536 nodes, and 4 bytes otherwise. |

`cls` checks the header, the section bounds and the tree checksum when it
opens a database, and checks every offset when it reads one. A damaged file
gives an error, never a crash.

---

[◀️ Prev | Installation](/docs/book/01-installation.md)

[▶️ Next | Place Sequence With CLI](/docs/book/03-place-sequence-cli.md)
