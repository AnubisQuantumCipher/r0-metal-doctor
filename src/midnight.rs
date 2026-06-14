//! Honest Midnight observer — a CPU-only proving target.
//!
//! Midnight does **not** and **cannot** prove on the Metal GPU on a Mac, on two
//! independent grounds: the stock proof server is CPU-only (Plonk + KZG over
//! BLS12-381; the only GPU fork is NVIDIA-CUDA-only), and it runs inside a Linux
//! Docker container that has no Metal passthrough on macOS. So this module never
//! claims a Metal lane. It reports only firsthand-checkable signals:
//! `compactc` presence, proof-server reachability (`/health`), env routing, and
//! container identity. It does **not** parse the proof server's verbose logs —
//! no `-v` schema has been verified, so that stays an explicit future slot.

use crate::sanitize::redact;
use crate::target::ValidationStatus;
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_millis(800);
const DEFAULT_PROVE_URL: &str = "http://127.0.0.1:6300/prove";

/// The stated, firsthand-grounded reason Midnight has no Metal lane on a Mac.
const METAL_NA_REASON: &str = "Structurally impossible on Apple Silicon: the stock Midnight proof server is CPU-only (Plonk + KZG over BLS12-381), the only GPU fork (Nocy) is NVIDIA-CUDA-only, and the server runs inside a Linux Docker container with no Metal passthrough.";

