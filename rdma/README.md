# dtmshr-rdma

Shared RDMA bring-up used by both `producer` and `consumer`: device open, protection domain, completion queue, RC queue pair, memory region registration. Both sides need the exact same libibverbs setup, so it lives here once instead of duplicated per crate.

Raw `rdma-sys` FFI underneath (rdma-core / libibverbs). See workspace root [`../README.md`](../README.md) for build requirements.

**Unverified**: not yet compiled against a real `rdma-sys` build — see `producer/README.md` for why.
