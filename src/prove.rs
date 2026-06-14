//! Observe a real risc0 proof run and report the prover lane the logs show.
//!
//! The run is streamed live (so a multi-minute proof shows progress instead of
//! looking hung) under a wall-clock timeout (so a stuck target can never hang
//! the doctor). The verdict is derived only from the lane lines actually
//! captured — see [`crate::lane`] — and on timeout it fails closed to
//! `indeterminate` rather than guessing.

use crate::lane::{self, Lane, LaneCounts};
use crate::sanitize::redact;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

/// Options controlling how a proof run is observed.
pub struct ProveOpts {
    /// `RUST_LOG` value for the child. Defaults to `debug` — risc0 emits no
    /// lane-selection lines at `info`, so `info` would always be indeterminate.
    pub rust_log: String,
    /// Wall-clock budget. On expiry the child is killed and the verdict fails
    /// closed to indeterminate.
    pub timeout_secs: u64,
    /// Collapse host-identifying paths/username in the stored report so it is
    /// safe to paste into a public bug report.
    pub redact: bool,
}

impl Default for ProveOpts {
    fn default() -> Self {
        ProveOpts {
            rust_log: "debug".to_string(),
            timeout_secs: 600,
            redact: true,
        }
    }
}

/// Outcome of observing one real proof run. The verdict derives only from the
/// matched lines (included verbatim, ANSI-stripped) so a reader can check it.
#[derive(Serialize, Debug)]
pub struct ProveReport {
    pub target: &'static str,
    pub project: String,
    pub command: String,
    pub rust_log: String,
    pub risc0_dev_mode: String,
    pub r0vm_version: Option<String>,
    pub cargo_risczero_version: Option<String>,
    pub risc0_matrix: crate::versions::Classification,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub timeout_secs: u64,
    pub wall_seconds: f64,
    pub verdict: String,
    pub observed_lane: Option<Lane>,
    pub lane_counts: LaneCounts,
    pub matched_lines: Vec<String>,
    pub note: String,
}

impl ProveReport {
    /// Compact verdict for one-line summaries (`metal-observed`, `mixed→metal`,
    /// `cpu-observed`, `indeterminate`).
    pub fn verdict_short(&self) -> String {
        if self.timed_out {
            return "indeterminate(timeout)".to_string();
        }
        if self.verdict.starts_with("mixed") {
            return match self.observed_lane {
                Some(lane) => format!("mixed→{lane}"),
                None => "mixed".to_string(),
            };
        }
        match self.observed_lane {
            Some(lane) => format!("{lane}-observed"),
            None => "indeterminate".to_string(),
        }
    }

    /// Process exit code for this report given an optional `--expect`ed lane.
    /// A timed-out run is indeterminate, never a mismatch.
    pub fn exit(&self, expected: Option<Lane>) -> ExitCode {
        if self.timed_out {
            return ExitCode::from(lane::EXIT_INDETERMINATE);
        }
        let verdict = match self.observed_lane {
            Some(lane) => lane::Verdict::Observed { lane },
            None => lane::Verdict::Indeterminate,
        };
        lane::exit_code(&verdict, expected)
    }
}

/// Read a child stream line by line, echoing each line live to our stderr (so
/// the user sees progress) and collecting it for the scan. Reads raw bytes and
/// decodes lossily per line, so a single non-UTF-8 chunk (a progress spinner, a
/// stray byte) can never terminate capture and drop later lane lines.
fn drain<R: Read + Send + 'static>(stream: R) -> Vec<String> {
    let mut reader = BufReader::new(stream);
    let mut lines = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break, // EOF
            Ok(_) => {
                while matches!(buf.last(), Some(b'\n' | b'\r')) {
                    buf.pop();
                }
                let line = String::from_utf8_lossy(&buf).into_owned();
                eprintln!("{line}");
                lines.push(line);
            }
            Err(_) => break,
        }
    }
    lines
}

