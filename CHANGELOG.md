# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-06-13

### Added
- **Dual-target registry** (`targets`): risc0 (a validated Metal-lane observer)
  and Midnight (CPU-only — Metal structurally **not applicable** on Apple
  Silicon), with per-lane honesty badges so a Midnight Metal result is
  impossible to render.
- **`midnight`** subcommand: observe the Midnight proving target firsthand —
  `compactc` detection, proof-server reachability (TCP + HTTP `/health`),
  Docker container identity, and env routing — stating the CPU-only /
  Metal-not-applicable finding with its reasons. No proof-server log schema is
  assumed.
- **`check`** subcommand: a one-line, paste-ready health summary.
- **`bundle`** subcommand: write a timestamped, **redacted** evidence directory
  for bug reports.
- **`prove --expect <lane>`** plus verdict-keyed exit codes (`0` ok, `1`
  mismatch, `2` indeterminate, `3` error) so CI can gate on the observed lane.
- **`prove --timeout-secs`** (default 600): a wall-clock budget; on expiry the
  run is killed and the verdict fails closed to indeterminate.
- **Live streaming** of the proof subprocess — progress instead of an apparent
  hang, and a stuck target can no longer hang the doctor.
- **risc0 version matrix**: reports self-date against tested versions and emit
  *indeterminate* for untested ones, keeping the "Metal unreachable" finding
  bound to risc0-zkvm 3.0.5 / risc0-zkp 3.0.4 / rv32im 4.0.4 (with the PR #3688
  and 5.0.0-rc.1 sentinels noted).
- Typed environment **checks** with remediation (RISC0_DEV_MODE, RISC0_PROVER,
  toolchain skew).
- `deny.toml` + a CI supply-chain gate (cargo-deny), CI on Apple Silicon plus a
  non-macOS stub build, and `SECURITY.md`.

### Changed
- Default `prove` log level is now `debug` (risc0 emits no lane lines at `info`,
  so the headline command no longer defaults to indeterminate).
- ANSI escape sequences are stripped from matched lines; host paths and username
  are **redacted by default** in all shareable output (`--no-redact` to disable).
- **Dominant-lane verdict**: a hybrid run (mostly GPU + a few CPU lines) reports
  the dominant lane instead of collapsing to an uninformative "mixed".
- Real human-readable output for `device` / `env` / `prove` (previously raw
  Rust debug structs); `--json` remains the stable machine contract.

### Fixed
- Removed a stray `src/main 2.rs` copy-collision file.

## [0.1.0] - 2026-06-11

### Added
- Initial release: `device`, `env`, `doctor`, and `prove` subcommands — a Metal
  device probe, a host/risc0 toolchain probe, and log-based prover-lane
  observation with a "report, never assume" verdict
  (metal / cuda / cpu / mixed / indeterminate) and 8 unit tests.

[Unreleased]: https://github.com/AnubisQuantumCipher/r0-metal-doctor/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/AnubisQuantumCipher/r0-metal-doctor/releases/tag/v0.2.0
[0.1.0]: https://github.com/AnubisQuantumCipher/r0-metal-doctor/releases/tag/v0.1.0
