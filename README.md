# Datamesher

Distributed compute mesh. Spare CPU cycles from producers, offloaded to consumers, over RDMA.

Reference: [Datamesher.ai](http://datamesher.ai)

Two actors:

- **Producer** — spare/excess power capacity ("extra watts"), runs **DTMSHR** to expose a single system image (SSI) compute node over RDMA.
- **Consumer** — existing machine, existing software, offloads CPU to a producer over RDMA instead of scaling locally.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design and current implementation state.

## Components

Rust workspace, RDMA via `rdma-sys` (rdma-core / libibverbs bindings). Linux only.

- [`rdma/`](rdma) — shared RDMA bring-up (`dtmshr-rdma`): device open, PD, CQ, RC queue pair, memory region registration.
- [`producer/`](producer) — DTMSHR node agent (RDMA server, SSI exposure).
- [`consumer/`](consumer) — offload client (hooks into existing workloads, routes CPU-bound work to a producer over RDMA).
- [`docs/`](docs) — deeper dev/setup notes.

## Getting started

- New here? Start with [ARCHITECTURE.md](ARCHITECTURE.md).
- Setting up a dev environment (build deps, soft-RoCE for testing without real RDMA hardware, WSL2 notes)? [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).
- Making changes? [CONTRIBUTING.md](CONTRIBUTING.md).

## Status

Early scaffold. Both sides open a device and bring up an RC queue pair + memory region locally. No connection handshake between them yet, no SSI exposure, no workload offload hook. Unverified against a real build — no Rust/Linux/libibverbs in the environment this was written in.
