//! Compute-lane vocabulary, observed-lane verdicts, and CI exit-code mapping.
//!
//! A "lane" is the compute backend a prover actually ran on (Metal GPU, CUDA
//! GPU, or CPU). The doctor only ever names a lane it observed in real logs —
//! see [`Verdict`], which keeps an honest `Indeterminate` state instead of
//! guessing, and [`LaneCounts::verdict`], which reports the *dominant* lane of a
//! mixed run rather than collapsing a genuine hybrid (mostly-GPU + a few CPU
//! kernel lines) into an uninformative "mixed".

use crate::sanitize::strip_ansi;
use serde::Serialize;
use std::process::ExitCode;

/// A compute lane a prover can execute on.
#[derive(clap::ValueEnum, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Lane {
    Metal,
    Cuda,
    Cpu,
}

impl Lane {
    pub fn as_str(self) -> &'static str {
        match self {
            Lane::Metal => "metal",
            Lane::Cuda => "cuda",
            Lane::Cpu => "cpu",
        }
    }
}

impl std::fmt::Display for Lane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Per-lane line counts gathered from a captured proof run.
#[derive(Serialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LaneCounts {
    pub metal: usize,
    pub cuda: usize,
    pub cpu: usize,
}

impl LaneCounts {
    pub fn total(self) -> usize {
        self.metal + self.cuda + self.cpu
    }

    /// Lanes with at least one observed line, ordered by count descending; ties
    /// keep Metal > Cuda > Cpu order (stable). Used to pick the dominant lane.
    fn ranked(self) -> Vec<(Lane, usize)> {
        let mut v = vec![
            (Lane::Metal, self.metal),
            (Lane::Cuda, self.cuda),
            (Lane::Cpu, self.cpu),
        ];
        v.retain(|(_, n)| *n > 0);
        // descending by count; stable sort preserves Metal>Cuda>Cpu on ties
        v.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        v
    }

    /// Derive a verdict from the counts. One lane → `Observed`; several →
    /// `Mixed` with the most-frequent lane as `dominant`; none → `Indeterminate`.
    pub fn verdict(self) -> Verdict {
        let ranked = self.ranked();
        match ranked.as_slice() {
            [] => Verdict::Indeterminate,
            [(lane, _)] => Verdict::Observed { lane: *lane },
            many => Verdict::Mixed {
                dominant: many[0].0,
                lanes: many.iter().map(|(l, _)| *l).collect(),
            },
        }
    }
}

/// What the doctor concluded about the prover lane. Derived only from observed
/// log lines; never inferred from configuration.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Verdict {
    /// Exactly one lane appeared in the logs.
    Observed { lane: Lane },
    /// Multiple lanes appeared. `dominant` ran the most lines; `lanes` lists all
    /// observed, most-frequent first. A genuine hybrid run (most lines on the
    /// GPU, a few circuit-kernel lines on the CPU) lands here *without* burying
    /// the dominant lane.
    Mixed { dominant: Lane, lanes: Vec<Lane> },
    /// No prover-lane lines were captured. No claim is made.
    Indeterminate,
}

impl Verdict {
    /// The lane to compare against `--expect` — the dominant one, if any.
    pub fn effective_lane(&self) -> Option<Lane> {
        match self {
            Verdict::Observed { lane } => Some(*lane),
            Verdict::Mixed { dominant, .. } => Some(*dominant),
            Verdict::Indeterminate => None,
        }
    }

    /// Backward-compatible label (`"metal-observed"`, `"indeterminate — …"`,
    /// `"mixed — …"`) for human and JSON continuity.
    pub fn label(&self) -> String {
        match self {
            Verdict::Observed { lane } => format!("{lane}-observed"),
            Verdict::Mixed { dominant, lanes } => {
                let others: Vec<&str> = lanes
                    .iter()
                    .filter(|l| **l != *dominant)
                    .map(|l| l.as_str())
                    .collect();
                format!("mixed — dominant {dominant} (also: {})", others.join(", "))
            }
            Verdict::Indeterminate => {
                "indeterminate — no prover-selection lines captured; rerun with RUST_LOG=debug"
                    .to_string()
            }
        }
    }
}

/// The doctor's own CI exit-code scheme, documented in `--help`:
/// `0` ok/matched · `1` lane mismatch · `2` indeterminate · `3` env/usage error.
/// `2` is distinct from `1` so a CI gate can tell *wrong lane* from *couldn't
/// tell* — preserving "report, never assume".
pub const EXIT_OK: u8 = 0;
pub const EXIT_MISMATCH: u8 = 1;
pub const EXIT_INDETERMINATE: u8 = 2;
pub const EXIT_ERROR: u8 = 3;

/// Map an observed verdict (and an optional `--expect`ed lane) to a process
/// exit code. With no expectation, any observed lane is success and only
/// `Indeterminate` is non-zero.
pub fn exit_code(verdict: &Verdict, expected: Option<Lane>) -> ExitCode {
    match (verdict.effective_lane(), expected) {
        (None, _) => ExitCode::from(EXIT_INDETERMINATE),
        (Some(_), None) => ExitCode::from(EXIT_OK),
        (Some(observed), Some(exp)) if observed == exp => ExitCode::from(EXIT_OK),
        (Some(_), Some(_)) => ExitCode::from(EXIT_MISMATCH),
    }
}

