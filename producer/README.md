# producer (DTMSHR node)

RDMA server exposing a single system image compute node to consumers.

Rust. RDMA bring-up lives in the shared [`../rdma`](../rdma) crate (`dtmshr-rdma`), built on `rdma-sys` (bindgen bindings over rdma-core / libibverbs).

Current: opens first RDMA device, allocates a protection domain + completion queue, creates an RC queue pair (INIT state), registers a placeholder memory region. No connection handshake, no SSI exposure yet — see TODOs in `src/main.rs`.

## Build

Linux only. Needs rdma-core dev headers + libclang for bindgen:

```bash
sudo apt install -y libibverbs-dev librdmacm-dev clang pkg-config
cargo build -p dtmshr-producer
```

Won't build on plain Windows — no libibverbs there. Use WSL2 with an RDMA-capable NIC, or a Linux box/VM.

**Unverified**: written and reviewed by hand, not yet compiled — this environment has no Rust toolchain and no Linux/libibverbs. First `cargo build` on a real Linux box will likely need small fixes to match whatever `rdma-sys`'s bindgen output actually named things.

TODO: out-of-band connection exchange (qp_num/psn/lid or gid/rkey) + RTR/RTS transition, SSI compute exposure, resource limits tied to available power headroom.
