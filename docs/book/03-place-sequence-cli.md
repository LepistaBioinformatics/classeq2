# 3. Place Sequences with CLI

[🏠 Home](/README.md)

[◀️ Docs](/docs/README.md)

---

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

---

[◀️ Prev | Build Classeq Database](/docs/book/02-build-db.md)

[▶️ Next | Configure API Server](/docs/book/04-configure-api-server.md)
