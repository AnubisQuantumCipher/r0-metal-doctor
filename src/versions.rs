//! The risc0 version matrix — what keeps the "Metal is unreachable" finding
//! honest as risc0 evolves.
//!
//! The CPU-fallback finding is **version-bound**: it is established firsthand
//! for the risc0-zkvm 3.0.5 / risc0-zkp 3.0.4 / rv32im 4.0.4 trio, where the
//! rv32im segment prover has no reachable Metal branch. It is *not* a timeless
//! claim:
//! - upstream PR #3688 (merged to `main` 2026-01-30, unreleased) adds a real
//!   Metal lane — the sentinel that will eventually falsify it;
//! - 5.0.0-rc.1 is a different ("m3") architecture where rv32im moved to a C++
//!   sys HAL, so this tool's file/line evidence does not apply.
//!
//! [`classify`] maps an observed `r0vm`/`cargo-risczero` version to a matrix
//! row, and returns an explicit *indeterminate* for any version not tested
//! here — it never auto-flips to "Metal works".

use serde::Serialize;

/// The doctor's conclusion about a given risc0 version, derived from the matrix.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub detected_version: Option<String>,
    /// Stable, scoped statement of the lane finding for this version — or an
    /// honest "not in the tested matrix" when the version is unknown.
    pub finding: String,
    /// Whether this version is one the tool has firsthand evidence for.
    pub in_tested_matrix: bool,
}

/// One row of the tested-against matrix (for `--help`/docs/JSON surfacing).
#[derive(Serialize, Debug, Clone)]
pub struct MatrixRow {
    pub versions: &'static str,
    pub channel: &'static str,
    pub finding: &'static str,
}

static MATRIX: &[MatrixRow] = &[
    MatrixRow {
        versions: "risc0-zkvm 3.0.5 / risc0-zkp 3.0.4 / risc0-circuit-rv32im 4.0.4",
        channel: "stable (the doctor's pin)",
        finding: "rv32im has no reachable Metal branch in any feature configuration; proving falls back to CPU. The `metal` feature is inert (maps only to `prove`); the prebuilt r0vm is CPU-only. Established firsthand on Apple Silicon.",
    },
    MatrixRow {
        versions: "risc0-zkvm 5.0.0-rc.1 (and the 5.x line)",
        channel: "prerelease",
        finding: "Different (\"m3\") architecture — rv32im moved to a C++ sys HAL. This tool's 3.0.x/4.0.x file-and-line evidence does NOT apply; re-verify by observation before making any lane claim.",
    },
    MatrixRow {
        versions: "upstream main, PR #3688 (merged 2026-01-30)",
        channel: "unreleased",
        finding: "Adds a real Metal lane for rv32im + recursion. In no tagged release yet. This is the sentinel that will eventually make the CPU-fallback finding false — observe the lane rather than trusting this note.",
    },
];

/// All known matrix rows, for surfacing in docs/JSON.
pub fn matrix() -> &'static [MatrixRow] {
    MATRIX
}

/// Extract a dotted version (e.g. `3.0.5`) from a tool version string like
/// `risc0-r0vm 3.0.5` or `cargo-risczero 3.0.5 (abcdef0 2026-02-03)`.
fn extract_semver(s: &str) -> Option<String> {
    for tok in s.split_whitespace() {
        let core = tok.trim_start_matches('v');
        let mut parts = core.split('.');
        let (a, b) = (parts.next(), parts.next());
        if let (Some(a), Some(b)) = (a, b) {
            if a.chars().all(|c| c.is_ascii_digit())
                && !a.is_empty()
                && b.chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
            {
                return Some(core.to_string());
            }
        }
    }
    None
}

/// Classify an observed risc0 version string against the matrix.
pub fn classify(version_string: Option<&str>) -> Classification {
    let detected = version_string.and_then(extract_semver);
    let finding = match detected.as_deref() {
        Some(v) if v.starts_with("3.0.") => {
            "risc0 3.0.x: rv32im has no reachable Metal branch; proving runs on CPU on Apple Silicon (firsthand-established for 3.0.5/3.0.4/4.0.4).".to_string()
        }
        Some(v) if v.starts_with("5.") => {
            format!("risc0 {v}: the 5.x line is a different (m3) architecture (rv32im moved to a C++ sys HAL). This tool's 3.0.x evidence does not apply; observe the run rather than trusting the version.")
        }
        Some(v) => {
            format!("risc0 {v}: not in the tested matrix — no Metal/CPU claim is made from the version; run `prove` to observe the lane.")
        }
        None => {
            "risc0 version not detected — install the toolchain (rzup) and run `prove` to observe the lane.".to_string()
        }
    };
    let in_tested_matrix = detected
        .as_deref()
        .map(|v| v.starts_with("3.0."))
        .unwrap_or(false);
    Classification {
        detected_version: detected,
        finding,
        in_tested_matrix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_version_from_tool_string() {
        assert_eq!(
            extract_semver("risc0-r0vm 3.0.5"),
            Some("3.0.5".to_string())
        );
        assert_eq!(
            extract_semver("cargo-risczero 3.0.5 (abc 2026-02-03)"),
            Some("3.0.5".to_string())
        );
        assert_eq!(extract_semver("no version here"), None);
    }

    #[test]
    fn pinned_version_is_in_matrix() {
        let c = classify(Some("risc0-r0vm 3.0.5"));
        assert!(c.in_tested_matrix);
        assert!(c.finding.contains("CPU"));
    }

    #[test]
    fn unknown_version_makes_no_claim() {
        let c = classify(Some("risc0-r0vm 9.9.9"));
        assert!(!c.in_tested_matrix);
        assert!(c.finding.contains("not in the tested matrix"));
    }

    #[test]
    fn five_x_flags_architecture_change() {
        let c = classify(Some("risc0-r0vm 5.0.0-rc.1"));
        assert!(!c.in_tested_matrix);
        assert!(c.finding.contains("m3") || c.finding.contains("architecture"));
    }
}
