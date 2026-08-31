# Contributing

## Prerequisites

- Linux (native or WSL2 — see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for WSL2 setup). RDMA on plain Windows isn't a thing this project targets.
- Rust (stable), via [rustup](https://rustup.rs).
- rdma-core dev headers + a C compiler toolchain, for `rdma-sys`'s bindgen build step:

  ```bash
  sudo apt install -y libibverbs-dev librdmacm-dev clang pkg-config build-essential
  ```

- An RDMA device. Real hardware, or soft-RoCE (`rxe`) for development without any — see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Build

```bash
cargo build --workspace
```

Or a single crate:

```bash
cargo build -p dtmshr-producer
cargo build -p dtmshr-consumer
```

## Before you touch RDMA code

Read [ARCHITECTURE.md](ARCHITECTURE.md), specifically "Code conventions". Short version: unsafe `ibv_*` FFI calls stay inside `rdma/src/lib.rs`; `producer` and `consumer` only use the safe wrapper types.

## Commit style

Small, focused commits. Message: what changed, then why (not what-and-how restated). No `--no-verify`, no force-push to `master`.

## Status

Everything here is early scaffold — see [ARCHITECTURE.md](ARCHITECTURE.md#current-state) for exactly what works today. None of the current RDMA code has been compiled or run against real hardware yet; if you're the first to do that, expect to fix bindgen/API mismatches and report back.