#[derive(Serialize, Debug)]
pub struct MidnightReport {
    pub target: &'static str,
    /// First-class stated finding — the headline, never a silent omission.
    pub finding: &'static str,
    pub metal_lane: &'static str,
    pub compute_lane: &'static str,
    pub proving_system: &'static str,
    pub compactc: ToolStatus,
    pub proof_server: ProofServerProbe,
    pub docker: DockerProbe,
    pub env_routing: Vec<EnvVar>,
    pub notes: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct ToolStatus {
    pub found: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub note: String,
}

#[derive(Serialize, Debug)]
pub struct ProofServerProbe {
    pub prove_url: String,
    pub url_source: String,
    pub host: String,
    pub port: u16,
    pub locality: String,
    pub tcp_listening: bool,
    pub health_status_line: Option<String>,
    /// Reachability status of the proof-server transport (NOT a proving or
    /// Metal claim): `Unconfigured` if nothing listens, `Remote` if reachable
    /// off-host (round-trip only), `Reachable` if a localhost port answered.
    pub status: ValidationStatus,
    pub summary: String,
}

#[derive(Serialize, Debug)]
pub struct DockerProbe {
    pub docker_available: bool,
    pub proof_server_containers: Vec<String>,
    pub note: String,
}

#[derive(Serialize, Debug)]
pub struct EnvVar {
    pub name: &'static str,
    pub set: bool,
    pub value: Option<String>,
}

pub fn probe() -> MidnightReport {
    let compactc = probe_compactc();
    let proof_server = probe_proof_server();
    let docker = probe_docker();
    let env_routing = probe_env();

    let mut notes = Vec::new();
    if compactc.found {
        notes.push(
            "compactc is installed, but it only COMPILES Compact to .zkir — it does not prove. Proving needs a running proof server.".into(),
        );
    }
    if proof_server.locality == "remote" {
        notes.push(
            "The prove URL is remote: only round-trip reachability is observable here — no local container, no logs, no proving run was watched.".into(),
        );
    }
    if !proof_server.tcp_listening {
        notes.push(
            "Nothing is listening on the prove URL — the Midnight proving lane is UNCONFIGURED on this machine, not ready.".into(),
        );
    }
    notes.push(
        "Midnight is a classical, trusted-setup pairing SNARK (NOT post-quantum, NOT a STARK). GPU proving for Midnight exists only as a third-party NVIDIA-CUDA fork on Linux — never on Apple Silicon.".into(),
    );
    notes.push(
        "This observer does not parse the proof server's verbose logs: no -v log schema has been verified firsthand, so any log-lane parser is left as an explicit experimental slot.".into(),
    );

    MidnightReport {
        target: "midnight",
        finding: "Midnight proving is CPU-only on Apple Silicon. Metal is not applicable.",
        metal_lane: METAL_NA_REASON,
        compute_lane: "cpu — Plonk/KZG over BLS12-381 runs on the CPU (blst MSM + rayon). Inferred from the architecture; this tool does not itself measure GPU utilization.",
        proving_system: "Plonk + KZG over BLS12-381 (JubJub embedded), halo2 lineage — classical, trusted-setup SNARK; not post-quantum, not a STARK/FRI system",
        compactc,
        proof_server,
        docker,
        env_routing,
        notes,
    }
}

fn probe_compactc() -> ToolStatus {
    // 1) on PATH
    if let Some(v) = crate::envprobe::version_of("compactc", &["--version"]) {
        return ToolStatus {
            found: true,
            version: Some(v),
            path: Some("compactc (on PATH)".into()),
            note: "compactc compiles Compact → .zkir; it does not prove.".into(),
        };
    }
    // 2) the rzup-style install layout: ~/.compact/versions/<ver>/<arch>-darwin/compactc
    if let Some(home) = std::env::var_os("HOME") {
        let versions = PathBuf::from(&home).join(".compact").join("versions");
        if let Ok(entries) = std::fs::read_dir(&versions) {
            let mut newest: Option<(String, PathBuf)> = None;
            for entry in entries.flatten() {
                let ver = entry.file_name().to_string_lossy().to_string();
                for arch in ["aarch64-darwin", "x86_64-darwin"] {
                    let bin = entry.path().join(arch).join("compactc");
                    if bin.exists() && newest.as_ref().map(|(v, _)| ver > *v).unwrap_or(true) {
                        newest = Some((ver.clone(), bin));
                    }
                }
            }
            if let Some((_, bin)) = newest {
                let version = crate::envprobe::version_of(&bin.to_string_lossy(), &["--version"]);
                return ToolStatus {
                    found: true,
                    version,
                    path: Some(redact(&bin.to_string_lossy())),
                    note: "compactc compiles Compact → .zkir; it does not prove.".into(),
                };
            }
        }
    }
    ToolStatus {
        found: false,
        version: None,
        path: None,
        note: "compactc not found on PATH or under ~/.compact/versions — install the Compact toolchain to compile circuits.".into(),
    }
}

fn probe_proof_server() -> ProofServerProbe {
    let (prove_url, url_source) = match std::env::var("ZKF_MIDNIGHT_PROOF_SERVER_PROVE_URL") {
        Ok(u) if !u.is_empty() => (u, "env: ZKF_MIDNIGHT_PROOF_SERVER_PROVE_URL".to_string()),
        _ => (
            DEFAULT_PROVE_URL.to_string(),
            "default (no ZKF_MIDNIGHT_PROOF_SERVER_PROVE_URL set)".to_string(),
        ),
    };
    let parsed = parse_authority(&prove_url);
    let locality = if is_local(&parsed.host) {
        "localhost"
    } else {
        "remote"
    };

    // A malformed `:port` in the URL is surfaced, not silently rebound to a
    // default — otherwise we'd probe and report a different port than stated.
    let port = match parsed.port {
        PortKind::Valid(p) => p,
        PortKind::Default(p) => p,
        PortKind::Malformed(ref raw) => {
            return ProofServerProbe {
                prove_url: redact(&prove_url),
                url_source,
                host: parsed.host.clone(),
                port: 0,
                locality: locality.to_string(),
                tcp_listening: false,
                health_status_line: None,
                status: ValidationStatus::Unconfigured,
                summary: format!("URL has a malformed port ('{raw}') — not probed"),
            };
        }
    };

    let listening = tcp_listening(&parsed.host, port);
    let health = if listening && parsed.scheme == "http" {
        http_status_line(&parsed.host, port)
    } else {
        None
    };

    let host = parsed.host;
    let (status, summary) = if !listening {
        (
            ValidationStatus::Unconfigured,
            format!("nothing listening on {host}:{port} — proving lane not set up here"),
        )
    } else if locality == "remote" {
        (
            ValidationStatus::Remote,
            format!("{host}:{port} reachable; round-trip only, not locally observed"),
        )
    } else {
        // Reachable, not Validated: the port answered, but that is transport
        // reachability — NOT a watched proving lane.
        let detail = match &health {
            Some(line) => format!("reachable on {host}:{port} (GET /health → {line})"),
            None => format!("reachable on {host}:{port} (no /health response)"),
        };
        (ValidationStatus::Reachable, detail)
    };

    ProofServerProbe {
        prove_url: redact(&prove_url),
        url_source,
        host,
        port,
        locality: locality.to_string(),
        tcp_listening: listening,
        health_status_line: health,
        status,
        summary,
    }
}

fn probe_docker() -> DockerProbe {
    let available = crate::envprobe::version_of("docker", &["--version"]).is_some();
    if !available {
        return DockerProbe {
            docker_available: false,
            proof_server_containers: Vec::new(),
            note: "docker not found — the Midnight proof server normally runs as a Linux container (midnightntwrk/proof-server).".into(),
        };
    }
    let mut containers = Vec::new();
    if let Ok(out) = Command::new("docker")
        .args(["ps", "--format", "{{.Image}} {{.Names}} {{.Status}}"])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let l = line.to_ascii_lowercase();
                if l.contains("proof-server") || l.contains("midnight") {
                    containers.push(line.trim().to_string());
                }
            }
        }
    }
    let note = if containers.is_empty() {
        "docker is available but no Midnight proof-server container is running. (A Linux container on macOS has no Metal access in any case.)".into()
    } else {
        "Proof-server container(s) found. They run Linux in Docker's VM — CPU-only, no Metal passthrough.".into()
    };
    DockerProbe {
        docker_available: true,
        proof_server_containers: containers,
        note,
    }
}

