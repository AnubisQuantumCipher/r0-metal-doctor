# Evidence index

Real `r0-metal-doctor` reports. **There are two kinds here — do not conflate
them.** All files are redacted (home path → `~`, username → `<user>`) and
ANSI-stripped. Regenerate your own with `r0-metal-doctor bundle --project <crate>`.

## 1. Stock risc0 → CPU (this is the finding)

Unmodified risc0 v3.0.5 proving on Apple Silicon runs on the **CPU** HAL. This is
the result documented in [../FINDINGS.md](../FINDINGS.md).

| File | Run | Verdict |
|------|-----|---------|
| `prove-hello-debug.json` | `cargo risczero new` hello, `RUST_LOG=debug` | `cpu-observed` |
| `prove-hello-info.json` | same, `RUST_LOG=info` | `indeterminate` (no lane lines at info) |
| `prove-hello-metal-feature-debug.json` | hello rebuilt with `features=["metal"]` | `cpu-observed` (the feature is inert) |
| `final-test-cpu-debug.json` | stock hello, debug | `cpu-observed` |

## 2. Patched risc0-metal-hybrid → Metal (a DIFFERENT prover)

These `metal-observed` runs come from the **separate, patched**
[risc0-metal-hybrid](https://github.com/AnubisQuantumCipher/risc0-metal-hybrid)
prover — **not** stock risc0. They demonstrate the Metal lane this doctor is
built to witness. **They do not contradict the stock-CPU finding above**: they
are a different prover, one that restores a real Metal lane to rv32im.

| File | Run | Verdict |
|------|-----|---------|
| `v0.2-hybrid-metal-observed.json` | risc0-metal-hybrid e2e, v0.2 schema (29 `hal::metal` lines) | `metal-observed` |
| `v0.2-hybrid-cpu-killswitch.json` | **same hybrid prover** forced to CPU with `ZKF_DISABLE_METAL=1`, v0.2 schema | `cpu-observed` |
| `prove-hybrid-metal-debug.json` | risc0-metal-hybrid e2e | `metal-observed` |
| `final-test-hybrid-debug.json` | risc0-metal-hybrid e2e + `RECEIPT VERIFIED` | `metal-observed` |
| `hybrid-stdout.txt`, `final-test-hybrid-stdout.txt` | raw stdout of the above | — |

## How to read a report

The verdict derives **only** from `matched_lines`, which ship in each file so you
can check the derivation. `lane_counts` shows how many lines were attributed to
each lane; a `metal-observed` verdict means the prover logged `…::hal::metal`
module paths during a run that exited successfully (a failed build is reported as
`indeterminate`, never as a lane).
