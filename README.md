# Datamesher

[![CI](https://github.com/taudas/datamesher/actions/workflows/ci.yml/badge.svg)](https://github.com/taudas/datamesher/actions/workflows/ci.yml)

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

Early scaffold. Both sides open a device, bring up an RC queue pair + memory region, exchange connection info over TCP, and connect the queue pair to RTS. Consumer sends a heartbeat, producer receives and logs it. No SSI exposure yet, no offload path, no request/response protocol.

Compiles clean (`cargo build --workspace`) on WSL2 Ubuntu 24.04 with rdma-core 61.0 — see [vendor/rdma-sys](vendor/rdma-sys) for the one dependency patch that took. Not yet run against a real RDMA device or soft-RoCE (WSL2's stock kernel doesn't ship `rdma_rxe`; see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)).
