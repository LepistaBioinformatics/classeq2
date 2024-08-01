# 6. Telemetry and Benchmark

[🏠 Home](/README.md)

[📋 Summary](/docs/README.md)

---

Classeq implements the opentelemetry logging and tracing, allowing you to use
any opentelemetry compatible backend to collect and analyze the logs and traces
in ANCII and JSONL format. The former is useful for human readability, while the
latter is useful for machine readability.

The telemetry codes are specific along the Classeq layers, and users can see the
telemetry codes description in the following files:

- [Core layer (Domain layer)](/core/src/domain/dtos/telemetry_code.rs)
- [CLI layer (CLI port layer)](/ports/cli/src/dtos/telemetry_code.rs)
- [Watcher codes (Watcher port layer)](/ports/watcher/src/dtos/telemetry_code.rs)

As example, if you want to see the telemetry codes for the placement at the top
level executing the CLI, you should set the INFO as the minimum log level and
set the log format to JSONL. Them, you can execute the placement command:

```bash
$ cat sequences.fasta | cls \ 
    --log-level info \ 
    --log-format jsonl \ 
    --log-file placed_sequences.jsonl \ 
    place \ 
    -d cls-database-name \ 
    -o placed_sequences.yaml
```

The command should generate a JSONL file with the telemetry codes for the
placement. Users can use the JSONL file to analyze the logs and traces. A simple
tool to analyze the JSONL file is the `jq` command. For example, to see the
telemetry codes for the placement, you can use the following command:

```bash
cat placed_sequences.jsonl | jq -C '.'
{
  "timestamp": "2024-07-28T03:00:48.854951Z",
  "level": "INFO",
  "fields": {
    "message": "Start multiple sequences placement from CLI",
    "code": "CLIPLACE0001"
  },
  "span": {
    "run_id": "6799f19f66dc43fe9e6774969430ae0a",
    "name": "PlacingSequenceCMD"
  },
  "spans": [
    {
      "run_id": "6799f19f66dc43fe9e6774969430ae0a",
      "name": "PlacingSequenceCMD"
    }
  ],
  "threadId": "ThreadId(1)"
}
{
  "timestamp": "2024-07-28T03:00:49.519852Z",
  "level": "INFO",
  "fields": {
    "message": "Execution times",
    "code": "CLIPLACE0002",
    "totalSeconds": 0.6641790866851807,
    "averageSeconds": 0.037732433527708054,
    "maxSeconds": 0.037732433527708054,
    "minSeconds": 0.037732433527708054
  },
  "span": {
    "run_id": "6799f19f66dc43fe9e6774969430ae0a",
    "name": "PlacingSequenceCMD"
  },
  "spans": [
    {
      "run_id": "6799f19f66dc43fe9e6774969430ae0a",
      "name": "PlacingSequenceCMD"
    }
  ],
  "threadId": "ThreadId(1)"
}
```

Note the `code` field in the JSONL file. The `code` field includes the telemetry
codes for the placement start (CLIPLACE0001) and end (CLIPLACE0001). The
difference between start and end can be used to calculate the execution time for
the placement.

In this way you can increase the logging level to DEBUG or TRACE to see more
details about the placement execution. The below table indicates the telemetry
code pairs used for a detailed benchmark of the placement process using the
CLI:

| Log Level | Code pair | Goal |
|-----------|-----------|------|
| INFO      | CLIPLACE0001, CLIPLACE0002 | Measure the total execution time for the placement |
| DEBUG     | UCPLACE0001, UCPLACE0002 | Measure the execution time ignoring the database deserialization time |
| DEBUG     | UCPLACE0003, UCPLACE0004 | Measure the total execution time of a single query sequence |
| TRACE     | UCPLACE0004, UCPLACE0005 | Measure the time to generate the query kmers |
| TRACE     | UCPLACE0005, UCPLACE0006 | Measure the time to build the query kmers map |
| TRACE     | UCPLACE0006, UCPLACE0007 | Measure the time to match the query kmers with the database kmers |
| TRACE     | UCPLACE0010 | Emitted at all introspection loop iterations |

For a detailed information about the telemetry codes, you can see the
aforementioned telemetry codes files.

To filter the telemetry codes in the JSONL file, you can use the `jq` command.
For example, to filter the telemetry codes for the placement execution time, you
can use the following command:

```bash
cat placed_sequences.jsonl | jq -C '. | select(.fields.code == "CLIPLACE0002")'
{
  "timestamp": "2024-07-28T03:00:49.519852Z",
  "level": "INFO",
  "fields": {
    "message": "Execution times",
    "code": "CLIPLACE0002",
    "totalSeconds": 0.6641790866851807,
    "averageSeconds": 0.037732433527708054,
    "maxSeconds": 0.037732433527708054,
    "minSeconds": 0.037732433527708054
  },
  "span": {
    "run_id": "6799f19f66dc43fe9e6774969430ae0a",
    "name": "PlacingSequenceCMD"
  },
  "spans": [
    {
      "run_id": "6799f19f66dc43fe9e6774969430ae0a",
      "name": "PlacingSequenceCMD"
    }
  ],
  "threadId": "ThreadId(1)"
}
```

The above result include some statistics about the placement execution time. The
`totalSeconds` field indicates the total execution time for the placement, while
the `averageSeconds`, `maxSeconds`, and `minSeconds` fields indicate the average,
maximum, and minimum execution time for the placement, respectively.

Additionally, to filter the CLIPLACE0001 > CLIPLACE0002 timestamps pair you can
use the following command:

```bash
cat placed_sequences.jsonl | jq -C '. | select(.fields.code == "CLIPLACE0001" or .fields.code == "CLIPLACE0002") | .timestamp, .fields.message' | xargs -n2

2024-07-28T03:14:07.366016Z Start multiple sequences placement from CLI
2024-07-28T03:14:08.040341Z Execution times
```

In the future, we will provide a tool to analyze the telemetry codes in the
JSONL file, allowing users to see the telemetry codes in a more user-friendly
way. But for now, you can use the `jq` command to analyze the telemetry codes in
the JSONL file.

---

[◀️ Prev | Place Sequence Using API](/docs/book/05-submit-placement-to-api.md)
