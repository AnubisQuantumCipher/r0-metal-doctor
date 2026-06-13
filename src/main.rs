mod bundle;
mod check;
mod device;
mod envprobe;
mod lane;
mod midnight;
mod prove;
mod report;
mod sanitize;
mod target;
mod versions;

use clap::{CommandFactory, Parser, Subcommand};
use lane::Lane;
use std::process::ExitCode;

/// Diagnose whether ZK proving on Apple Silicon actually uses the Metal GPU.
///
/// A report-never-assume doctor for ZK proving on Apple Silicon. It carries a
/// validated Metal-lane observer for RISC Zero (risc0), and an honest CPU-only
/// view of Midnight. It never asserts a prover lane it did not watch run.
///
/// EXIT CODES (this tool's own scheme, for CI gating):
///   0  ok / observed lane matched --expect
///   1  lane mismatch (observed a lane other than --expect)
///   2  indeterminate (could not tell — distinct from a mismatch)
///   3  environment or usage error
#[derive(Parser)]
#[command(name = "r0-metal-doctor", version, about, long_about = None)]
struct Cli {
    /// Emit machine-readable JSON instead of human-readable text
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Probe the Metal GPU: device name, unified memory, working-set limits
    Device,
    /// Probe the host environment: rust + risc0 toolchain, env vars that steer prover selection
    Env,
    /// Run device + env probes and print one combined report with notes
    Doctor,
    /// List the proving targets the doctor knows about and each one's honesty badge
    Targets {
        /// Show a single target by id (`risc0` or `midnight`); omit to list all
        target: Option<String>,
    },
    /// Observe the Midnight proving target: CPU-only, Metal not applicable —
    /// compactc, proof-server reachability, container identity, env routing
    Midnight,
    /// One-shot, paste-ready health summary (device + env, plus a proof
    /// observation if --project is given)
    Check {
        /// Optionally observe a risc0 proof run in this host crate too
        #[arg(long)]
        project: Option<String>,
        /// Wall-clock budget (seconds) for the optional proof run
        #[arg(long, default_value_t = 600)]
        timeout_secs: u64,
        /// Keep host paths/username (default: redacted)
        #[arg(long)]
        no_redact: bool,
    },
    /// Write a timestamped, redacted evidence directory for sharing in a bug report
    Bundle {
        /// Parent directory to write the bundle into (default: cwd)
        #[arg(long)]
        out: Option<String>,
        /// Optionally include a risc0 proof observation from this host crate
        #[arg(long)]
        project: Option<String>,
        /// Wall-clock budget (seconds) for the optional proof run
        #[arg(long, default_value_t = 600)]
        timeout_secs: u64,
        /// Keep host paths/username (default: redacted for safe sharing)
        #[arg(long)]
        no_redact: bool,
    },
    /// Observe a real risc0 proof run inside a host crate and report which
    /// prover lane the logs show (requires the risc0 toolchain)
    Prove {
        /// Path to a risc0 project (host crate) to run; defaults to cwd
        #[arg(long, default_value = ".")]
        project: String,
        /// Extra args passed to `cargo run` of the host crate
        #[arg(long)]
        args: Option<String>,
        /// Assert the observed lane: exit 0 if it matches, 1 if it differs,
        /// 2 if indeterminate. Turns the diagnostic into a CI gate. A hybrid
        /// (mixed) run matches on its dominant lane.
        #[arg(long, value_enum)]
        expect: Option<Lane>,
        /// Wall-clock budget (seconds) before the run is killed and the verdict
        /// fails closed to indeterminate
        #[arg(long, default_value_t = 600)]
        timeout_secs: u64,
        /// RUST_LOG for the child (default `debug`; `info` emits no lane lines)
        #[arg(long, default_value = "debug")]
        rust_log: String,
        /// Keep host paths/username in the report (default: redacted for safe sharing)
        #[arg(long)]
        no_redact: bool,
    },
    /// Generate shell completions (bash, zsh, fish, …) to stdout
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Render a man page (roff) to stdout
    Man,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(lane::EXIT_ERROR)
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Command::Device => {
            report::emit_device(&device::probe(), cli.json)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Env => {
            report::emit_env(&envprobe::probe(), cli.json)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor => {
            let combined = report::Doctor {
                device: device::probe(),
                env: envprobe::probe(),
            };
            let notes = combined.notes();
            report::emit_doctor(&combined, &notes, cli.json)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Targets { target } => match target {
            None => {
                report::emit_targets(target::all(), cli.json)?;
                Ok(ExitCode::SUCCESS)
            }
            Some(id) => match target::by_id(&id) {
                Some(t) => {
                    report::emit_targets(std::slice::from_ref(t), cli.json)?;
                    Ok(ExitCode::SUCCESS)
                }
                None => {
                    let known: Vec<&str> = target::all().iter().map(|t| t.id).collect();
                    eprintln!("error: unknown target '{id}'. known: {}", known.join(", "));
                    Ok(ExitCode::from(lane::EXIT_ERROR))
                }
            },
        },
        Command::Midnight => {
            let r = midnight::probe();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                print!("{}", midnight::render_human(&r));
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Check {
            project,
            timeout_secs,
            no_redact,
        } => {
            let report = check::run(&check::CheckOpts {
                project,
                timeout_secs,
                redact: !no_redact,
            })?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report.summary_line);
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Bundle {
            out,
            project,
            timeout_secs,
            no_redact,
        } => {
            let path = bundle::create(&bundle::BundleOpts {
                out_dir: out,
                project,
                timeout_secs,
                redact: !no_redact,
            })?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "bundle": path.display().to_string() })
                );
            } else {
                println!("wrote evidence bundle to {}", path.display());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
            Ok(ExitCode::SUCCESS)
        }
        Command::Man => {
            clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Prove {
            project,
            args,
            expect,
            timeout_secs,
            rust_log,
            no_redact,
        } => {
            let opts = prove::ProveOpts {
                rust_log,
                timeout_secs,
                redact: !no_redact,
            };
            let outcome = prove::observe(&project, args.as_deref(), &opts)?;
            report::emit_prove(&outcome, cli.json)?;
            Ok(outcome.exit(expect))
        }
    }
}
