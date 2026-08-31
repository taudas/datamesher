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

- `producer/` — DTMSHR node agent (RDMA server, SSI exposure).
- `consumer/` — offload client (hooks into existing workloads, routes CPU-bound work to a producer over RDMA).
- `docs/` — architecture, protocol, and design notes.

## Status

Early scaffold. No implementation yet.
