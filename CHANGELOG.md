## v0.8.1 (2024-08-03)

### Fix

- fix the new minimizer building
- include a deletion option to log file if exits
- remove rest-max and rest-avg struct elements not used

## v0.8.0 (2024-08-01)

### Feat

- implements the annotation parsing during sequences placement via cli port

### Refactor

- fix the access control of shared use cases of core module and output-format dto from cli port
- rename the map-kmers-to-tree to build-database to preciselly indicate their goal
- rename place-sequences use-case dtos file to indicate a internal module
- convert the map-kmers-to-tree to module instead of a file

## v0.7.3 (2024-07-31)

### Fix

- fix the security issue to upgrade bytes subtle zerovec and zerovec-derive

## v0.7.2 (2024-07-29)

### Fix

- replace yanked versions of bytes zerovec zerovec-derive and subtle

## v0.7.1 (2024-07-29)

### Perf

- do implement a profilling tool to generate pprof files

## v0.7.0 (2024-07-28)

### Feat

- improve the telemetry from the watcher to allow delivery analysis logs to users

## v0.6.0 (2024-07-25)

### Feat

- given the database format of classeq the describe-db function was implemented to allow users do inspect database without convert it to yaml format
- optimize the database persistence and loading and implements the telemetry to the watcher module
- do implement the zstandard compression to classeq database

### Refactor

- move the crate import to the top in describe-db command
- remove unused telemetry import to the execution message struct

## v0.5.1 (2024-07-24)

### Refactor

- group functional elements of the introspection loop to turn easy the algorithm comprehension

## v0.5.0 (2024-07-23)

### Feat

- improve the placement process to remove shared kmers between one-vs-rest pairs

### Fix

- fix the log file extension setting during runtime
- replace the default minimizer size from 2 to 4 given the optimization results
- fix the log format format specification
- review telemetry steps and turn one-vs-rest intersection evaluation as an optional element of the cli
- fix the placement process bug introduced together the performance improvement of commit a19004d03af025f4ef9524d5c483504e04244e1f
- set the minimizer default size on the cli port to two instead of four
- update the root tree script to allow set the root type during the root tree script runtime

### Perf

- re-evaluate the indexed kmers search engines to speed-up the placement process
- upgrade kmers mapping process to speedup the indexation process

## v0.4.4 (2024-07-14)

### Perf

- upgrade the main sequence-placement use-case to speedup the placement process

## v0.4.3 (2024-07-14)

## v0.4.2 (2024-07-09)

### Fix

- add a random sleeper to allow multiple workers to run in parallel

### Perf

- review the cpu and memory limits of watcher and api services in docker-compose

## v0.4.1 (2024-07-07)

## v0.4.0 (2024-07-07)

### Feat

- wip - update the implementation of the place-sequences method
- reconfigure the shared information between api and watcher
- wip - do implement the dir watcher and job scheduler

### Fix

- fix error handling in place-sequences use-cases to avoid panic

### Refactor

- refactor the api to allow get files by inode

## v0.3.0 (2024-06-23)

### Feat

- wip - do implement the directory watcher with a infinite scheduler
- move shared elements between ports to a dedicated lib
- wip - do implement the analysis configuration initializer
- implements the basic workdir implementation

### Refactor

- rename sched to watcher to mirror the exact function of the port

## v0.2.7 (2024-06-16)

### Perf

- replace the hash algorithm by murmur3 and implements the minimizer mapping for group kmers

## v0.2.6 (2024-06-13)

### Refactor

- remove unused structural methods and tests of kmer-map object

## v0.2.5 (2024-06-12)

### Fix

- fix tracing appender version

## v0.2.4 (2024-06-12)

### Fix

- fix the sequence parsing and log tracing

### Perf

- upgrade kmersmap to map kmers using hash values instead of string

## v0.2.3 (2024-06-09)

### Fix

- fix the kmers generation that will not use the reverse complement of sequences

### Refactor

- refactor the sequence management to add parallel processing to key steps of thesequence placement

## v0.2.2 (2024-06-04)

### Fix

- fix branch tree and clade data loading without branch support

## v0.2.1 (2024-06-04)

### Refactor

- rename cli port from cls2 to cls

## v0.2.0 (2024-06-04)

### Feat

- wip - implements the multi sequence placement method
- implements the build database feature as a cli port
- wip - implements the basis for the sequence placement based on the classeq-py rules
- turn tree uuid fixed with uuid3 to allow identification of the same tree index
- implements a use-case to map kmers to the tree nodes

### Fix

- include the kmer-size into the kmers-map object to persist it into the indexing object
- turn kmers map sorted on serialize to allow better evaluation of indices

### Refactor

- rename io port to convert mirroring the command used to execute
- refactor stuff logif of the place-sequence use-case
- refactor annotation to allow more informative tags
- rename node-type to reduce number of characters used for storage index
- move annotations to the tree struct and turn unused fields unserialized

### Perf

- improve performance on control the thread pool used during predictions

## v0.1.0 (2024-05-30)

### Feat

- implements a port to convert trees to yaml and json formats
- implements the tree parsing from newick files
- initial commit
