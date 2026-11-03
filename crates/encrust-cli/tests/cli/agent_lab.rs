//! `agent-lab`: the autopilot scored on a model, and a client that does nothing left with no
//! score rather than counted.

use std::process::Command;

use core_geometry::{Vec3, cuboid};
use serde_json::Value;

use crate::support::*;

#[test]
fn the_autopilot_is_scored_and_a_client_that_starts_no_job_is_not() {
    let model = write_mesh_stl("agent-lab-cube", &cuboid(Vec3::ZERO, Vec3::splat(20.0)));
    let work = output_dir("agent-lab-work");
    let run = work.join("run.json");
    let encrust = env!("CARGO_BIN_EXE_encrust");
    let panel = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/project-panel.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_agent-lab"))
        .arg(&model)
        .arg("--printer")
        .arg(&panel)
        .arg("--resin")
        .arg(test_resin())
        .args([
            "--agent",
            "idle=true",
            "--encrust",
            encrust,
            "--encrust-mcp",
            encrust,
        ])
        .arg("--work")
        .arg(&work)
        .arg("--json")
        .arg(&run)
        .output()
        .expect("agent-lab starts");
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        stderr(&output)
    );

    let kept: Value = serde_json::from_str(&std::fs::read_to_string(&run).expect("kept"))
        .expect("the run is JSON");
    let cube = &kept["models"][0];
    assert_eq!(cube["autopilot"]["score"]["status"], "ok", "{cube}");
    let idle = &cube["agents"][0];
    assert_eq!(idle["agent"], "idle");
    assert_eq!(idle["tool_calls"], 0);
    assert_eq!(idle["prepared"]["why_not"], "started no job");
    assert!(idle["judged"].is_null(), "nothing to judge: {idle}");
}
