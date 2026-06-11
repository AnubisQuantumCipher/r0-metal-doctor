use crate::device::DeviceReport;
use crate::envprobe::EnvReport;
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
        notes
    }
}

pub fn emit<T: Serialize + std::fmt::Debug>(value: &T, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{value:#?}");
    }
    Ok(())
}

pub fn emit_doctor(doctor: &Doctor, notes: &[String], json: bool) -> anyhow::Result<()> {
    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            device: &'a DeviceReport,
            env: &'a EnvReport,
            notes: &'a [String],
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                device: &doctor.device,
                env: &doctor.env,
                notes,
            })?
        );
    } else {
        println!("{:#?}", doctor.device);
        println!("{:#?}", doctor.env);
        println!("\nnotes:");
        for n in notes {
            println!("  - {n}");
        }
    }
    Ok(())
}
