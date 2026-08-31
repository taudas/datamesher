# Architecture

Datamesher: two actors trading spare CPU over RDMA. Reference: [Datamesher.ai](http://datamesher.ai)

## Actors

**Producer** — has spare/excess power capacity ("extra watts"). Runs a dedicated compute node with **DTMSHR**, which implements RDMA to expose a single system image (SSI) compute node to consumers.

**Consumer** — runs existing software on their existing machine, offloads CPU-bound work to a producer instead of scaling locally. Software stays the same; only where the CPU cycles come from changes.

```
 consumer host                          producer host
 +---------------------+                +---------------------+
 | existing software    |                |                       |
 |        |             |                |   DTMSHR              |
 | dtmshr-consumer       | <-- RDMA --> |   (dtmshr-producer)  |
 | (offload client)      |  RC queue pair |   exposes SSI        |
 |                       |                |   compute node        |
 +---------------------+                +---------------------+
```

## Workspace layout

Rust workspace, three crates:

- [`rdma/`](rdma) (`dtmshr-rdma`) — shared RDMA bring-up: open device, protection domain, completion queue, RC queue pair, memory region registration. Both producer and consumer need identical libibverbs setup, so it's factored out once rather than duplicated.
- [`producer/`](producer) (`dtmshr-producer`) — DTMSHR node agent.
- [`consumer/`](consumer) (`dtmshr-consumer`) — offload client.

All RDMA access goes through `rdma-sys` (bindgen bindings over rdma-core / libibverbs) — actually [`vendor/rdma-sys`](vendor/rdma-sys), a patched copy (see that dir's README for why). Linux only — see [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for environment setup.

## Current state

Both `producer` and `consumer`:

1. Open the first RDMA device on the host.
2. Allocate a protection domain and completion queue.
3. Create an RC (reliable connected) queue pair, transitioned to `INIT`.
4. Register a memory region (SSI buffer on the producer, offload staging buffer on the consumer).
5. Exchange `ConnectionInfo` (qp_num, psn, GID, rkey, addr) over a plain TCP handshake — producer listens (`net::accept_and_exchange`), consumer dials in (`net::connect_and_exchange`).
6. Drive the queue pair through `RTR` to `RTS` using the peer's info (`QueuePair::connect`), addressed by GID (RoCE has no LID).

At that point both sides have a connected RC queue pair and each other's rkey/addr. The producer posts a receive buffer and polls its completion queue in a loop, logging whatever lands on it. The consumer sends a `"ping {n}"` message every 5 seconds (`QueuePair::post_send`) and drains its own completion queue so it doesn't overflow. This is a heartbeat, not a request/response protocol, and not the SSI data path (which will likely be one-sided RDMA read/write, not send/receive).

The whole workspace compiles clean (`cargo build --workspace`) on WSL2 Ubuntu 24.04 with rdma-core 61.0. Not yet run against a real device or soft-RoCE — that WSL2 kernel doesn't ship `rdma_rxe` (see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)), so everything above is verified at the type/FFI level (every `ibv_*` call, struct field, and constified/bitfield enum reference matches what rdma-core 61.0's headers actually generate), not at the "does a QP actually reach RTS on real hardware" level.

## Open design questions

- **RDMA transport**: RoCEv2 vs. InfiniBand vs. iWARP vs. soft-RoCE (dev only, see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)). Current code assumes GID-based addressing (RoCE); pure InfiniBand would need LID-based `ah_attr` instead.
- **SSI model**: process migration vs. remote exec vs. full VM — decides what "exposing a compute node" actually means on the wire.
- **Discovery**: how a consumer finds/selects a producer (registry, broadcast, static config?).
- **Trust/auth**: what stops an unauthenticated consumer from attaching to a producer's QP.
- **Metering**: producer's "extra watts" budget, consumer's usage accounting.

## Code conventions

- `rdma-sys` is raw unsafe FFI (bindgen output, no safety wrapper upstream). Keep `unsafe` blocks narrow and confined to `rdma/src/lib.rs` — `producer` and `consumer` should only touch the safe wrapper types (`RdmaEndpoint`, `QueuePair`, `MemoryRegion`), never call `ibv_*` directly.
- Every wrapper type owns its libibverbs handle and frees it in `Drop`, in reverse creation order (CQ/PD/context/device-list; QP; MR). Don't add manual cleanup calls in `main.rs` — let `Drop` handle it.
