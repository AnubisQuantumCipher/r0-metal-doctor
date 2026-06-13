# Releasing r0-metal-doctor

This project versions with [SemVer](https://semver.org) and keeps a
[CHANGELOG.md](CHANGELOG.md). Releases produce prebuilt macOS binaries via
[cargo-dist](https://opensource.axo.dev/cargo-dist/) (the `dist` tool).

> These commands are **operator actions** — publishing to crates.io and pushing
> a release tag are not run by automation on your behalf.

## One-time setup (generates the release workflow)

cargo-dist's config and `release.yml` should be **generated**, not hand-written,
because option names shift between dist versions. Run once:

```bash
cargo install cargo-dist            # or: cargo install dist
dist init --yes                     # writes .github/workflows/release.yml + [workspace.metadata.dist]
git add .github/workflows/release.yml Cargo.toml dist-workspace.toml 2>/dev/null
git commit -m "ci: add cargo-dist release workflow"
```

Recommended `dist init` answers: target `aarch64-apple-darwin` (and optionally
`x86_64-apple-darwin`); installers: shell, optionally `homebrew`; CI: GitHub.

## Cutting a release

```bash
# 1. bump the version and move the CHANGELOG [Unreleased] section to the new tag
#    edit Cargo.toml `version` and CHANGELOG.md
# 2. commit, then tag and push — dist builds the archives + GitHub Release
git commit -am "release: v0.2.0"
git tag v0.2.0
git push --follow-tags
```

## crates.io

The `Cargo.toml` metadata is publish-ready (`description`, `license`,
`repository`, `readme`, `keywords`, `categories`):

```bash
cargo publish --dry-run   # verify packaging
cargo publish             # operator action
```

## Install channels (live only after the first tagged release)

- **Shell installer** — the script attached to the GitHub Release by dist.
- **Homebrew** — `brew install AnubisQuantumCipher/tap/r0-metal-doctor`, if a
  `homebrew-tap` repo + `HOMEBREW_TAP_TOKEN` are configured and the homebrew
  installer is enabled in `dist init`.
- **cargo-binstall** — `cargo binstall r0-metal-doctor` works once Releases
  exist (dist's artifact naming is binstall-compatible). Verify with
  `cargo binstall --dry-run` after the first release; do not advertise it before.

## Honesty about signing

cargo-dist ships **unsigned / ad-hoc** macOS binaries by default. Do **not**
describe them as "notarized" or "signed" unless an Apple Developer ID certificate
and `notarytool` credentials are actually wired into the release workflow.

For unsigned binaries, document the Gatekeeper bypass for users:

```bash
xattr -d com.apple.quarantine ./r0-metal-doctor
```

To ship genuinely notarized binaries, provision a Developer ID Application
certificate and notarytool credentials and configure dist's macOS signing —
then, and only then, update the docs to say "notarized".
