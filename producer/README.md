# producer (DTMSHR node)

RDMA server exposing a single system image compute node to consumers.

Rust. RDMA bring-up lives in the shared [`../rdma`](../rdma) crate (`dtmshr-rdma`), built on `rdma-sys` (bindgen bindings over rdma-core / libibverbs).

Current: opens first RDMA device, brings up an RC queue pair and memory region, listens on TCP for a consumer's `ConnectionInfo`, exchanges its own, and connects the queue pair through to RTS. Then serves the [`rdma::job`](../rdma/src/job.rs) protocol: receives a `JobRequest`, executes it, RDMA WRITEs the result into the buffer the consumer named, and sends a `JobDone` notice. That's remote-exec-as-a-service (one placeholder opcode: uppercase), not real SSI compute exposure — see [ARCHITECTURE.md](../ARCHITECTURE.md#open-design-questions).

## Build

Linux only. Needs rdma-core dev headers + libclang for bindgen:

```bash
sudo apt install -y libibverbs-dev librdmacm-dev clang pkg-config
cargo build -p dtmshr-producer
```

## Run

```bash
cargo run -p dtmshr-producer -- 0.0.0.0:7471   # bind addr, defaults to 0.0.0.0:7471
```

Compiles clean on WSL2 Ubuntu 24.04 + rdma-core 61.0. Not yet run against a real device — no RDMA hardware or working soft-RoCE in that environment (see [docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md)).

Won't build on plain Windows — no libibverbs there. Use WSL2 with an RDMA-capable NIC, or a Linux box/VM.

TODO: SSI compute exposure, resource limits tied to available power headroom.