fn probe_env() -> Vec<EnvVar> {
    [
        "ZKF_MIDNIGHT_PROOF_SERVER_PROVE_URL",
        "ZKF_MIDNIGHT_PROOF_SERVER_VERIFY_URL",
        "ZKF_MIDNIGHT_ALLOW_COMPAT_DELEGATE",
    ]
    .into_iter()
    .map(|name| {
        let value = std::env::var(name).ok();
        EnvVar {
            name,
            set: value.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
            value: value.map(|v| redact(&v)),
        }
    })
    .collect()
}

// --- tiny std-only network helpers (no TLS, no url crate) ---

/// Where the port came from: an explicit valid port, the scheme default (no
/// port segment), or a present-but-unparseable port (surfaced, never silently
/// rebound to a default).
enum PortKind {
    Valid(u16),
    Default(u16),
    Malformed(String),
}

struct Authority {
    scheme: String,
    host: String,
    port: PortKind,
}

fn parse_authority(url: &str) -> Authority {
    let (scheme, rest) = url.split_once("://").unwrap_or(("http", url));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let default_port = if scheme == "https" { 443 } else { 80 };

    // IPv6 literal: `[::1]` or `[::1]:6300`.
    let (host, port_str) = if let Some(rest) = authority.strip_prefix('[') {
        match rest.split_once(']') {
            Some((h, after)) => (h.to_string(), after.strip_prefix(':')),
            None => (rest.to_string(), None), // unterminated bracket; treat all as host
        }
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) if !h.is_empty() => (h.to_string(), Some(p)),
            _ => (authority.to_string(), None),
        }
    };

    let port = match port_str {
        None | Some("") => PortKind::Default(default_port),
        Some(p) => match p.parse::<u16>() {
            Ok(n) => PortKind::Valid(n),
            Err(_) => PortKind::Malformed(p.to_string()),
        },
    };

    Authority {
        scheme: scheme.to_string(),
        host,
        port,
    }
}

fn is_local(host: &str) -> bool {
    matches!(
        host,
        "127.0.0.1" | "localhost" | "::1" | "[::1]" | "0.0.0.0"
    )
}

fn tcp_listening(host: &str, port: u16) -> bool {
    match (host, port).to_socket_addrs() {
        Ok(addrs) => addrs
            .into_iter()
            .any(|a| TcpStream::connect_timeout(&a, PROBE_TIMEOUT).is_ok()),
        Err(_) => false,
    }
}