/// Per-lane markers. A captured line is attributed to a lane only if it carries
/// one of these — risc0's HAL **module path** (`…::hal::metal`) or an explicit
/// lane-selection phrase. This is deliberately stricter than "mentions the word
/// in a proving context", which would miscount two kinds of line:
/// - the RISC-V **executor**, which always runs on CPU regardless of the proving
///   HAL (`…::prove::executor: … cpu …` would otherwise inject a spurious CPU
///   lane and flip a clean Metal run to "mixed");
/// - a **file path** like `hal/metal.rs` (slash + `.rs`, never `hal::metal`).
const METAL_MARKERS: [&str; 3] = ["hal::metal", "metal hal", "metal prover"];
const CUDA_MARKERS: [&str; 3] = ["hal::cuda", "cuda hal", "cuda prover"];
const CPU_MARKERS: [&str; 4] = ["hal::cpu", "cpu hal", "cpu prover", "cpu fallback"];

/// Classify one (already lowercased) line by its HAL marker, if any. Metal takes
/// precedence, then CUDA, then CPU — a single risc0 module-path line names one
/// backend, so this never double-counts.
fn lane_of(lower: &str) -> Option<Lane> {
    if METAL_MARKERS.iter().any(|m| lower.contains(m)) {
        Some(Lane::Metal)
    } else if CUDA_MARKERS.iter().any(|m| lower.contains(m)) {
        Some(Lane::Cuda)
    } else if CPU_MARKERS.iter().any(|m| lower.contains(m)) {
        Some(Lane::Cpu)
    } else {
        None
    }
}

/// True for a cargo/rustc build line — a compiler diagnostic or status line.
/// With marker-based attribution this is mostly belt-and-suspenders, but it
/// still drops an `error:`/`warning:` line that happens to name a HAL path.
fn looks_like_build_output(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("-->")
        || t.starts_with("error")
        || t.starts_with("warning:")
        || t.starts_with("note:")
        || t.starts_with("Compiling")
        || t.starts_with("Checking")
        || t.starts_with("Building")
        || t.starts_with("Finished")
        || t.starts_with("Running")
}

