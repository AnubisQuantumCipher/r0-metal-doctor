mod device;
mod envprobe;
mod prove;
mod report;

use clap::{Parser, Subcommand};

/// Diagnose whether RISC Zero proving on Apple Silicon actually uses Metal.
///
/// This tool reports what it observes. It never asserts a prover lane it
/// did not watch run.
#[derive(Parser)]
#[command(name = "r0-metal-doctor", version, about)]
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
    /// Observe a real proof run inside a risc0 project directory and report
    /// which prover lane the logs show (requires the risc0 toolchain)
    Prove {
        /// Path to a risc0 project (host crate) to run; defaults to cwd
        #[arg(long, default_value = ".")]
        project: String,
        /// Extra args passed to `cargo run` of the host crate
        #[arg(long)]
        args: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Device => {
            let d = device::probe();
            report::emit(&d, cli.json)
        }
        Command::Env => {
            let e = envprobe::probe();
            report::emit(&e, cli.json)
        }
        Command::Doctor => {
            let combined = report::Doctor {
                device: device::probe(),
                env: envprobe::probe(),
            };
            let notes = combined.notes();
            report::emit_doctor(&combined, &notes, cli.json)
        }
        Command::Prove { project, args } => {
            let outcome = prove::observe(&project, args.as_deref())?;
            report::emit(&outcome, cli.json)
        }
    }
}
