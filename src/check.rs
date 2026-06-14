//! `check` — a one-shot, paste-ready health summary.
//!
//! Runs the universal probes (Metal device + host env) and, when a `--project`
//! is given, observes one risc0 proof run, then prints a single compact line
//! suitable for dropping into a bug report or CI log:
//!
//! ```text
//! r0-metal-doctor 0.2.0 | Apple M4 Max (apple9, unified) | risc0 3.0.5 | lane: cpu-observed (debug) | DEV_MODE off
//! ```

use crate::device::{self, DeviceReport};
use crate::envprobe::{self, EnvReport};
use crate::lane::Lane;
use crate::prove::{self, ProveOpts, ProveReport};
use crate::versions;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct CheckReport {
    pub tool: String,
    pub device: String,
    pub risc0: String,
    pub dev_mode: String,
    pub lane: String,
    pub lane_lines: usize,
    pub observed_lane: Option<Lane>,
    pub timed_out: bool,
    pub summary_line: String,
}

pub struct CheckOpts {
    pub project: Option<String>,
    pub timeout_secs: u64,
    pub redact: bool,
}

pub fn tool_string() -> String {
    format!("r0-metal-doctor {}", env!("CARGO_PKG_VERSION"))
}

fn device_string(dev: &DeviceReport) -> String {
    if !dev.metal_available {
        return "no Metal device".to_string();
    }
    let fam = dev.apple_gpu_family.as_deref().unwrap_or("?");
    let mem = if dev.unified_memory == Some(true) {
        "unified"
    } else {
        "discrete"
    };
    format!(
        "{} ({fam}, {mem})",
        dev.device_name.as_deref().unwrap_or("Metal device")
    )
}

fn risc0_string(env: &EnvReport) -> String {
    match versions::classify(env.r0vm.as_deref()).detected_version {
        Some(v) => format!("risc0 {v}"),
        None => "risc0 toolchain not installed".to_string(),
    }
}

fn dev_mode_string(env: &EnvReport) -> String {
    match env.risc0_dev_mode.as_deref() {
        Some(v) if matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on") => {
            "ON (fake receipts!)".to_string()
        }
        _ => "off".to_string(),
    }
}

fn lane_string(prove: Option<&ProveReport>) -> String {
    match prove {
        Some(r) if r.timed_out => "timed out".to_string(),
        Some(r) => format!("{} ({})", r.verdict_short(), r.rust_log),
        None => "not observed (pass --project <crate>)".to_string(),
    }
}

/// Build the compact one-line summary from already-collected reports. Shared by
/// `check` and `bundle` so proving never runs twice.
pub fn summary_line(dev: &DeviceReport, env: &EnvReport, prove: Option<&ProveReport>) -> String {
    format!(
        "{} | {} | {} | lane: {} | DEV_MODE {}",
        tool_string(),
        device_string(dev),
        risc0_string(env),
        lane_string(prove),
        dev_mode_string(env)
    )
}

pub fn run(opts: &CheckOpts) -> anyhow::Result<CheckReport> {
    let dev = device::probe();
    let env = envprobe::probe();

    let prove_report = match &opts.project {
        Some(project) => Some(prove::observe(
            project,
            None,
            &ProveOpts {
                rust_log: "debug".to_string(),
                timeout_secs: opts.timeout_secs,
                redact: opts.redact,
            },
        )?),
        None => None,
    };
    let prove_ref = prove_report.as_ref();

    Ok(CheckReport {
        tool: tool_string(),
        device: device_string(&dev),
        risc0: risc0_string(&env),
        dev_mode: dev_mode_string(&env),
        lane: lane_string(prove_ref),
        lane_lines: prove_ref.map(|r| r.lane_counts.total()).unwrap_or(0),
        observed_lane: prove_ref.and_then(|r| r.observed_lane),
        timed_out: prove_ref.map(|r| r.timed_out).unwrap_or(false),
        summary_line: summary_line(&dev, &env, prove_ref),
    })
}
