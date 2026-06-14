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
    /// Typed misconfiguration checks with remediation advice.
    pub checks: Vec<EnvCheck>,
}

impl EnvReport {
    /// A copy with host-identifying details redacted from the free-form value
    /// fields. `RISC0_PROVER` and `RUST_LOG` are arbitrary user strings that
    /// commonly hold paths/usernames; version strings and observations are
    /// redacted too. Used for shareable artifacts (the evidence `bundle`).
    pub fn redacted(&self) -> EnvReport {
        let r = |o: &Option<String>| o.as_deref().map(crate::sanitize::redact);
        EnvReport {
            os: self.os.clone(),
            arch: self.arch.clone(),
            rustc: r(&self.rustc),
            cargo: r(&self.cargo),
            rzup: r(&self.rzup),
            cargo_risczero: r(&self.cargo_risczero),
            r0vm: r(&self.r0vm),
            risc0_dev_mode: r(&self.risc0_dev_mode),
            risc0_prover: r(&self.risc0_prover),
            rust_log: r(&self.rust_log),
            observations: self
                .observations
                .iter()
                .map(|o| crate::sanitize::redact(o))
                .collect(),
            checks: self
                .checks
                .iter()
                .map(|c| EnvCheck {
                    level: c.level,
                    message: crate::sanitize::redact(&c.message),
                    remediation: c.remediation.as_deref().map(crate::sanitize::redact),
                })
                .collect(),
        }
    }
}

/// A single environment check: a severity, what was found, and how to fix it.
#[derive(Serialize, Debug, Clone)]
pub struct EnvCheck {
    /// `ok` · `info` · `warn`
    pub level: &'static str,
    pub message: String,
    pub remediation: Option<String>,
}

impl EnvCheck {
    fn warn(message: impl Into<String>, remediation: impl Into<String>) -> Self {
        EnvCheck {
            level: "warn",
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }
    fn info(message: impl Into<String>, remediation: impl Into<String>) -> Self {
        EnvCheck {
            level: "info",
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }
    fn ok(message: impl Into<String>) -> Self {
        EnvCheck {
            level: "ok",
            message: message.into(),
            remediation: None,
        }
    }
}

fn build_checks(
    rzup: &Option<String>,
    cargo_risczero: &Option<String>,
    r0vm: &Option<String>,
    dev_mode: &Option<String>,
    prover: &Option<String>,
) -> Vec<EnvCheck> {
    let mut checks = Vec::new();

    if let Some(v) = dev_mode {
        if matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on") {
            checks.push(EnvCheck::warn(
                "RISC0_DEV_MODE is enabled — receipts are FAKE and no real proving runs",
                "unset RISC0_DEV_MODE before any lane question is meaningful",
            ));
        }
    }
    if let Some(p) = prover {
        checks.push(EnvCheck::info(
            format!("RISC0_PROVER is set ({p}) — it overrides default prover selection"),
            "unset RISC0_PROVER to observe the default selection path",
        ));
    }
    let toolchain_present = cargo_risczero.is_some() || r0vm.is_some();
    if !toolchain_present {
        checks.push(EnvCheck::warn(
            "risc0 toolchain not found (no cargo-risczero or r0vm on PATH)",
            "install via rzup: https://dev.risczero.com/api/zkvm/install",
        ));
    } else if rzup.is_some() && r0vm.is_none() {
        checks.push(EnvCheck::warn(
            "rzup is installed but r0vm is not on PATH — possible toolchain skew",
            "run `rzup install` and ensure ~/.risc0/bin is on PATH",
        ));
    }
    if checks.iter().all(|c| c.level != "warn") {
        checks.insert(0, EnvCheck::ok("no blocking misconfiguration detected"));
    }
    checks
}

pub(crate) fn version_of(bin: &str, args: &[&str]) -> Option<String> {
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

    let checks = build_checks(
        &rzup,
        &cargo_risczero,
        &r0vm,
        &risc0_dev_mode,
        &risc0_prover,
    );

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
        checks,
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

    #[test]
    fn redacted_scrubs_home_from_value_fields() {
        // RISC0_PROVER / RUST_LOG are arbitrary user strings that can hold home
        // paths; the redacted copy (used by `bundle`) must scrub them.
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/tester".into());
        let mut r = probe();
        r.risc0_prover = Some(format!("{home}/secret/prover"));
        r.rust_log = Some(format!("debug,{home}/trace"));
        // the home path also reaches the derived observations and checks
        r.observations.push(format!(
            "RISC0_PROVER={home}/secret/prover overrides selection"
        ));
        r.checks.push(EnvCheck::info(
            format!("RISC0_PROVER is set ({home}/secret)"),
            "unset RISC0_PROVER",
        ));
        let red = r.redacted();
        let j = serde_json::to_string(&red).expect("serialize");
        assert!(
            !j.contains(&home),
            "redacted env.json still contains $HOME: {j}"
        );
        assert!(red.risc0_prover.as_deref().unwrap().starts_with('~'));
    }
}
