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
- Note: README's clone URL and Cargo.toml's `repository` field point at github.com/AnubisQuantumCipher/r0-metal-doctor, which 404s until the push happens — they describe the published state on purpose

## 2026-06-11 — Day 1, evening: stage 2 complete

**Shipped**
- Operator approved the toolchain install ("say the word" → word said). rzup 0.5.0 → cargo-risczero 3.0.5, r0vm 3.0.5, guest rust 1.94.1, cpp 2024.1.5
- Hello-world project scaffolded via `cargo risczero new` at /tmp/r0probe-hello
- Two live observations, reports preserved in `evidence/`:
  - `RUST_LOG=info`: exit 0, 70.2 s, verdict `indeterminate` (zero lane lines at info — itself a finding: a normal run never tells you the lane)
  - `RUST_LOG=debug`: exit 0, 33.2 s, verdict **`cpu-observed`** — six lines, all `risc0_zkp::hal::cpu` / `risc0_circuit_rv32im::prove::hal::cpu` (`src/prove.rs:26` scanner, evidence verbatim)
- Static corroboration recorded in FINDINGS.md: risc0-zkp `default = []`, `metal = ["prove"]` (Cargo.toml:45-46); risc0-zkvm `default = ["client","bonsai"]` (Cargo.toml:22-25); Metal HAL source present but feature-gated off; `cargo tree -e features` confirms the observed build never resolved the metal feature
- Human-readable `doctor` output rewritten from Debug structs to formatted text (independent verification report's limitation #4)
- An independent verification session (separate report on the operator's desktop) confirmed: clean from-scratch build, 8/8 tests, device facts corroborated against the OS, no-overclaim behavior on every tested path

**Blocked**
- Publishing (git push) remains an operator action

**Next**
- Operator: push the repo, then the #3753 comment (launch kit dossier 02) ships with tool + evidence + findings attached

## 2026-06-11 — Day 1, night: the opt-in path tested, found dead

**Shipped**
- Operator asked for actual GPU proving. Tested the only remaining path: rebuilt the hello host with `risc0-zkvm features = ["metal"]`. Compiles (2m01s), runs, still **`cpu-observed`** — identical CPU-HAL lines (`evidence/prove-hello-metal-feature-debug.json`)
- Binary forensics, host: zero `hal::metal` strings, three `hal::cpu` strings — the feature compiled no Metal code
- Binary forensics, r0vm 3.0.5 prebuilt (97.9 MB): links Metal.framework but embeds zero `hal::metal` strings and `risc0/zkp/src/hal/cpu.rs` source paths — the default prover server is CPU-only by construction
- Manifest root cause in FINDINGS.md Part 2: risc0-zkvm `cuda` forwards to six subcrates, `metal = ["prove"]` forwards to none; risc0-circuit-rv32im 4.0.4 has NO metal feature — the circuit has CPU and CUDA lanes only; risc0-zkp's MetalHal is orphaned
- Conclusion upgraded: GPU proving on risc0 v3.0.5 / Apple Silicon is not "off by default" — it is unreachable in every configuration

**Next**
- Operator: push the repo; the #3753 comment now carries a complete impossibility proof, not just a default-behavior observation
- The constructive follow-on (and the commercial one): restoring a Metal lane to the rv32im 4.x circuit is real engineering work in exactly the operator's specialty (verified Metal proving kernels). That conversation belongs off-thread, after the finding lands
