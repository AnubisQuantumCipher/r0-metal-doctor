# Contributing to r0-metal-doctor

Thanks for helping! This tool has one prime directive that shapes every change.

## The prime directive: report, never assume

The tool may only state a compute lane it **watched run** in real logs. It must
never infer a lane from configuration, documentation, or a feature flag. When it
cannot tell, it says `indeterminate` and makes no claim. Concretely:

- A verdict must derive only from captured `matched_lines` (which ship in every
  report so the derivation is checkable).
- A failed or timed-out run claims **no** lane — it is `indeterminate`.
- The "stock risc0 → CPU" finding is **version-bound** (see `src/versions.rs`):
  keep it scoped to the tested trio and emit `indeterminate` for untested
  versions. Never let it read as a timeless claim.
- **Midnight** is CPU-only and is **not** risc0: it is Plonk + KZG over
  BLS12-381 (a classical SNARK), it does not use Metal on a Mac, and the data
  model (`src/target.rs`) keeps its Metal lane `NotApplicable`. Never add a
  Midnight Metal/GPU claim, and never imply Midnight uses risc0/STARKs/Pasta
  curves/post-quantum.

If a change would make the tool assert something it didn't observe, it doesn't
belong here.

## Dev workflow

```bash
cargo test                                   # unit tests
cargo clippy --all-targets -- -D warnings    # warnings are errors
cargo fmt --all -- --check                   # formatting
cargo deny check                             # supply-chain (advisories/licenses/sources)
```

All four are gated in CI ([.github/workflows/ci.yml](.github/workflows/ci.yml))
on an Apple-Silicon runner. Add a test for any new verdict logic — the scanner is
the part most likely to be wrong (see the regression tests in `src/lane.rs`).

## Adding a proving target

Targets live in `src/target.rs` as a `TargetDesc` with per-lane
`ValidationStatus`. Honesty is structural: a lane you have not observed firsthand
must **not** be `Validated`. Use `NotApplicable` (with a reason), `Unconfigured`,
`Remote`, or `Experimental` as appropriate. If a target needs a log parser,
calibrate it against **real, firsthand** output before shipping — do not hardcode
an assumed log schema.

## PR checklist

- [ ] `cargo test`, `clippy -D warnings`, `fmt --check`, `cargo deny check` all pass
- [ ] No new claim the tool can't observe firsthand
- [ ] CHANGELOG.md updated under `[Unreleased]`
- [ ] Any new finding is version-scoped and grounded with a citation
