# Findings — risc0 v3.0.5 proving lane on Apple Silicon

Date: 2026-06-11 · Host: Apple M4 Max, macOS 26.0, 48 GB unified memory
Context: [risc0/risc0#3753](https://github.com/risc0/risc0/issues/3753) ("release-3.0 docs incorrectly claim Metal proving is enabled by default")

## The observed result

A stock risc0 setup — toolchain installed via the official rzup (cargo-risczero 3.0.5, r0vm 3.0.5), project scaffolded with `cargo risczero new`, host crate depending on `risc0-zkvm = { version = "^3.0.5" }` with default features — was run through `r0-metal-doctor prove` with `RISC0_DEV_MODE=0` forced.

**Run 1 (`RUST_LOG=info`):** proof completed (exit 0, 70.2 s wall including first build). Zero prover-selection lines in the logs. Verdict: `indeterminate` — at info level, risc0 v3.0.5 does not say which lane it used. Raw report: [evidence/prove-hello-info.json](evidence/prove-hello-info.json).

**Run 2 (`RUST_LOG=debug`):** proof completed (exit 0, 33.2 s wall). Verdict: **`cpu-observed`**. Six matched lines, all from CPU HAL modules — module paths verbatim:

```
risc0_circuit_rv32im::prove::hal::cpu: witgen: 32768
risc0_circuit_rv32im::prove::hal::cpu: accumulate: 32768
risc0_zkp::hal::cpu: output: 163840, input: 3375104, combos: 103, ...
(3 further risc0_zkp::hal::cpu lines)
```

Raw report with full ANSI-coded lines: [evidence/prove-hello-debug.json](evidence/prove-hello-debug.json).

**On a machine where the Metal hardware side is fully present** (system-default Metal device, unified memory, apple9 GPU family, ~40.2 GB recommended working set — independently corroborated against the OS), **a stock risc0 v3.0.5 proof executed its HAL operations on the CPU.** No Metal HAL module appears anywhere in the captured logs.

## Why — static evidence from the installed crates

The published crate manifests explain the observation. File paths are from the local cargo registry (`~/.cargo/registry/src/index.crates.io-*/`):

- `risc0-zkp-3.0.4/Cargo.toml:45-46` — `default = []` and `metal = ["prove"]`. The Metal HAL is feature-gated and **not** a default feature. The Metal HAL source does exist (`risc0-zkp-3.0.4/src/hal/metal.rs`), so the lane is shipped but dormant.
- `risc0-zkvm-3.0.5/Cargo.toml:22-25` — `default = ["client", "bonsai"]`. Neither `prove` nor `metal` is in the default feature set.
- The `cargo risczero new` template host depends on `risc0-zkvm = { version = "^3.0.5" }` with no feature overrides, and `cargo tree -e features` on the actual built project confirms only `default`/`client`/`bonsai` resolved.

So a default install + default template never compiles the Metal feature into the proving path. Whether the release-3.0 documentation claims otherwise is the subject of #3753 (the issue quotes the docs); this repo's claim is limited to what was observed and what the manifests state.

## Scope — what this does and does not establish

- It **does** establish: on this host, this risc0 version, with the official install path and default template, proving ran on the CPU HAL while a fully capable Metal device sat idle. It also establishes that info-level logging is silent about lane selection — a user cannot tell from a normal run.
- It does **not** establish: behavior of other versions, of projects that explicitly enable the `metal` feature, of Bonsai/remote proving, or what the docs say (not independently re-read here).
- Reproduction: `rzup install`, `cargo risczero new hello`, then `RUST_LOG=debug r0-metal-doctor prove --project hello --json`.
