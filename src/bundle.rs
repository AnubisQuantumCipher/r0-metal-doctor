//! `bundle` — write a timestamped, redacted evidence directory.
//!
//! Formalizes the hand-built `evidence/` folder used for risc0 bug reports: one
//! command writes device facts, host env, the proving-target registry, the
//! Midnight observation, and (with `--project`) a risc0 lane observation, plus a
//! one-line summary. Redacted by default — home paths and username are stripped
//! — so the directory is safe to attach to a public issue.

use crate::{check, device, envprobe, midnight, prove, target};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct BundleOpts {
    pub out_dir: Option<String>,
    pub project: Option<String>,
    pub timeout_secs: u64,
    pub redact: bool,
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let s = serde_json::to_string_pretty(value)?;
    fs::write(path, s).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn create(opts: &BundleOpts) -> Result<PathBuf> {
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = opts
        .out_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(format!("r0-metal-doctor-bundle-{epoch}"));
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let dev = device::probe();
    let env = envprobe::probe();
    // Drop the stable per-device registry id from the shareable bundle
    // (precautionary — it is a persistent machine identifier).
    let dev_out = if opts.redact {
        let mut d = dev.clone();
        d.registry_id = None;
        d
    } else {
        dev.clone()
    };
    write_json(&dir.join("device.json"), &dev_out)?;
    if opts.redact {
        write_json(&dir.join("env.json"), &env.redacted())?;
    } else {
        write_json(&dir.join("env.json"), &env)?;
    }
    write_json(&dir.join("targets.json"), &target::all())?;
    write_json(&dir.join("midnight.json"), &midnight::probe())?;

    let prove_report = match &opts.project {
        Some(project) => {
            let r = prove::observe(
                project,
                None,
                &prove::ProveOpts {
                    rust_log: "debug".to_string(),
                    timeout_secs: opts.timeout_secs,
                    redact: opts.redact,
                },
            )?;
            write_json(&dir.join("prove.json"), &r)?;
            fs::write(dir.join("prove-matched.log"), r.matched_lines.join("\n"))?;
            Some(r)
        }
        None => None,
    };

    let summary = check::summary_line(&dev, &env, prove_report.as_ref());
    fs::write(dir.join("summary.txt"), format!("{summary}\n"))?;

    let redaction_note = if opts.redact {
        "All files in this bundle are REDACTED (home paths -> ~, username -> <user>). Safe to attach to a public bug report. Re-run with --no-redact for an unredacted local copy."
    } else {
        "WARNING: generated with --no-redact — may contain your home path and username. Review before sharing publicly."
    };

    let readme = format!(
        "r0-metal-doctor evidence bundle\n\n{summary}\n\n{redaction_note}\n\nFiles:\n  \
         device.json        Metal GPU device facts\n  \
         env.json           host toolchain + env vars\n  \
         targets.json       proving targets + per-lane honesty badges\n  \
         midnight.json      Midnight target observation (CPU-only, Metal N/A)\n  \
         prove.json         risc0 lane observation (present if --project was given)\n  \
         prove-matched.log  the matched lane lines, verbatim (ANSI-stripped)\n  \
         summary.txt        the one-line summary\n"
    );
    fs::write(dir.join("README.txt"), readme)?;

    Ok(dir)
}
