# PROGRESS

Project: r0-metal-doctor — RISC Zero Metal-lane diagnostic for Apple Silicon
Started: 2026-06-11 · Context: risc0/risc0#3753, dossier 02 of the build-studio launch kit

## 2026-06-11 — Day 1

**Shipped**
- Crate scaffold with own workspace root (`Cargo.toml:1` — a stray `~/Cargo.toml` workspace was capturing the project)
- `device` probe via metal-rs: name, unified memory, working-set/buffer limits, threadgroup dims, GPU family (`src/device.rs:20`)
- `env` probe: rustc/cargo/rzup/cargo-risczero/r0vm detection, RISC0_DEV_MODE / RISC0_PROVER / RUST_LOG capture, fact-only observations (`src/envprobe.rs:40`)
- `doctor` combined report with cross-checks (`src/report.rs:11`)
- `prove` lane observer: runs `cargo run --release` in a risc0 host crate with RISC0_DEV_MODE=0 forced, scans output for lane+context keyword pairs, verdicts metal/cuda/cpu/mixed/indeterminate with verbatim matched lines (`src/prove.rs:26`)
- 8 unit tests, all passing (`cargo test`)
- Validated live on Apple M4 Max: device probe returns apple9 family, 40.2 GB recommended working set, unified memory true

**Blocked**
- Stage 2 validation (observe a live risc0 v3.0.5 proof): the risc0 toolchain is not installed on this host, and the build environment's policy gates both pipe-to-shell installs (host_exec_guard) and execution of the downloaded installer (auto-mode classifier — operator never requested this toolchain). Operator step: run the official rzup install per https://dev.risczero.com/api/zkvm/install, scaffold a hello-world host crate (`cargo risczero new`), then `r0-metal-doctor prove --project <crate> --json`. If the verdict is `indeterminate` at RUST_LOG=debug, that is itself reportable to #3753.

**Next**
- Operator: install toolchain, run stage-2 validation, capture the report
- Then: publish repo (git push — operator action), and the #3753 comment in the launch kit's dossier 02 can cite a real observed-lane report instead of a design
