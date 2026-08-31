**Vendored from [rdma-sys 0.3.0](https://crates.io/crates/rdma-sys/0.3.0).** Its pinned `bindgen = "^0.59.2"` panics on the anonymous-union naming in newer rdma-core headers (hit on Ubuntu 24.04's `ib_user_ioctl_verbs.h`, rdma-core 61.0) — `"ib_uverbs_flow_action_esp_encap_union_(anonymous_at_...)" is not a valid Ident`. Only change here: `Cargo.toml`'s `bindgen` dependency bumped from `0.59.2` to `0.72`, which parses those headers fine. Everything else, including `build.rs` and the manually-written types in `src/`, is unmodified upstream. Wired in via `[patch.crates-io]` in the workspace root `Cargo.toml`.

---

# Rdma ibverbs lib Rust binding

This lib is the ibverbs low-level Rust binding. As inline function and nested structure are not handled properly in the automatic bind generator, we deal with them case by case manually.
