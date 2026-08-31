# consumer (offload client)

Runs on the consumer's existing machine. Offloads CPU-bound work from existing software to a producer's DTMSHR node over RDMA.

Rust. RDMA bring-up lives in the shared [`../rdma`](../rdma) crate (`dtmshr-rdma`), built on `rdma-sys` (bindgen bindings over rdma-core / libibverbs).

Current: mirrors the producer's bring-up, then dials the producer's TCP address, exchanges `ConnectionInfo`, and connects the queue pair through to RTS. Then sends a `"ping {n}"` heartbeat every 5s (two-sided send, matching the producer's receive loop). That's a traffic smoke test, not the offload path — no producer discovery, no workload hook yet. See TODOs in `src/main.rs`.

## Build

Linux only, same as producer:

```bash
sudo apt install -y libibverbs-dev librdmacm-dev clang pkg-config
cargo build -p dtmshr-consumer
```

## Run

```bash
cargo run -p dtmshr-consumer -- <producer-host>:7471
```

Compiles clean on WSL2 Ubuntu 24.04 + rdma-core 61.0. Not yet run against a real device — no RDMA hardware or working soft-RoCE in that environment (see [docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md)).

TODO: producer discovery/selection, workload interception/offload API.