/// Scan captured output for prover-lane evidence. Returns the per-lane counts and
/// the matched lines, ANSI-stripped and ordered metal→cuda→cpu, capped so a giant
/// log can't bloat the report.
pub fn scan(output: &str) -> (LaneCounts, Vec<String>) {
    let mut counts = LaneCounts::default();
    let (mut metal, mut cuda, mut cpu) = (Vec::new(), Vec::new(), Vec::new());

    for raw in output.lines() {
        let clean = strip_ansi(raw);
        if looks_like_build_output(&clean) {
            continue;
        }
        match lane_of(&clean.to_ascii_lowercase()) {
            Some(Lane::Metal) => {
                counts.metal += 1;
                metal.push(clean.trim().to_string());
            }
            Some(Lane::Cuda) => {
                counts.cuda += 1;
                cuda.push(clean.trim().to_string());
            }
            Some(Lane::Cpu) => {
                counts.cpu += 1;
                cpu.push(clean.trim().to_string());
            }
            None => {}
        }
    }

    let mut matched = Vec::new();
    matched.extend(metal);
    matched.extend(cuda);
    matched.extend(cpu);
    matched.truncate(60);
    (counts, matched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(s: &str) -> LaneCounts {
        scan(s).0
    }

    #[test]
    fn metal_line_yields_metal_verdict() {
        let (c, m) = scan("INFO risc0: using metal prover for segment\n");
        assert_eq!(c.verdict(), Verdict::Observed { lane: Lane::Metal });
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn cpu_line_yields_cpu_verdict() {
        let c = counts("DEBUG prove: falling back to cpu hal\n");
        assert_eq!(c.verdict(), Verdict::Observed { lane: Lane::Cpu });
    }

    #[test]
    fn lane_word_without_a_hal_marker_is_ignored() {
        let (c, m) = scan("Compiling metal v0.32.0\nFinished release target\n");
        assert_eq!(c.verdict(), Verdict::Indeterminate);
        assert!(m.is_empty());
    }

    #[test]
    fn ansi_is_stripped_before_matching_and_storing() {
        let line =
            "\u{1b}[2m..\u{1b}[0m DEBUG \u{1b}[2mrisc0_zkp::hal::metal\u{1b}[0m: io: 32768\n";
        let (c, m) = scan(line);
        assert_eq!(c.verdict(), Verdict::Observed { lane: Lane::Metal });
        assert!(!m[0].contains('\u{1b}'));
        assert!(m[0].contains("risc0_zkp::hal::metal"));
    }

    #[test]
    fn compiler_diagnostic_pointing_at_metal_rs_is_ignored() {
        // Regression: a build failure whose error points at
        // `risc0-zkp/src/hal/metal.rs:47` must NOT be read as metal-observed
        // (it is `hal/metal`, not the `hal::metal` module path).
        let diag = "  --> ~/.cargo/registry/src/index.crates.io-1949cf8c/risc0-zkp-3.0.4/src/hal/metal.rs:47:26\n";
        let (c, m) = scan(diag);
        assert_eq!(c.verdict(), Verdict::Indeterminate);
        assert!(m.is_empty());
    }

    #[test]
    fn real_tracing_lane_line_still_counts() {
        let line = "2026-06-13T20:59:57Z DEBUG risc0_circuit_rv32im::prove::hal::metal: witgen(metal-hybrid): 32768\n";
        let (c, _) = scan(line);
        assert_eq!(c.verdict(), Verdict::Observed { lane: Lane::Metal });
    }

    #[test]
    fn cpu_executor_line_does_not_taint_a_metal_run() {
        // Regression: the RISC-V executor always runs on CPU regardless of the
        // proving HAL. A `prove::executor: … cpu …` line must NOT inject a CPU
        // lane and flip a genuine all-Metal run to "mixed".
        let log = "\
2026 DEBUG risc0_zkp::prove::executor: cpu segments=4\n\
2026 DEBUG risc0_circuit_rv32im::prove::hal::metal: witgen(metal-hybrid): 32768\n\
2026 DEBUG risc0_zkp::hal::metal: io: 32768\n";
        let c = counts(log);
        assert_eq!(
            c,
            LaneCounts {
                metal: 2,
                cuda: 0,
                cpu: 0
            }
        );
        assert_eq!(c.verdict(), Verdict::Observed { lane: Lane::Metal });
    }

    #[test]
    fn lane_line_with_source_location_still_counts() {
        // With `tracing` configured `with_file(true)`, a real lane line embeds a
        // span location like `metal.rs:88:` — it must still count (the old
        // blanket `.rs:` filter wrongly dropped it).
        let line = "DEBUG risc0_zkp::hal::metal metal.rs:88: io: 32768\n";
        let (c, _) = scan(line);
        assert_eq!(c.verdict(), Verdict::Observed { lane: Lane::Metal });
    }

    #[test]
    fn dominant_lane_is_not_buried_by_a_minority_line() {
        // 3 metal lines + 1 cpu line = a hybrid run; dominant is metal, not "mixed → cpu".
        let log = "\
prover hal::metal witgen\n\
prover hal::metal accumulate\n\
prover hal::metal fri\n\
prover hal::cpu eval_check\n";
        let c = counts(log);
        assert_eq!(
            c,
            LaneCounts {
                metal: 3,
                cuda: 0,
                cpu: 1
            }
        );
        match c.verdict() {
            Verdict::Mixed {
                dominant,
                ref lanes,
            } => {
                assert_eq!(dominant, Lane::Metal);
                assert_eq!(lanes, &vec![Lane::Metal, Lane::Cpu]);
            }
            other => panic!("expected Mixed dominant metal, got {other:?}"),
        }
    }

    #[test]
    fn exit_codes_match_the_scheme() {
        let metal = Verdict::Observed { lane: Lane::Metal };
        let cpu = Verdict::Observed { lane: Lane::Cpu };
        let indet = Verdict::Indeterminate;

        // no expectation: any observed lane is success; indeterminate is 2
        assert_eq!(
            format!("{:?}", exit_code(&metal, None)),
            format!("{:?}", ExitCode::from(0))
        );
        assert_eq!(
            format!("{:?}", exit_code(&indet, None)),
            format!("{:?}", ExitCode::from(2))
        );
        // expect metal: metal ok, cpu mismatch, indeterminate still 2
        assert_eq!(
            format!("{:?}", exit_code(&metal, Some(Lane::Metal))),
            format!("{:?}", ExitCode::from(0))
        );
        assert_eq!(
            format!("{:?}", exit_code(&cpu, Some(Lane::Metal))),
            format!("{:?}", ExitCode::from(1))
        );
        assert_eq!(
            format!("{:?}", exit_code(&indet, Some(Lane::Metal))),
            format!("{:?}", ExitCode::from(2))
        );
    }

    #[test]
    fn expect_matches_the_dominant_lane_of_a_mixed_run() {
        // Documented policy: --expect compares against the DOMINANT lane of a
        // hybrid run (see the --expect help text). Pinned so the semantics are
        // explicit and a future strict mode is a deliberate change.
        let mixed = Verdict::Mixed {
            dominant: Lane::Metal,
            lanes: vec![Lane::Metal, Lane::Cpu],
        };
        assert_eq!(
            format!("{:?}", exit_code(&mixed, Some(Lane::Metal))),
            format!("{:?}", ExitCode::from(0)) // dominant metal matches
        );
        assert_eq!(
            format!("{:?}", exit_code(&mixed, Some(Lane::Cpu))),
            format!("{:?}", ExitCode::from(1)) // dominant is metal, not cpu
        );
    }
}
