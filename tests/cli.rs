//! End-to-end tests that exercise the actual built binary, not just internals.
//! These catch CLI-wiring regressions (arg parsing, exit codes, JSON shape)
//! that unit tests can't see.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_r0-metal-doctor"))
}

fn json_of(args: &[&str]) -> serde_json::Value {
    let out = bin().args(args).output().expect("spawn");
    assert!(
        out.status.success(),
        "{args:?} exited {:?}",
        out.status.code()
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{args:?} bad json: {e}"))
}

#[test]
fn device_json_is_valid() {
    let v = json_of(&["device", "--json"]);
    assert!(v.get("metal_available").is_some());
}

#[test]
fn env_json_is_valid() {
    let v = json_of(&["env", "--json"]);
    assert!(v.get("os").is_some());
    assert!(v.get("checks").is_some());
}

#[test]
fn doctor_json_carries_version_matrix() {
    let v = json_of(&["doctor", "--json"]);
    assert!(v.get("risc0_version_matrix").is_some());
}

#[test]
fn targets_json_lists_both_targets() {
    let v = json_of(&["targets", "--json"]);
    let arr = v.as_array().expect("array");
    let ids: Vec<&str> = arr.iter().filter_map(|t| t["id"].as_str()).collect();
    assert!(ids.contains(&"risc0"));
    assert!(ids.contains(&"midnight"));
}

#[test]
fn midnight_json_never_offers_a_validated_metal_lane() {
    // End-to-end honesty check: the serialized Midnight report must not present
    // Metal as a working/validated lane.
    let v = json_of(&["midnight", "--json"]);
    let s = v.to_string();
    assert!(!s.contains("metal-observed"));
    assert!(v["metal_lane"]
        .as_str()
        .unwrap()
        .contains("Structurally impossible"));
    // structural check against the registry, too
    let targets = json_of(&["targets", "midnight", "--json"]);
    let midnight = &targets.as_array().unwrap()[0];
    let metal = midnight["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "metal")
        .unwrap();
    assert_eq!(metal["status"], "not_applicable");
}

#[test]
fn check_runs_without_project() {
    let out = bin().args(["check"]).output().expect("spawn");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("r0-metal-doctor"));
}

#[test]
fn unknown_target_exits_3() {
    let out = bin()
        .args(["targets", "definitely-not-a-target"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn prove_on_missing_project_exits_3() {
    let out = bin()
        .args(["prove", "--project", "/definitely/not/a/dir"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn completions_and_man_render() {
    for args in [["completions", "bash"], ["completions", "zsh"]] {
        let out = bin().args(args).output().expect("spawn");
        assert!(out.status.success() && !out.stdout.is_empty());
    }
    let man = bin().arg("man").output().expect("spawn");
    assert!(man.status.success() && !man.stdout.is_empty());
}