fn http_status_line(host: &str, port: u16) -> Option<String> {
    use std::io::{Read, Write};
    let addr = (host, port).to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(PROBE_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(PROBE_TIMEOUT)).ok()?;
    let req = format!(
        "GET /health HTTP/1.1\r\nHost: {host}:{port}\r\nUser-Agent: r0-metal-doctor\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = [0u8; 256];
    let n = stream.read(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf[..n]);
    text.lines().next().map(|l| l.trim().to_string())
}

/// Human-readable rendering for the `midnight` subcommand.
pub fn render_human(r: &MidnightReport) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "midnight proving target");
    let _ = writeln!(s, "  finding             {}", r.finding);
    let _ = writeln!(s, "  metal lane          [N/A] {}", r.metal_lane);
    let _ = writeln!(s, "  compute lane        {}", r.compute_lane);
    let _ = writeln!(s, "  proving system      {}", r.proving_system);

    let c = &r.compactc;
    let _ = writeln!(
        s,
        "\ncompactc            {}",
        if c.found {
            format!(
                "{} ({})",
                c.version.as_deref().unwrap_or("present"),
                c.path.as_deref().unwrap_or("?")
            )
        } else {
            "not found".into()
        }
    );

    let p = &r.proof_server;
    let _ = writeln!(s, "\nproof server");
    let _ = writeln!(
        s,
        "  prove url           {}  ({})",
        p.prove_url, p.url_source
    );
    let _ = writeln!(s, "  locality            {}", p.locality);
    let _ = writeln!(
        s,
        "  status              {} {}",
        p.status.badge(),
        p.summary
    );

    let d = &r.docker;
    let _ = writeln!(
        s,
        "\ndocker              {}",
        if d.docker_available {
            "available"
        } else {
            "not found"
        }
    );
    for line in &d.proof_server_containers {
        let _ = writeln!(s, "  container           {line}");
    }
    let _ = writeln!(s, "  {}", d.note);

    let _ = writeln!(s, "\nenv routing");
    for e in &r.env_routing {
        let _ = writeln!(
            s,
            "  {:<42} {}",
            e.name,
            e.value.as_deref().unwrap_or("unset")
        );
    }

    let _ = writeln!(s, "\nnotes");
    for n in &r.notes {
        let _ = writeln!(s, "  - {n}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port_value(p: &PortKind) -> Option<u16> {
        match p {
            PortKind::Valid(n) | PortKind::Default(n) => Some(*n),
            PortKind::Malformed(_) => None,
        }
    }

    #[test]
    fn parses_default_localhost_url() {
        let a = parse_authority("http://127.0.0.1:6300/prove");
        assert_eq!(a.scheme, "http");
        assert_eq!(a.host, "127.0.0.1");
        assert_eq!(port_value(&a.port), Some(6300));
        assert!(is_local(&a.host));
    }

    #[test]
    fn parses_remote_https_url_without_explicit_port() {
        let a = parse_authority("https://prover.example.com/prove");
        assert_eq!(a.scheme, "https");
        assert_eq!(a.host, "prover.example.com");
        assert_eq!(port_value(&a.port), Some(443));
        assert!(!is_local(&a.host));
    }

    #[test]
    fn parses_ipv6_loopback_as_local() {
        let a = parse_authority("http://[::1]:6300/prove");
        assert_eq!(a.host, "::1");
        assert_eq!(port_value(&a.port), Some(6300));
        assert!(is_local(&a.host));
    }

    #[test]
    fn malformed_port_is_surfaced_not_rebound() {
        let a = parse_authority("https://example.com:99999/prove");
        assert!(matches!(a.port, PortKind::Malformed(_)));
    }

    #[test]
    fn report_never_offers_a_metal_lane() {
        let r = probe();
        // The metal lane is always the stated N/A reason — never a working lane.
        assert!(r.metal_lane.contains("Structurally impossible"));
        assert_eq!(r.compute_lane.split_whitespace().next(), Some("cpu"));
        // serialized form must not assert metal-observed anywhere
        let j = serde_json::to_string(&r).unwrap();
        assert!(!j.contains("metal-observed"));
    }
}
