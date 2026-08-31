# Datamesher

Distributed compute mesh. Spare CPU cycles from producers, offloaded to consumers, over RDMA.

Reference: [Datamesher.ai](http://datamesher.ai)

## Actors

### Producer

Runs a dedicated compute node on spare/excess power capacity ("extra watts"). Runs **DTMSHR**, which implements RDMA to expose a single system image (SSI) compute node to consumers.

- Hardware: idle/underutilized machine, power headroom available.
- Software: `DTMSHR` node agent.
- Exposes: one RDMA-backed SSI compute endpoint.

### Consumer

Runs existing software on their existing machine, offloading CPU load to a producer instead of scaling locally.

- Hardware/software: unchanged, existing workload.
- Offload path: existing app -> DTMSHR client -> RDMA -> producer's SSI node.

## Components

Rust workspace, RDMA via `rdma-sys` (rdma-core / libibverbs bindings). Linux only — see each crate's README for build requirements.

- `rdma/` — shared RDMA bring-up (`dtmshr-rdma`): device open, PD, CQ, RC queue pair, memory region registration.
- `producer/` — DTMSHR node agent (RDMA server, SSI exposure).
- `consumer/` — offload client (hooks into existing workloads, routes CPU-bound work to a producer over RDMA).
- `docs/` — architecture, protocol, and design notes.

## Status

Early scaffold. Both sides open a device and bring up an RC queue pair + memory region locally. No connection handshake between them yet, no SSI exposure, no workload offload hook. Unverified against a real build — no Rust/Linux/libibverbs in the environment this was written in.
