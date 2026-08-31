# producer (DTMSHR node)

RDMA server exposing a single system image compute node to consumers.

Rust, `rdma-sys` (bindgen bindings over rdma-core / libibverbs).

Current: enumerates first RDMA device, opens it, allocates a protection domain and completion queue. No queue pairs, no SSI exposure yet.

## Build

Linux only. Needs rdma-core dev headers + libclang for bindgen:

```bash
sudo apt install -y libibverbs-dev librdmacm-dev clang pkg-config
cargo build
```

Won't build on plain Windows — no libibverbs there. Use WSL2 with an RDMA-capable NIC, or a Linux box/VM.

TODO: RDMA transport (RC queue pairs), SSI compute exposure, resource limits tied to available power headroom.