pub fn observe(project: &str, extra_args: Option<&str>, opts: &ProveOpts) -> Result<ProveReport> {
    let dir = Path::new(project);
    if !dir.join("Cargo.toml").exists() {
        bail!(
            "{} has no Cargo.toml — point --project at a risc0 host crate",
            dir.display()
        );
    }

    // Capture the observed toolchain versions so the report self-dates against
    // the version matrix (the "unreachable" finding is version-bound).
    let env = crate::envprobe::probe();

    // Force real proving: a dev-mode run produces fake receipts and proves
    // nothing about lanes.
    let dev_mode = "0".to_string();

    let mut cmd = Command::new("cargo");
    cmd.arg("run").arg("--release");
    if let Some(extra) = extra_args {
        cmd.arg("--");
        cmd.args(extra.split_whitespace());
    }
    cmd.current_dir(dir)
        .env("RUST_LOG", &opts.rust_log)
        .env("RISC0_DEV_MODE", &dev_mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let shown = format!(
        "cargo run --release{}",
        extra_args.map(|a| format!(" -- {a}")).unwrap_or_default()
    );

    let started = Instant::now();
    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn `{shown}` in {}", dir.display()))?;

    // Two reader threads (one per pipe) avoid the classic deadlock where a full
    // stderr pipe blocks the child while we drain only stdout.
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let t_out = thread::spawn(move || drain(stdout));
    let t_err = thread::spawn(move || drain(stderr));

    let timeout = Duration::from_secs(opts.timeout_secs);
    let (status_code, timed_out) = match child.wait_timeout(timeout)? {
        Some(status) => (status.code(), false),
        None => {
            // Budget exhausted — kill and reap so the reader threads see EOF.
            let _ = child.kill();
            let _ = child.wait();
            (None, true)
        }
    };
    let wall = started.elapsed().as_secs_f64();

    let mut lines = t_out.join().unwrap_or_default();
    lines.extend(t_err.join().unwrap_or_default());
    let combined = lines.join("\n");

    let (lane_counts, mut matched_lines) = lane::scan(&combined);

    let raw_verdict = lane_counts.verdict();
    let succeeded = status_code == Some(0);
    let (verdict, observed_lane) = if timed_out {
        (
            format!(
                "indeterminate — timed out after {}s before the run completed; see lane_counts/matched_lines",
                opts.timeout_secs
            ),
            None,
        )
    } else if !succeeded {
        // A failed run (e.g. a build error) did not prove anything; any lane
        // lines it emitted are unreliable, so we make no lane claim.
        let how = status_code
            .map(|c| format!("with code {c}"))
            .unwrap_or_else(|| "via a signal".to_string());
        (
            format!(
                "indeterminate — the proof run exited {how} (no successful proof observed); no lane is claimed"
            ),
            None,
        )
    } else {
        (raw_verdict.label(), raw_verdict.effective_lane())
    };

    let mut project_str = dir.display().to_string();
    if opts.redact {
        project_str = redact(&project_str);
        for line in &mut matched_lines {
            *line = redact(line);
        }
    }

    Ok(ProveReport {
        target: "risc0",
        project: project_str,
        command: shown,
        rust_log: opts.rust_log.clone(),
        risc0_dev_mode: dev_mode,
        risc0_matrix: crate::versions::classify(env.r0vm.as_deref()),
        r0vm_version: env.r0vm,
        cargo_risczero_version: env.cargo_risczero,
        exit_code: status_code,
        timed_out,
        timeout_secs: opts.timeout_secs,
        wall_seconds: wall,
        verdict,
        observed_lane,
        lane_counts,
        matched_lines,
        note: "verdict derives only from the matched_lines above; if it says indeterminate, no claim is made"
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_project_errors_cleanly() {
        let e = observe("/definitely/not/a/project", None, &ProveOpts::default()).unwrap_err();
        assert!(e.to_string().contains("no Cargo.toml"));
    }
}
