FROM ubuntu:26.04
RUN apt-get update && apt-get install -y --no-install-recommends linux-tools-7.0.0-42-generic linux-tools-common linux-perf binutils time hyperfine python3 python3-yaml ca-certificates && rm -rf /var/lib/apt/lists/*
ENV PATH=/usr/lib/linux-tools/7.0.0-42-generic:$PATH
