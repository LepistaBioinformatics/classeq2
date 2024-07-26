# 2. Build Database

[🏠 Home](/README.md)

[◀️ Docs](/docs/README.md)

---

Naturally as a phylogenetic placer, Classeq requires a reference tree and a
reference multi sequence FASTA file to place sequences. The reference tree
should be rooted and in Newick format. The reference multi sequence FASTA file
should contain all sequences that are used to build the reference tree.

## 2.1 Root the Reference Tree

Case you don't have rooted reference tree, you should root it before build the
database. To root the reference tree, you can use the Python script
`root_tree.py` available in the `scripts` directory. The script requires the
reference tree in Newick format as input. The script uses the midpoint rooting
method to root the tree. The output is the rooted tree in Newick format. The
script can be used as follows:

```bash
python \ 
    scripts/root_tree.py \ 
    reference_tree.nwk \ 
    rooted_reference_tree.nwk
```

If you prefer to use Docker, you can use the following command to root the
reference tree:

```bash
docker run \
    -it --rm \ 
    -u $( id -u ${USER} ):$( id -g ${USER} ) \ 
    -v ${PWD}:/data \ 
    -w /data \ 
    --entrypoint root_tree.py \ 
    sgelias/classeq-cli \ 
    reference_tree.nwk \ 
    rooted_reference_tree.nwk
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
`classeq-database.cls`, and uses the
([Zstandard](https://github.com/facebook/zstd)) format as default.

### Additional options

**K-mer and Minimizer sizes**: As default classeq build the database using kmers
of size 35 and a minimizer size of 4. You can change these values using the `-k`
and `-m` options, respectively.

**Tree sanitization**: During the database building, the tree is sanitized to
remove branches with low phylogenetic signal. The `-s` option allows you to
change the threshold used to remove branches. The default value is 70.

## 2.3 Database conversion and description

The database is stored in a binary file with the `.cls` extension. The database
can be converted to a YAML, a human-readable format using the CLI command `cls
convert database`. The command requires the database file as input and outputs
the database in a desired format. The command can be used as follows:

```bash
cls convert database \ 
    cls-database-name.cls \ 
    -o cls-database-name.yml
```

You can convert from YAML to binary using the command `cls convert database` as
well. The command requires the database file in YAML format as input and outputs
the database in binary format.

In addition, you can describe the database using the CLI command `cls
describe-db`. The command requires the database file as input and outputs the
database description. The output format should be JSON, YAML, or TSV. The
default output format is TSV and the option `-f` allows you them to change. The
command can be used as follows:

```bash
cls describe-db \ 
    cls-database-name.cls
```

---

[◀️ Prev | Installation](/docs/book/01-installation.md)

[▶️ Next | Place Sequence With CLI](/docs/book/03-place-sequence-cli.md)
