---
name: Bug report
about: Report incorrect or surprising r0-metal-doctor output
title: ""
labels: bug
---

## What happened

<!-- What did the tool report, and what did you expect? -->

## Evidence bundle (please attach)

Run this and attach the resulting directory — it is **redacted by default**
(no home path or username), so it is safe to share:

```bash
r0-metal-doctor bundle --project path/to/your/risc0-host-crate
# (omit --project if the issue is not about a proof run)
```

Or paste the one-line summary:

```bash
r0-metal-doctor check
```

## Environment

- `r0-metal-doctor --version`:
- macOS version / chip (e.g. macOS 26, M4 Max):
- risc0 toolchain (`r0vm --version`, `cargo-risczero --version`):

## Notes

<!-- Anything else. If the verdict was `indeterminate`, the matched_lines in the
     bundle help us see why. -->
