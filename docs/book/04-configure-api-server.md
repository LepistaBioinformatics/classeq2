# 4. Configure API Server 🔧

[🏠 Home](/README.md)

[📋 Summary](/docs/README.md)

---

The server lets people place sequences without installing classeq2: they send
a FASTA file over HTTP and collect the result later. It is made of two
programs that share one configuration file and one data directory.

| Program | Crate | What it does |
|---|---|---|
| `cls-api` | `classeq-api` | The HTTP server. It creates a work directory for each analysis, receives the query file, records which database to use, and serves the files of the work directory. It places nothing itself. |
| `cls-watcher` | `classeq-watcher` | A background service. It looks into the data directory at a fixed interval, finds the analyses that are waiting, places their sequences and writes the result next to them. |

Because the two only meet in the data directory, they can run on one machine
or on several that mount the same directory.

## 4.1 Install

```bash
cargo install classeq-api classeq-watcher
```

The repository also has `Dockerfile.api`, `Dockerfile.watcher` and
`docker-compose.prod.yaml`, which starts both with the configuration file and
the directories of data and of databases mounted as volumes. Edit the volume
paths of the compose file before using it.

## 4.2 The configuration file

One YAML file serves both programs. `settings/config.prod.yaml` is a template.

```yaml
fs:
  serveDirectory: /var/classeq/data
  publicDirectory: public
  inputDirectory: input
  outputDirectory: output
  configFileName: config.yaml
  resultsFileName: result.yaml
  successFileName: success.yaml
  runningFileName: running.yaml
  loggingFileName: logging.jsonl
  errorFileName: error.yaml

server:
  address: "0.0.0.0"
  port: 8080
  workers: 8

models:
- id: 85db5358-0ae8-3714-921d-dba6c2d7bd59
  name: Bacillus subtilis group
  gene: gyrB
  modelPath: /etc/classeq/models/bsub-gyrb/bsub-gyrb.cls
  annotationsPath: /etc/classeq/models/bsub-gyrb/annotations.yaml

watcher:
  workerName: cls-watcher
  maxThreads: 12
  workers: 1
  interval: 15
  retries: 3
```

**`fs`, the data directory** (read by both programs; every key is required)

| Key | Meaning |
|---|---|
| `serveDirectory` | Root of the data. Both programs must see the same directory |
| `publicDirectory` | Directory inside the root where the work directories are created |
| `inputDirectory` | Directory inside a work directory for the uploaded files |
| `outputDirectory` | Directory inside a work directory for the result |
| `configFileName` | File that records the analysis asked for |
| `resultsFileName` | Name of the result file. Its extension follows the output format of the analysis |
| `runningFileName`, `successFileName`, `errorFileName` | Files that mark the state of an analysis ([4.5](#45-what-the-watcher-does)) |
| `loggingFileName` | Log of the placement, one JSON record per line |

**`server`** (read by `cls-api`)

| Key | Meaning |
|---|---|
| `address` | Address to listen on. `0.0.0.0` accepts connections from other machines |
| `port` | Port |
| `workers` | Number of HTTP workers. Optional, 1 by default |

**`models`, the databases offered** (read by both programs)

| Key | Meaning |
|---|---|
| `id` | Identifier that clients send to choose the database. Use the `ID` that `cls describe-db` prints for it |
| `name` | Name shown to clients |
| `gene` | Gene or marker of the reference. Optional |
| `modelPath` | Path of the `.cls` file, as the watcher sees it |
| `annotationsPath` | Path of the annotation file ([3.7](/docs/book/03-place-sequence-cli.md#37-naming-the-clades-annotations--a)). Optional |

The paths are never sent to clients. A database published with classeq2 can be
used as it is:

```bash
cls db fetch bsub-gyrb -o /etc/classeq/models
cls describe-db -d /etc/classeq/models/bsub-gyrb/bsub-gyrb.cls | grep '^ID'
```

**`watcher`** (read by `cls-watcher`; every key is required)

| Key | Meaning |
|---|---|
| `workerName` | Name of the worker in the logs |
| `interval` | Seconds between two looks into the data directory |
| `maxThreads` | Threads used to place the sequences of an analysis |
| `workers` | Number of workers that look into the directory. Keep it at 1 for each running watcher |
| `retries` | Times a failed look is tried again |

## 4.3 Start the two programs

`cls-api` takes the location of the configuration file from the environment
variable `SETTINGS_PATH`. It writes its log to the standard output as JSON
records.

```bash
SETTINGS_PATH=/etc/classeq/config.yaml cls-api
```

`cls-watcher` takes it as an argument:

```bash
cls-watcher --log-level info watch --config-file /etc/classeq/config.yaml
```

`--log-format jsonl` writes the watcher's log as JSON records, and `--log-file`
writes it to a file that is rotated every day.

To check that the server is up, any address it does not know answers
`healthy`, and `/models` lists the databases of the configuration file:

```bash
curl http://localhost:8080/
# healthy

curl http://localhost:8080/models
# [{"id":"85db5358-0ae8-3714-921d-dba6c2d7bd59","name":"Bacillus subtilis group","gene":"gyrB"}]
```

## 4.4 What is on disk

Each analysis is a directory named by an identifier that the server makes up:

```
/var/classeq/data/public/
└── 01a1e7dd-a374-7320-9df6-e30c77a35ab5/
    ├── input/
    │   └── reads.fasta          the uploaded query file
    ├── config.yaml              the analysis asked for
    ├── running.yaml             written when the placement starts
    ├── output/
    │   └── result.yaml          the placements
    ├── logging.jsonl            the log of the placement
    └── success.yaml             or error.yaml, with the reason
```

Nothing is removed by the server or by the watcher. Old work directories are
to be cleaned by the administrator.

## 4.5 What the watcher does

Every `interval` seconds the watcher lists the work directories. One is
waiting when it has the configuration file of an analysis and none of the
three state files. For each waiting directory the watcher:

1. reads the analysis and finds the database by its `id`;
2. loads the database and its annotations;
3. writes the running file;
4. places the sequences and writes the result;
5. writes the log, and then the success file or the error file.

An analysis is taken once. A directory with a success or an error file is not
looked at again, so a new analysis needs a new work directory.

## 4.6 Before opening it to others

- **There is no authentication.** Whoever reaches the server can create work
  directories, and whoever knows the identifier of one can read its files. The
  identifiers are long and random, which is not access control. Run the server
  inside a trusted network, or behind a proxy that authenticates.
- **Use TLS in the proxy.** The server speaks plain HTTP.
- **The server places with the default rule of `cls place`, without the
  evidence rule (`-e`)** ([3.3](/docs/book/03-place-sequence-cli.md#33-the-evidence-rule)),
  and with the match coverage that the database was calibrated with. An
  analysis cannot ask for other settings; for those, use the command line.
- **Size the watcher for the load.** Analyses are placed one after the other
  by each watcher, with `maxThreads` threads each.

---

[◀️ Prev | Place Sequence With CLI](/docs/book/03-place-sequence-cli.md)

[▶️ Next | Place Sequence Using API](/docs/book/05-submit-placement-to-api.md)
