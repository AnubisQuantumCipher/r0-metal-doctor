# r0-metal-doctor

Diagnose whether RISC Zero proving on Apple Silicon actually uses Metal.

Context: [risc0/risc0#3753](https://github.com/risc0/risc0/issues/3753) reports that the release-3.0 docs say Metal proving is on by default for Apple Silicon, while v3.0.5 appears to silently fall back to CPU. Whether that is true on a given machine should be a measurement, not a docs dispute. This tool makes it one.

**Design rule: report, never assume.** The tool only claims a prover lane it watched run. Device capability is reported as device capability, not as "GPU proving works."

## Commands

```bash
# Metal GPU facts: device, unified memory, working-set and buffer limits, GPU family
r0-metal-doctor device --json

# Host toolchain + env facts: rustc, rzup/cargo-risczero/r0vm, RISC0_DEV_MODE, RISC0_PROVER
r0-metal-doctor env --json

# Both probes plus cross-checks in one report
r0-metal-doctor doctor --json

# The real question: run a proof in a risc0 host crate and report which lane the logs show
r0-metal-doctor prove --project path/to/risc0-host-crate --json
```

`prove` runs `cargo run --release` in the target project with `RISC0_DEV_MODE=0` (forced — a dev-mode run proves nothing about lanes) and `RUST_LOG=info` (or your value), captures all output, and scans for prover-lane evidence. A line only counts if it mentions a lane keyword (metal/cuda/cpu) **and** a proving-context keyword (prover/prove/proving/hal/receipt), so crate names in build output don't pollute the verdict. Verdicts:

- `metal-observed` / `cuda-observed` / `cpu-observed` — at least one matching log line, included verbatim in `matched_lines`
- `mixed` — multiple lanes mentioned; read `matched_lines` and judge
- `indeterminate` — no prover-selection lines captured; rerun with `RUST_LOG=debug`. **No claim is made.**

The verdict derives only from `matched_lines`. They ship in the report so you can check the derivation.

## Sample output (real run, Apple M4 Max, 2026-06-11)

```json
{
  "device": {
    "metal_available": true,
    "device_name": "Apple M4 Max",
    "unified_memory": true,
    "recommended_max_working_set_bytes": 40200896512,
    "max_buffer_length_bytes": 30150672384,
    "max_threads_per_threadgroup": [1024, 1024, 1024],
    "apple_gpu_family": "apple9"
  },
  "notes": [
    "Metal device present with unified memory (Apple M4 Max) — the hardware side of GPU proving is available",
    "risc0 toolchain not detected — install (rzup) and rerun; until a proof is observed, no lane claim can be made",
    "host is Apple Silicon macOS — CUDA is unavailable here by construction; the only GPU lane risc0 could use is Metal"
  ]
}
```

## Install

```bash
git clone https://github.com/AnubisQuantumCipher/r0-metal-doctor
cd r0-metal-doctor
cargo install --path .
```

Requires macOS for the Metal probe (other platforms get an honest "Metal does not exist here" report). The `prove` command additionally requires the [RISC Zero toolchain](https://dev.risczero.com/api/zkvm/install) and a risc0 host crate to observe.

## Status

- `device`, `env`, `doctor`: working, tested (8 unit tests), validated on Apple M4 Max.
- `prove`: **validated against live risc0 v3.0.5 proof runs** (2026-06-11, M4 Max), in both default and `metal`-feature configurations. Both verdict **`cpu-observed`** at `RUST_LOG=debug`; at `info` the verdict is `indeterminate` because risc0 emits no lane-selection lines at that level. Combined finding: on v3.0.5, Apple Silicon GPU proving is unreachable in every configuration — the shipped r0vm binary contains no Metal HAL, the `metal` cargo feature forwards nowhere (unlike `cuda`), and the rv32im 4.0.4 circuit has no Metal lane at all. Raw reports in [evidence/](evidence/), full writeup with exact citations in [FINDINGS.md](FINDINGS.md).

## License

MIT
