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

## Part 2 — the opt-in path is dead too (same day, follow-up)

The obvious rebuttal to Part 1 is "so enable the feature." That was tested. It does not work, and the evidence says it cannot work in this version.

**Experiment:** the same hello project was rebuilt with `risc0-zkvm = { version = "^3.0.5", features = ["metal"] }`. It compiles cleanly (2m01s). The observed run still verdicts **`cpu-observed`** with the identical CPU-HAL module lines — raw report: [evidence/prove-hello-metal-feature-debug.json](evidence/prove-hello-metal-feature-debug.json). Binary inspection confirms why: the built host contains **zero** `hal::metal` strings and three `hal::cpu` strings. The feature changed nothing.

**Why the feature is a no-op** (manifest citations from the local registry):

- `risc0-zkvm-3.0.5/Cargo.toml` — `cuda = ["prove", "risc0-circuit-keccak/cuda", "risc0-circuit-recursion/cuda", "risc0-circuit-rv32im/cuda", "risc0-groth16/cuda", "risc0-zkp/cuda", ...]`: the CUDA switch forwards to every proving subcrate. Directly below it, `metal = ["prove"]`: the Metal switch forwards to nothing. It is `prove` under another name.
- `risc0-circuit-rv32im-4.0.4/Cargo.toml` `[features]` — the circuit crate that performs witness generation and accumulation has features `cuda`, `default`, `execute`, `prove`, `std`, `witgen_debug`. **There is no `metal` feature.** The current rv32im circuit has CPU and CUDA lanes only.
- `risc0-zkp-3.0.4/src/hal/metal.rs` exists and `src/hal/mod.rs:22` declares the module — but with no circuit-level Metal lane to drive it, the MetalHal is orphaned library code.

**The default path is equally closed.** The prebuilt `r0vm` 3.0.5 binary for aarch64-apple-darwin (97.9 MB, shipped by rzup) was inspected directly: it embeds `risc0_zkp::hal::cpu` module strings with source paths (`risc0/zkp/src/hal/cpu.rs`), and zero `hal::metal` strings. It links Metal.framework, but no Metal HAL was compiled in. Since the default-features host delegates proving to r0vm, the out-of-box path executes on a prover binary that physically lacks GPU code.

**Combined conclusion:** on risc0 v3.0.5 / rv32im circuit 4.0.4, Apple Silicon GPU proving is not "off by default" — it is unreachable in every configuration: the default prover binary lacks the code, the opt-in feature is disconnected, and the circuit crate has no Metal lane to connect it to. This is consistent with, and stronger than, the docs complaint in #3753.

## Scope — what this does and does not establish

- It **does** establish: on this host and this version, proving ran on the CPU HAL in both the default configuration and the explicit `metal`-feature configuration; the shipped r0vm binary contains no Metal HAL; the `metal` feature forwards nowhere; the rv32im 4.0.4 circuit has no Metal lane. It also establishes that info-level logging is silent about lane selection — a user cannot tell from a normal run.
- It does **not** establish: behavior of other risc0 versions (whether any earlier release had a live Metal lane was not investigated), Bonsai/remote proving, or what the docs say (not independently re-read here; the docs claim is #3753's).
- Reproduction: `rzup install`, `cargo risczero new hello`, then `RUST_LOG=debug r0-metal-doctor prove --project hello --json`; for Part 2, add `features = ["metal"]` to the host's risc0-zkvm dependency and repeat.
