use serde::Serialize;
use std::process::Command;

/// Host toolchain and environment facts that steer risc0 prover selection.
/// Facts only — the `verdict` strings name what was observed, not what the
/// docs promise.
#[derive(Serialize, Debug)]
pub struct EnvReport {
    pub os: String,
    pub arch: String,
    pub rustc: Option<String>,
    pub cargo: Option<String>,
    pub rzup: Option<String>,
    pub cargo_risczero: Option<String>,
    pub r0vm: Option<String>,
    /// RISC0_DEV_MODE: when truthy, risc0 produces fake receipts and no real
    /// proving (CPU or GPU) happens at all.
    pub risc0_dev_mode: Option<String>,
    /// RISC0_PROVER: explicit prover override (e.g. "local", "ipc", "bonsai").
    pub risc0_prover: Option<String>,
    pub rust_log: Option<String>,
    pub observations: Vec<String>,
}

fn version_of(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

pub fn probe() -> EnvReport {
    let rustc = version_of("rustc", &["--version"]);
    let cargo = version_of("cargo", &["--version"]);
    let rzup = version_of("rzup", &["--version"]);
    let cargo_risczero = version_of("cargo-risczero", &["--version"])
        .or_else(|| version_of("cargo", &["risczero", "--version"]));
    let r0vm = version_of("r0vm", &["--version"]);

    let risc0_dev_mode = std::env::var("RISC0_DEV_MODE").ok();
    let risc0_prover = std::env::var("RISC0_PROVER").ok();
    let rust_log = std::env::var("RUST_LOG").ok();

    let mut observations = Vec::new();

    if cargo_risczero.is_none() && r0vm.is_none() && rzup.is_none() {
        observations.push(
            "risc0 toolchain not found (no rzup, cargo-risczero, or r0vm on PATH) — install via rzup before the `prove` probe can observe anything".into(),
        );
    }
    if let Some(v) = &risc0_dev_mode {
        if matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on") {
            observations.push(
                "RISC0_DEV_MODE is enabled: receipts are fake and NO real proving runs — any Metal-vs-CPU question is moot until this is unset".into(),
            );
        }
    }
    if let Some(p) = &risc0_prover {
        observations.push(format!(
            "RISC0_PROVER={p} explicitly overrides prover selection — the observed lane will reflect this, not the default path"
        ));
    }
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        observations.push(
            "host is Apple Silicon macOS — CUDA is unavailable here by construction; the only GPU lane risc0 could use is Metal".into(),
        );
    }

    EnvReport {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        rustc,
        cargo,
        rzup,
        cargo_risczero,
        r0vm,
        risc0_dev_mode,
        risc0_prover,
        rust_log,
        observations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_of_returns_none_for_missing_binary() {
        assert!(version_of("definitely-not-a-real-binary-xyz", &["--version"]).is_none());
    }

    #[test]
    fn probe_reports_current_platform() {
        let r = probe();
        assert_eq!(r.os, std::env::consts::OS);
        assert_eq!(r.arch, std::env::consts::ARCH);
    }

    #[test]
    fn report_serializes_to_json() {
        let r = probe();
        let j = serde_json::to_string(&r).expect("serialize");
        assert!(j.contains("\"os\""));
        assert!(j.contains("\"observations\""));
    }
}
