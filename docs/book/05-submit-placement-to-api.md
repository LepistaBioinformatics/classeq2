# 5. Submit Placement to API 🔗

[🏠 Home](/README.md)

[📋 Summary](/docs/README.md)

---

With the server running ([4](/docs/book/04-configure-api-server.md)), an
analysis takes five requests: choose a database, open a work directory, upload
the sequences, ask for the analysis, and collect the result. The examples use
`curl` against a server at `http://localhost:8080`.

## 5.1 The requests

| Request | What it does | Answer |
|---|---|---|
| `GET /models` | Lists the databases | `200` with the list |
| `POST /wd` | Opens a work directory | `201` with its identifier |
| `POST /wd/{id}` | Uploads a file (multipart form, field `file`) | `201`; `409` if the file exists |
| `POST /wd/{id}/config` | Asks for the analysis (JSON body) | `201`; `400` if a field is missing |
| `GET /wd/{id}` | Lists the files of the work directory | `200` with the list; `204` while it is empty |
| `GET /wd/{id}/{file}` | Sends one file, by the number that the list gives it | `200` with the file; `204` if there is no such file |

Every request on a work directory answers `400` when the identifier is not
valid and `404` when the directory does not exist.

## 5.2 Choose a database

```bash
curl http://localhost:8080/models
```

```json
[{"id":"85db5358-0ae8-3714-921d-dba6c2d7bd59","name":"Bacillus subtilis group","gene":"gyrB"}]
```

Keep the `id` of the database to use.

## 5.3 Open a work directory

```bash
curl -X POST http://localhost:8080/wd
```

```json
{"workDirId":"01a1e7dd-a374-7320-9df6-e30c77a35ab5"}
```

The identifier is the only key to the analysis. Keep it; the server has no
list of the work directories of a user.

## 5.4 Upload the sequences

The query is a FASTA file, sent as the field `file` of a multipart form:

```bash
WD=01a1e7dd-a374-7320-9df6-e30c77a35ab5
curl -X POST -F "file=@reads.fasta" http://localhost:8080/wd/$WD
```

```json
{"status":201,"msg":"File saved successfully"}
```

A second upload under the same name is refused with `409`. Add `?force=true`
to the address to replace the file.

The analysis names its query file by a number, which the list of the work
directory gives:

```bash
curl http://localhost:8080/wd/$WD
```

```json
[{"id":23013,"name":"input/reads.fasta","createdAt":"2026-11-17T04:57:04.659082677+00:00","updatedAt":"2026-11-17T04:57:04.659082677+00:00","accessedAt":"2026-11-17T04:57:04.659082677+00:00","size":4440}]
```

Here the file is number `23013`.

## 5.5 Ask for the analysis

```bash
curl -X POST http://localhost:8080/wd/$WD/config \
    -H 'Content-Type: application/json' \
    -d '{
      "name": "Lot 42",
      "description": "gyrB reads of lot 42",
      "email": "analyst@example.org",
      "queryFileId": 23013,
      "modelId": "85db5358-0ae8-3714-921d-dba6c2d7bd59",
      "outputFormat": "yaml",
      "workDir": "01a1e7dd-a374-7320-9df6-e30c77a35ab5"
    }'
```

```json
{"status":201,"msg":"Analysis configuration saved successfully"}
```

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | A name for the analysis |
| `description` | no | Free text |
| `email` | yes | Contact of who asked for it |
| `queryFileId` | yes | Number of the uploaded query file |
| `modelId` | yes | `id` of the database |
| `outputFormat` | yes | `yaml` or `jsonl` |
| `workDir` | yes | Identifier of the work directory |
| `removeIntersection` | no | `true` scores each clade only by the k-mers that none of its siblings share, as `--remove-intersection` of `cls place` |
| `overwrite` | no | Recorded with the analysis |

`name`, `description`, `email`, `workDir` and `overwrite` are kept in the
record of the analysis and do not change the placement. No message is sent to
the address.

The request only records the analysis. The watcher finds it at its next look
into the data directory and places the sequences.

## 5.6 Wait and collect the result

List the work directory until `success.yaml` or `error.yaml` is there:

```bash
curl http://localhost:8080/wd/$WD
```

| File | Meaning |
|---|---|
| `config.yaml` | The analysis, as recorded |
| `running.yaml` | The placement has started |
| `output/result.yaml` | The placements (`result.jsonl` with `"outputFormat": "jsonl"`) |
| `logging.jsonl` | The log of the placement |
| `success.yaml` | The analysis ended; the result is complete |
| `error.yaml` | The analysis failed; the file gives the reason |

Then ask for the result by its number in the list:

```bash
curl -o result.yaml http://localhost:8080/wd/$WD/23091
```

The result has the format of `cls place`
([3.2](/docs/book/03-place-sequence-cli.md#32-output-format)), one record
for each query, with the annotations of its clade when the database has an
annotation file:

```yaml
---
query: read_1
code: IdentityFound
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
  status: IdentityFound
  clade:
    id: 81
    parent: 80
    kind: LEAF
    name: GCF_000015785.2
    length: 0.0032181877
  one: 232
  rest: 162
```

## 5.7 When it fails

`error.yaml` holds one message. The common ones:

| Message | Cause |
|---|---|
| `Model with ID … not found` | `modelId` is not an `id` of the configuration file of the server |
| `Query file with inode … not found` | `queryFileId` is not the number of a file in `input/` |
| `Failed to parse the configuration file` | The recorded analysis could not be read |
| `Failed to load the model with ID …` | The database file is missing or was built by a version that this one cannot read |

A failed analysis is not tried again. Open a new work directory, upload the
file and ask again.

## 5.8 Things to know

- The server places without the evidence rule of `cls place -e`
  ([3.3](/docs/book/03-place-sequence-cli.md#33-the-evidence-rule)), so its
  records have no `scores` field and can be `Inconclusive` where two clades
  tie. For the evidence rule, use the command line.
- Whoever has the identifier of a work directory can read its files. Do not
  publish it.
- The files stay on the server until its administrator removes them.

---

[◀️ Prev | Configure API Server](/docs/book/04-configure-api-server.md)

[▶️ Next | Benchmarking Classeq](/docs/book/06-telemetry-and-benchmark.md)
