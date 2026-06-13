//! Proving-target registry — the structural honesty layer.
//!
//! The doctor observes more than one proving system. Each [`TargetDesc`] lists
//! the compute lanes that system *could* use on Apple Silicon, and tags each
//! with a [`ValidationStatus`]. Honesty lives in the data, not in prose: a
//! target whose Metal lane is [`ValidationStatus::NotApplicable`] literally has
//! no `Validated` Metal entry, so no output path can render it as
//! Metal-accelerated.
//!
//! - **risc0** is the validated target: its Metal and CPU lanes have both been
//!   observed running in real logs on this hardware (deep observation:
//!   `r0-metal-doctor prove`).
//! - **Midnight** is a CPU-only target: its proving runs in a Linux Docker
//!   container (Plonk + KZG over BLS12-381) that cannot reach Metal on a Mac.
//!   Its Metal lane is `NotApplicable`, stated with the reason (deep
//!   observation: `r0-metal-doctor midnight`).

use serde::Serialize;

/// How thoroughly — and whether — a lane is real for a target on this platform.
/// Rendered as an explicit badge, never a bare checkmark that conflates "works"
/// with "validated here".
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    /// Watched it run in real logs on this hardware.
    Validated,
    /// Structurally impossible on this platform — stated with a reason.
    NotApplicable,
    /// Possible, but not set up on this machine right now.
    Unconfigured,
    /// A local endpoint observed reachable (e.g. a proof-server port answered) —
    /// transport only, NOT a proving or lane observation.
    Reachable,
    /// Observed only via network round-trip; not locally watched.
    Remote,
    /// Community / extensible slot, unverified here.
    Experimental,
}

impl ValidationStatus {
    pub fn badge(self) -> &'static str {
        match self {
            ValidationStatus::Validated => "[VALIDATED]",
            ValidationStatus::NotApplicable => "[N/A]",
            ValidationStatus::Unconfigured => "[UNCONFIGURED]",
            ValidationStatus::Reachable => "[REACHABLE]",
            ValidationStatus::Remote => "[REMOTE]",
            ValidationStatus::Experimental => "[EXPERIMENTAL]",
        }
    }
}

/// One compute lane of a target, with its validation status and a plain-English
/// note. For `NotApplicable` lanes the note carries the *reason*.
#[derive(Serialize, Debug, Clone)]
pub struct LaneDesc {
    pub name: &'static str,
    pub status: ValidationStatus,
    pub note: &'static str,
}

/// A proving system the doctor knows how to observe.
#[derive(Serialize, Debug, Clone)]
pub struct TargetDesc {
    pub id: &'static str,
    pub display: &'static str,
    /// One-line honesty headline — never stronger than the target's best lane.
    pub headline: &'static str,
    /// The command that performs deep, firsthand observation of this target.
    pub observer: &'static str,
    pub lanes: &'static [LaneDesc],
}

const RISC0: TargetDesc = TargetDesc {
    id: "risc0",
    display: "RISC Zero zkVM (rv32im segment prover)",
    headline: "Validated lane observer — Metal and CPU both observed firsthand.",
    observer: "r0-metal-doctor prove --project <risc0-host-crate>",
    lanes: &[
        LaneDesc {
            name: "metal",
            status: ValidationStatus::Validated,
            note: "Observable firsthand as `risc0_zkp::hal::metal` on a real proof run (seen via the risc0-metal-hybrid sibling). NOTE: stock risc0 3.0.5 / rv32im 4.0.4 ships no reachable Metal branch and proves on CPU — see FINDINGS.md.",
        },
        LaneDesc {
            name: "cpu",
            status: ValidationStatus::Validated,
            note: "Observed firsthand as `risc0_…::hal::cpu` — the stock risc0 3.0.5 lane on Apple Silicon.",
        },
        LaneDesc {
            name: "cuda",
            status: ValidationStatus::NotApplicable,
            note: "CUDA needs an NVIDIA GPU; unavailable on Apple Silicon by construction.",
        },
    ],
};

const MIDNIGHT: TargetDesc = TargetDesc {
    id: "midnight",
    display: "Midnight proof server (Compact; Plonk + KZG over BLS12-381)",
    headline: "CPU-only — Metal is not applicable on Apple Silicon.",
    observer: "r0-metal-doctor midnight",
    lanes: &[
        LaneDesc {
            name: "metal",
            status: ValidationStatus::NotApplicable,
            note: "Structurally impossible on a Mac: the stock proof server is CPU-only (Plonk + KZG over BLS12-381), the only GPU fork (Nocy) is NVIDIA-CUDA-only, and the server runs inside a Linux Docker container with no Metal passthrough.",
        },
        LaneDesc {
            name: "cpu",
            status: ValidationStatus::Unconfigured,
            note: "Midnight proves on the CPU (BLS12-381 Plonk/KZG; `blst` MSM + rayon). This is the structural lane, but this tool has not watched a Midnight proof run — bring up the proof server to observe one. It does not measure GPU utilization.",
        },
        LaneDesc {
            name: "cuda",
            status: ValidationStatus::Experimental,
            note: "A third-party CUDA fork (Nocy) exists for NVIDIA hardware on Linux; never available on Apple Silicon. Reported only as ecosystem context — no Metal/GPU lane here.",
        },
    ],
};

static TARGETS: &[TargetDesc] = &[RISC0, MIDNIGHT];

/// All proving targets the doctor knows about.
pub fn all() -> &'static [TargetDesc] {
    TARGETS
}

/// Look up a target by id (`"risc0"`, `"midnight"`).
pub fn by_id(id: &str) -> Option<&'static TargetDesc> {
    TARGETS.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midnight_has_no_validated_metal_lane() {
        // The core honesty invariant: the data model cannot render Midnight as
        // Metal-accelerated, because its only `metal` lane is NotApplicable.
        let midnight = by_id("midnight").expect("midnight target exists");
        let metal = midnight
            .lanes
            .iter()
            .find(|l| l.name == "metal")
            .expect("metal lane listed");
        assert_eq!(metal.status, ValidationStatus::NotApplicable);
        assert!(
            !midnight
                .lanes
                .iter()
                .any(|l| l.name == "metal" && l.status == ValidationStatus::Validated),
            "Midnight must never have a Validated Metal lane"
        );
    }

    #[test]
    fn risc0_metal_lane_is_validated() {
        let risc0 = by_id("risc0").expect("risc0 target exists");
        let metal = risc0.lanes.iter().find(|l| l.name == "metal").unwrap();
        assert_eq!(metal.status, ValidationStatus::Validated);
    }

    #[test]
    fn unknown_target_is_none() {
        assert!(by_id("snark-machine").is_none());
    }
}
