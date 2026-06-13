use crate::device::DeviceReport;
use crate::envprobe::EnvReport;
use crate::target::TargetDesc;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct Doctor {
    pub device: DeviceReport,
    pub env: EnvReport,
}

impl Doctor {
    /// Cross-probe observations. Facts in, facts out — the strongest thing
    /// this returns is "run `prove` to observe the lane", because only a
    /// watched run proves a lane.
    pub fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();

        match (self.device.metal_available, &self.device.unified_memory) {
            (true, Some(true)) => notes.push(format!(
                "Metal device present with unified memory ({}) — the hardware side of GPU proving is available",
                self.device.device_name.as_deref().unwrap_or("unknown")
            )),
            (true, _) => notes.push("Metal device present".into()),
            (false, _) => {
                notes.push("no Metal device — any GPU-proving question ends here".into())
            }
        }

        let has_toolchain = self.env.cargo_risczero.is_some() || self.env.r0vm.is_some();
        if has_toolchain {
            notes.push(
                "risc0 toolchain detected — run `r0-metal-doctor prove --project <host-crate>` to observe which prover lane actually executes; device capability alone proves nothing about lane selection"
                    .into(),
            );
        } else {
            notes.push(
                "risc0 toolchain not detected — install (rzup) and rerun; until a proof is observed, no lane claim can be made"
                    .into(),
            );
        }

        // env observations, minus the toolchain-missing line the block above
        // already covers
        notes.extend(
            self.env
                .observations
                .iter()
                .filter(|o| !o.starts_with("risc0 toolchain not found"))
                .cloned(),
        );

        // Version-scoped lane context — keeps the "Metal unreachable" finding
        // honest as risc0 evolves (see src/versions.rs).
        notes.push(crate::versions::classify(self.env.r0vm.as_deref()).finding);

        notes
    }
}

fn print_device_human(d: &DeviceReport) {
    println!("metal device");
    if d.metal_available {
        println!(
            "  name                {}",
            d.device_name.as_deref().unwrap_or("?")
        );
        println!(
            "  unified memory      {}",
            d.unified_memory.map_or("?".into(), |b| b.to_string())
        );
        if let Some(b) = d.recommended_max_working_set_bytes {
            println!("  working set (rec.)  {:.1} GB", b as f64 / 1e9);
        }
        if let Some(b) = d.max_buffer_length_bytes {
            println!("  max buffer          {:.1} GB", b as f64 / 1e9);
        }
        if let Some(f) = &d.apple_gpu_family {
            println!("  gpu family          {f}");
        }
    } else {
        println!("  none — {}", d.note.as_deref().unwrap_or("not available"));
    }
}

fn print_env_human(e: &EnvReport) {
    println!("host environment ({} / {})", e.os, e.arch);
    for (label, val) in [
        ("rustc", &e.rustc),
        ("cargo", &e.cargo),
        ("rzup", &e.rzup),
        ("cargo-risczero", &e.cargo_risczero),
        ("r0vm", &e.r0vm),
    ] {
        println!("  {label:<18}{}", val.as_deref().unwrap_or("not found"));
    }
    for (label, val) in [
        ("RISC0_DEV_MODE", &e.risc0_dev_mode),
        ("RISC0_PROVER", &e.risc0_prover),
        ("RUST_LOG", &e.rust_log),
    ] {
        println!("  {label:<18}{}", val.as_deref().unwrap_or("unset"));
    }
    if !e.checks.is_empty() {
        println!("\nchecks");
        for c in &e.checks {
            println!("  [{}] {}", c.level, c.message);
            if let Some(rem) = &c.remediation {
                println!("        → {rem}");
            }
        }
    }
}

pub fn emit_device(d: &DeviceReport, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(d)?);
    } else {
        print_device_human(d);
    }
    Ok(())
}

pub fn emit_env(e: &EnvReport, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(e)?);
    } else {
        print_env_human(e);
    }
    Ok(())
}

pub fn emit_prove(r: &crate::prove::ProveReport, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(r)?);
        return Ok(());
    }
    println!("risc0 lane observation");
    println!("  project             {}", r.project);
    println!(
        "  command             RUST_LOG={} RISC0_DEV_MODE=0 {}",
        r.rust_log, r.command
    );
    if let Some(v) = &r.r0vm_version {
        println!("  r0vm                {v}");
    }
    println!(
        "  exit code           {}",
        r.exit_code.map_or("—".into(), |c| c.to_string())
    );
    print!("  wall time           {:.1}s", r.wall_seconds);
    if r.timed_out {
        print!(" (TIMED OUT at {}s)", r.timeout_secs);
    }
    println!();
    println!(
        "  lanes seen          metal={} cuda={} cpu={}",
        r.lane_counts.metal, r.lane_counts.cuda, r.lane_counts.cpu
    );
    println!("  verdict             {}", r.verdict);
    println!("  risc0 finding       {}", r.risc0_matrix.finding);
    if !r.matched_lines.is_empty() {
        println!("\n  matched lines (the verdict derives only from these):");
        for line in &r.matched_lines {
            println!("    {line}");
        }
    }
    println!("\n  {}", r.note);
    Ok(())
}

/// Render the proving-target registry: each target's honesty headline and its
/// per-lane validation badges. The badges are the structural honesty layer —
/// a `[N/A]` Metal lane can never read as a working one.
pub fn emit_targets(targets: &[TargetDesc], json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(targets)?);
        return Ok(());
    }
    println!("proving targets ({} known)\n", targets.len());
    for t in targets {
        println!("{}  [{}]", t.display, t.id);
        println!("  {}", t.headline);
        for lane in t.lanes {
            println!(
                "    {:<14} {:<5} {}",
                lane.status.badge(),
                lane.name,
                lane.note
            );
        }
        println!("  observe: {}\n", t.observer);
    }
    println!(
        "Badges: [VALIDATED] watched it run here · [N/A] structurally impossible (reason given)"
    );
    println!(
        "        [UNCONFIGURED] possible, not set up · [REACHABLE] port answered (transport only)"
    );
    println!("        [REMOTE] round-trip only · [EXPERIMENTAL] unverified here");
    Ok(())
}

pub fn emit_doctor(doctor: &Doctor, notes: &[String], json: bool) -> anyhow::Result<()> {
    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            device: &'a DeviceReport,
            env: &'a EnvReport,
            notes: &'a [String],
            risc0_version_matrix: &'static [crate::versions::MatrixRow],
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                device: &doctor.device,
                env: &doctor.env,
                notes,
                risc0_version_matrix: crate::versions::matrix(),
            })?
        );
    } else {
        print_device_human(&doctor.device);
        println!();
        print_env_human(&doctor.env);
        println!("\nnotes");
        for n in notes {
            println!("  - {n}");
        }
    }
    Ok(())
}
