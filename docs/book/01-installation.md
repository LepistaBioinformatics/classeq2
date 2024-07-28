# 1. Installation

[🏠 Home](/README.md)

[📋 Summary](/docs/README.md)

---

> **_IMPORTANT:_** The Classeq software was developed and tested using the linux
> environment. We recommend using a linux distribution to run the software or
> use a virtual machine with a linux distribution, or use the Docker image, if
> you are using Windows or MacOS. Therefore, be free to test the software in
> other environments, but we can't guarantee that it will work as expected.

## 1.1 Using Cargo

Classeq works primarily as a command-line tool. To install it, you need to have
Rust installed on your machine. If you don't have Rust installed, you can
install following the official [installation documentation
guide](https://www.rust-lang.org/tools/install).

After installing Rust, you can install Classeq with CLI using the following
command:

```bash
cargo install classeq-cli
```

In addition to the CLI port, Classeq also has an API server. The server
configuration and usage are described in the [Configure the API
server](./04-configure-api-server.md) and [Place Sequences using the
API](./05-submit-placement-to-api.md) sections.

## 1.2 Using Docker

If you prefer to use Docker, you can pull the official Classeq image from Docker
Hub:

```bash
docker pull sgelias/classeq-cli:latest
```

[▶️ Next | Build Classeq Database](/docs/book/02-build-db.md)
