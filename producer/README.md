# producer (DTMSHR node)

RDMA server exposing a single system image compute node to consumers.

Rust. RDMA bring-up lives in the shared [`../rdma`](../rdma) crate (`dtmshr-rdma`), built on `rdma-sys` (bindgen bindings over rdma-core / libibverbs).

Current: opens first RDMA device, brings up an RC queue pair and memory region, listens on TCP for a consumer's `ConnectionInfo`, exchanges its own, and connects the queue pair through to RTS. Then posts a receive buffer and polls its completion queue in a loop, logging any two-sided message that lands on it. That's a control channel, not SSI exposure — no request/response protocol yet, and the consumer doesn't send anything yet either. See TODOs in `src/main.rs`.

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
