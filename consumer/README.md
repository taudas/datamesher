# consumer (offload client)

Runs on the consumer's existing machine. Offloads CPU-bound work from existing software to a producer's DTMSHR node over RDMA.

Rust. RDMA bring-up lives in the shared [`../rdma`](../rdma) crate (`dtmshr-rdma`), built on `rdma-sys` (bindgen bindings over rdma-core / libibverbs).

Current: mirrors the producer's bring-up — opens first RDMA device, allocates a protection domain + completion queue, creates an RC queue pair (INIT state), registers a placeholder staging memory region. No producer discovery, no connection handshake, no workload hook yet — see TODOs in `src/main.rs`.

## Build

Linux only, same as producer:

```bash
sudo apt install -y libibverbs-dev librdmacm-dev clang pkg-config
cargo build -p dtmshr-consumer
```

**Unverified**: not yet compiled — this environment has no Rust toolchain and no Linux/libibverbs.

TODO: producer discovery/selection, out-of-band connection exchange + RTR/RTS transition, workload interception/offload API.
