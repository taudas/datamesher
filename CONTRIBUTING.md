# Contributing

## Prerequisites

- Linux (native or WSL2 — see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for WSL2 setup). RDMA on plain Windows isn't a thing this project targets.
- Rust (stable), via [rustup](https://rustup.rs).
- rdma-core dev headers + a C compiler toolchain, for `rdma-sys`'s bindgen build step:

  ```bash
  sudo apt install -y libibverbs-dev librdmacm-dev clang pkg-config build-essential
  ```

- An RDMA device to *run* against (not needed just to build) — real hardware, or soft-RoCE (`rxe`) for development without any. See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md); note stock WSL2's kernel doesn't ship `rdma_rxe`.

`rdma-sys`'s pinned `bindgen` version can't parse newer rdma-core headers (Ubuntu 24.04+) — already patched via a vendored copy, see [vendor/rdma-sys](vendor/rdma-sys). Nothing extra needed on your end.

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

## CI/CD

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on every push/PR to `master`: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`. All four pass clean as of this writing — keep it that way; run them locally before pushing:

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

[`.github/workflows/release.yml`](.github/workflows/release.yml) builds release binaries and attaches them to a GitHub Release whenever a `v*.*.*` tag is pushed (`git tag v0.1.0 && git push origin v0.1.0`). Linux x86_64 only, no other targets make sense for this project.

## Status

Everything here is early scaffold — see [ARCHITECTURE.md](ARCHITECTURE.md#current-state) for exactly what works today. `cargo build --workspace` is verified clean on WSL2 Ubuntu 24.04 + rdma-core 61.0. None of it has run against a real device or soft-RoCE yet; if you're the first to do that, expect some runtime surprises and please report back.
