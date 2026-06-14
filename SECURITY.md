# Security Policy

## Reporting a vulnerability

Please report security issues **privately**, not as a public issue.

- Preferred: open a private report via **GitHub Security Advisories** —
  the repository's **Security → Report a vulnerability** tab
  (GitHub Private Vulnerability Reporting). If that tab is not visible, open a
  regular issue asking the maintainer to enable private reporting (without
  vulnerability details) and a private channel will be arranged.

Please include: the version (`r0-metal-doctor --version`), your OS/arch, repro
steps, and impact. We aim to acknowledge within a few days and to coordinate a
fix and disclosure timeline with you; we follow responsible-disclosure norms
(typically up to 90 days) but will move faster for actively-exploited issues.

## Supported versions

This is pre-1.0 software; only the latest released `0.x` line receives security
fixes.

| Version | Supported |
|---------|-----------|
| 0.2.x   | ✅        |
| < 0.2   | ❌        |

## Important: `prove` executes the target project's code

The `prove` subcommand runs `cargo run --release` inside the project you point
`--project` at. **This compiles and executes that project's code (including its
`build.rs` and dependencies) with your user privileges. It is not sandboxed.**

Only run `r0-metal-doctor prove` against projects you trust, exactly as you
would only `cargo run` a repository you trust. The other subcommands
(`device`, `env`, `doctor`, `targets`, `midnight`, `check`, `bundle` without
`--project`) do **not** execute third-party code — they only read device
capabilities, environment variables, and run version probes.

Argument handling is safe by construction: the subprocess is spawned with an
argument vector via `std::process::Command` (no shell), so there is no shell
interpolation of `--args`.

## Sharing evidence safely

`bundle` (and `prove`/`check`) **redact** host-identifying details by default —
your home directory collapses to `~` and your username to `<user>` — so output
is safe to attach to a public bug report. `--no-redact` disables this for local
use; review such output before sharing.

## Supply-chain notes (for transparency)

- **`paste` (RUSTSEC-2024-0436, unmaintained)** is an accepted *transitive*
  advisory: `paste` is pulled in via the `metal` crate's `objc`/`foreign-types`
  chain and has no drop-in safe upgrade there yet. It is explicitly tracked in
  [`deny.toml`](deny.toml) and re-evaluated on a `metal`/`objc2` modernization.
- **`block 0.1.6`** triggers a Rust *future-incompatibility lint*
  (`static of uninhabited type`, rust-lang/rust#74840) — **not** a RustSec
  advisory, and invisible to `cargo audit`/`cargo deny`. It reaches us through
  the same `metal`/`objc` chain and is pending an upstream migration. It is a
  forward-compat warning, not a present security defect.
