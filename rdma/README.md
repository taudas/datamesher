# dtmshr-rdma

Shared RDMA bring-up used by both `producer` and `consumer`: device open, protection domain, completion queue, RC queue pair, memory region registration. Both sides need the exact same libibverbs setup, so it lives here once instead of duplicated per crate.

Raw `rdma-sys` FFI underneath (rdma-core / libibverbs). See workspace root [`../README.md`](../README.md) for build requirements — note the crate actually used is [`../vendor/rdma-sys`](../vendor/rdma-sys), patched in via `[patch.crates-io]`.

Compiles clean on WSL2 Ubuntu 24.04 + rdma-core 61.0. Not yet run against a real device (see [../docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md)) — so verified at the type/FFI-signature level, not at the "does this actually connect a QP" level.
