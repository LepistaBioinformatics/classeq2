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
