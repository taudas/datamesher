# dtmshr-rdma

Shared RDMA bring-up used by both `producer` and `consumer`: device open, protection domain, completion queue, RC queue pair, memory region registration, RDMA WRITE. Both sides need the exact same libibverbs setup, so it lives here once instead of duplicated per crate.

Also has `job` — a minimal request/response protocol (encode/decode + a placeholder "execute" function) layered on top, shared because both producer and consumer need to speak the same wire format. See its module docs and [`../ARCHITECTURE.md`](../ARCHITECTURE.md#current-state).

Raw `rdma-sys` FFI underneath (rdma-core / libibverbs). See workspace root [`../README.md`](../README.md) for build requirements — note the crate actually used is [`../vendor/rdma-sys`](../vendor/rdma-sys), patched in via `[patch.crates-io]`.

Compiles clean on WSL2 Ubuntu 24.04 + rdma-core 61.0, with real unit tests for `job`'s encode/decode/execute (pure logic, no hardware needed — `cargo test -p dtmshr-rdma`). The RDMA calls themselves aren't run against a real device yet (see [../docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md)) — verified at the type/FFI-signature level, not at the "does this actually connect a QP" level.
