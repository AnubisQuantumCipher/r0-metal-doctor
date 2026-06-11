use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// Outcome of observing one real proof run. The verdict is derived only from
/// log lines actually captured during this run; the matched lines are included
/// verbatim so the reader can check the derivation.
#[derive(Serialize, Debug)]
pub struct ProveReport {
    pub project: String,
    pub command: String,
    pub rust_log: String,
    pub risc0_dev_mode: String,
    pub exit_code: Option<i32>,
    pub wall_seconds: f64,
    pub verdict: String,
    pub matched_lines: Vec<String>,
    pub note: String,
}

/// Scan captured output for prover-lane evidence. A line counts only if it
/// mentions a lane keyword AND a proving-context keyword, to avoid matching
/// e.g. a crate name in the build output.
pub fn scan_lanes(output: &str) -> (String, Vec<String>) {
    const CONTEXT: [&str; 5] = ["prover", "prove", "proving", "hal", "receipt"];
    let mut metal = Vec::new();
    let mut cuda = Vec::new();
    let mut cpu = Vec::new();

    for line in output.lines() {
        let l = line.to_ascii_lowercase();
        if !CONTEXT.iter().any(|c| l.contains(c)) {
            continue;
        }
        if l.contains("metal") {
            metal.push(line.trim().to_string());
        } else if l.contains("cuda") {
            cuda.push(line.trim().to_string());
        } else if l.contains("cpu") {
            cpu.push(line.trim().to_string());
        }
    }

    let mut matched: Vec<String> = Vec::new();
    matched.extend(metal.iter().cloned());
    matched.extend(cuda.iter().cloned());
    matched.extend(cpu.iter().cloned());
    matched.truncate(40);

    let verdict = match (!metal.is_empty(), !cuda.is_empty(), !cpu.is_empty()) {
        (true, false, false) => "metal-observed",
        (false, true, false) => "cuda-observed",
        (false, false, true) => "cpu-observed",
        (false, false, false) => {
            "indeterminate — no prover-selection lines captured; rerun with RUST_LOG=debug"
        }
        _ => "mixed — multiple lanes mentioned; read matched_lines and judge",
    };
    (verdict.to_string(), matched)
}

pub fn observe(project: &str, extra_args: Option<&str>) -> Result<ProveReport> {
    let dir = Path::new(project);
    if !dir.join("Cargo.toml").exists() {
        bail!(
            "{} has no Cargo.toml — point --project at a risc0 host crate",
            dir.display()
        );
    }

    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    // Force real proving: a dev-mode run proves nothing about lanes.
    let dev_mode = "0".to_string();

    let mut cmd = Command::new("cargo");
    cmd.arg("run").arg("--release");
    if let Some(extra) = extra_args {
        cmd.arg("--");
        cmd.args(extra.split_whitespace());
    }
    cmd.current_dir(dir)
        .env("RUST_LOG", &rust_log)
        .env("RISC0_DEV_MODE", &dev_mode);

    let shown = format!("cargo run --release{}", extra_args.map(|a| format!(" -- {a}")).unwrap_or_default());
    let started = Instant::now();
    let out = cmd
        .output()
        .with_context(|| format!("failed to spawn `{shown}` in {}", dir.display()))?;
    let wall = started.elapsed().as_secs_f64();

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let (verdict, matched_lines) = scan_lanes(&combined);

    Ok(ProveReport {
        project: dir.display().to_string(),
        command: shown,
        rust_log,
        risc0_dev_mode: dev_mode,
        exit_code: out.status.code(),
        wall_seconds: wall,
        verdict,
        matched_lines,
        note: "verdict derives only from the matched_lines above; if it says indeterminate, no claim is made"
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metal_line_yields_metal_verdict() {
        let (v, m) = scan_lanes("INFO risc0: using metal prover for segment\n");
        assert_eq!(v, "metal-observed");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn cpu_line_yields_cpu_verdict() {
        let (v, _) = scan_lanes("DEBUG prove: falling back to cpu hal\n");
        assert_eq!(v, "cpu-observed");
    }

    #[test]
    fn lane_word_without_context_is_ignored() {
        let (v, m) = scan_lanes("Compiling metal v0.32.0\nFinished release target\n");
        assert!(v.starts_with("indeterminate"));
        assert!(m.is_empty());
    }

    #[test]
    fn mixed_lanes_are_reported_as_mixed() {
        let (v, m) =
            scan_lanes("INFO prover: metal enabled\nWARN prover: cpu fallback engaged\n");
        assert!(v.starts_with("mixed"));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn missing_project_errors_cleanly() {
        let e = observe("/definitely/not/a/project", None).unwrap_err();
        assert!(e.to_string().contains("no Cargo.toml"));
    }
}
