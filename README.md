# r0-metal-doctor

**A report-never-assume doctor for ZK proving on Apple Silicon.** A validated
Metal-lane observer for [RISC Zero](https://risczero.com) (risc0), plus an honest
CPU-only view of [Midnight](https://midnight.network). It only ever names a
compute lane it watched run — it never asserts one from configuration.

[![ci](https://github.com/AnubisQuantumCipher/r0-metal-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/AnubisQuantumCipher/r0-metal-doctor/actions/workflows/ci.yml)
&nbsp;License: MIT

Companion to **[risc0-metal-hybrid](https://github.com/AnubisQuantumCipher/risc0-metal-hybrid)**,
which restores a real Metal proving lane to risc0 on Apple Silicon. This tool is
the independent witness: it reports `metal-observed` for that prover and
`cpu-observed` for stock risc0, from the runtime logs' module paths.

---

## Why

risc0/risc0#3753 reports that the release-3.0 docs say Metal proving is on by
default for Apple Silicon, while in practice it falls back to CPU. Whether that
is true on a given machine should be a **measurement, not a docs dispute**. This
tool makes it one.

**Design rule: report, never assume.** The tool only claims a lane it watched
run. Device capability is reported as device capability — not as "GPU proving
works." When it cannot tell, it says `indeterminate` and makes no claim.

## Install

```bash
# From source (works today)
git clone https://github.com/AnubisQuantumCipher/r0-metal-doctor
cd r0-metal-doctor
cargo install --path .
```

Requires macOS for the Metal probe (other platforms get an honest "Metal does
not exist here" report). The `prove` command additionally needs the
[RISC Zero toolchain](https://dev.risczero.com/api/zkvm/install).

> Prebuilt binaries, `brew install`, and `cargo binstall` become available with
> the first tagged GitHub Release (see [release.md](release.md)). They are not
> claimed to work before that release exists.

## Commands

```bash
# Universal probes — useful to any Apple-Silicon ZK developer
r0-metal-doctor device          # Metal GPU: name, unified memory, working-set/buffer limits, family
r0-metal-doctor env             # host: rustc + risc0 toolchain, env vars, misconfiguration checks
r0-metal-doctor doctor          # device + env + cross-checks + version-scoped notes
r0-metal-doctor check           # one-line, paste-ready health summary
r0-metal-doctor targets         # the proving targets the doctor knows, with honesty badges

# risc0: observe which lane a real proof actually uses
r0-metal-doctor prove --project path/to/risc0-host-crate

# Midnight: CPU-only target — proof-server reachability, compactc, container identity
r0-metal-doctor midnight

# Bundle a redacted evidence directory for a bug report
r0-metal-doctor bundle --project path/to/risc0-host-crate

# Any command: add --json for machine-readable output
```

### `prove` — the headline command

Runs `cargo run --release` in the target project with `RISC0_DEV_MODE=0` forced
(a dev-mode run produces fake receipts and proves nothing about lanes) and
`RUST_LOG=debug` (risc0 emits no lane lines at `info`). It **streams the run
live** under a wall-clock timeout, then scans the captured logs. A line counts
toward a lane only if it carries that backend's **HAL module path** (e.g.
`…::hal::metal`) or an explicit lane-selection phrase — not merely the word in a
proving context. So a crate name in build output, a `hal/metal.rs` path in a
compiler error, and the always-on-CPU RISC-V executor (`…::prove::executor`) all
**cannot** move the verdict. A non-zero exit or a timeout claims no lane.

Verdicts: `metal-observed` / `cuda-observed` / `cpu-observed`; `mixed` reports
the **dominant** lane (a hybrid run is mostly-GPU with a few CPU kernel lines —
the GPU is not buried); `indeterminate` makes no claim. The verdict derives only
from `matched_lines`, which ship in the report so you can check it.

### CI gating: `--expect` and exit codes

`prove --expect metal` turns the diagnostic into an executable contract:

| Exit | Meaning |
|------|---------|
| `0`  | ok — observed lane matched `--expect` (or no `--expect` given) |
| `1`  | mismatch — a different lane was observed |
| `2`  | **indeterminate** — could not tell (distinct from a mismatch) |
| `3`  | environment / usage error |

```bash
# Fail the build the moment proving falls back to CPU:
r0-metal-doctor prove --project . --expect metal
```

(This is the tool's own scheme, documented here — not a system standard.)

## Two proving targets, honest about each

`r0-metal-doctor targets` lists what the doctor can observe and how validated
each lane is. The honesty is structural: a `[N/A]` lane can never render as a
working one.

| Target | Metal lane | Compute | What this tool does |
|--------|-----------|---------|---------------------|
| **risc0** (rv32im) | `[VALIDATED]` observer | CPU or Metal | Watches a real proof run and reports the lane from `risc0_…::hal::{metal,cpu}` module paths. |
| **Midnight** | `[N/A]` — see below | **CPU only** | Reports proof-server reachability, `compactc`, container identity, env routing. |

### About Midnight (read this before assuming)

Midnight does **not** prove on the Metal GPU on a Mac — categorically, on two
independent grounds:

1. Its proving system is **Plonk + KZG over BLS12-381** (halo2 lineage; a
   classical, trusted-setup pairing SNARK — **not** a STARK, not post-quantum).
   The stock proof server is CPU-only; the only GPU fork is **NVIDIA-CUDA-only**.
2. The proof server runs as a **Linux Docker container**, which on macOS has no
   Metal passthrough even in principle.

So Midnight is **not** risc0, does **not** use Metal, and this tool says so as a
first-class finding. For a Midnight developer it honestly reports: device
capability, whether the proof server is reachable (`/health`), `compactc`
version, container identity, and env routing — never an invented GPU lane.

## The finding (version-scoped, so it stays true)

On **risc0-zkvm 3.0.5 / risc0-zkp 3.0.4 / risc0-circuit-rv32im 4.0.4** (the
tested trio, M4 Max; established 2026-06-11 — see [FINDINGS.md](FINDINGS.md) —
and re-verified with v0.2 on 2026-06-13), stock risc0 proves rv32im **on the
CPU** on Apple Silicon. Three independent, non-conflated facts:

1. risc0-zkp **ships** a Metal HAL that compiles — "a Metal HAL exists" is true.
2. The `metal` cargo feature is **inert** for rv32im (it maps only to `prove`;
   on risc0-zkvm 3.0.5 it is deprecated) — enabling it changes nothing.
3. The rv32im circuit's **prover never calls the HAL** (no Metal branch is
   reachable in any feature configuration) — so it falls back to CPU.

This is **not a timeless claim about "risc0 and Metal."** Upstream
[PR #3688](https://github.com/risc0/risc0/pull/3688) (merged to `main`
2026-01-30, **unreleased**) adds a real Metal lane and is the sentinel that will
make this false; risc0 5.0.0-rc.1 is a different ("m3") architecture and this
evidence does not apply to it. `prove` re-checks the observed version against
this matrix and emits `indeterminate` for anything untested — it never
auto-flips to "Metal works."

Full writeup with exact citations: **[FINDINGS.md](FINDINGS.md)**.

## Evidence

The [`evidence/`](evidence/) directory holds real reports. **Two kinds — do not
conflate them** ([evidence/README.md](evidence/README.md) has the index):

- **stock risc0** runs verdict `cpu-observed` (or `indeterminate` at `info`).
- **`metal-observed`** runs come from the *patched*
  [risc0-metal-hybrid](https://github.com/AnubisQuantumCipher/risc0-metal-hybrid)
  prover — a separate project — and demonstrate the lane the doctor is built to
  witness. They do **not** contradict the stock finding above; they are a
  different prover.

## Safety

`prove`/`bundle` with `--project` run `cargo run` on the target project, which
**executes that project's code (not sandboxed) — point it only at projects you
trust.** All shareable output is **redacted by default** (home path → `~`,
username → `<user>`); `--no-redact` to disable. See [SECURITY.md](SECURITY.md).

## License

MIT
