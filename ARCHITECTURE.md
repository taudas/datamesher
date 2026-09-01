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

At that point both sides have a connected RC queue pair and each other's rkey/addr, and run a small job/RPC protocol on top (`rdma::job`):

1. Consumer builds a `JobRequest` (opcode + inline input + its own result buffer's addr/rkey) and sends it two-sided (`QueuePair::post_send`).
2. Producer receives it, runs `job::execute` (currently one opcode: uppercase the input — a placeholder job picked to prove the round trip without opening the arbitrary-code-execution/sandboxing question a real job type would raise), and RDMA WRITEs the result straight into the consumer's named buffer (`QueuePair::post_rdma_write`) — one-sided, the consumer's CPU does nothing for this step.
3. A plain RDMA WRITE doesn't tell the receiver it landed, so the producer follows up with a two-sided `JobDone { job_id }` notice. The consumer, waiting on that, then reads its own result buffer locally.

This is remote-exec-as-a-service, explicitly **not** SSI (see below) — it's the concrete first answer to "what does offload mean on the wire," scoped down from the harder single-system-image framing in the project's original pitch.

The whole workspace compiles clean (`cargo build --workspace`) on WSL2 Ubuntu 24.04 with rdma-core 61.0, and `rdma::job`'s encode/decode/execute logic has real unit tests (`cargo test --workspace`, no hardware needed — it's pure data). Not yet run against a real device or soft-RoCE — that WSL2 kernel doesn't ship `rdma_rxe` (see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)), so the RDMA calls themselves are verified at the type/FFI level (every `ibv_*` call, struct field, and constified/bitfield enum reference matches what rdma-core 61.0's headers actually generate), not at the "does a QP actually reach RTS and carry a job on real hardware" level.

## Open design questions

- **RDMA transport**: RoCEv2 vs. InfiniBand vs. iWARP vs. soft-RoCE (dev only, see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)). Current code assumes GID-based addressing (RoCE); pure InfiniBand would need LID-based `ah_attr` instead.
- **Real job execution**: the job/RPC model above is scoped to one safe placeholder opcode. A real job type means deciding what code actually gets to run on the producer, and how it's sandboxed/resource-limited — this is a security question as much as a design one.
- **SSI model**: is "single system image" (process migration, full VM) ever actually pursued, or does the project settle on job/RPC as the permanent model? Left open; job/RPC is what's built.
- **Discovery**: how a consumer finds/selects a producer (registry, broadcast, static config?).
- **Trust/auth**: what stops an unauthenticated consumer from attaching to a producer's QP, or submitting jobs it shouldn't be able to run.
- **Metering**: producer's "extra watts" budget, consumer's usage accounting.

## Code conventions

- `rdma-sys` is raw unsafe FFI (bindgen output, no safety wrapper upstream). Keep `unsafe` blocks narrow and confined to `rdma/src/lib.rs` — `producer` and `consumer` should only touch the safe wrapper types (`RdmaEndpoint`, `QueuePair`, `MemoryRegion`), never call `ibv_*` directly.
- Every wrapper type owns its libibverbs handle and frees it in `Drop`, in reverse creation order (CQ/PD/context/device-list; QP; MR). Don't add manual cleanup calls in `main.rs` — let `Drop` handle it.
