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

/// An executable shell script `body` in `dir`.
#[cfg(unix)]
fn script(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).expect("the temp dir is writable");
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\nset -e\n{body}\n")).expect("writable");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[cfg(unix)]
#[test]
fn both_preparations_are_drawn_and_a_judge_of_its_own_rules_between_them() {
    let model = write_mesh_stl("agent-lab-judged", &cuboid(Vec3::ZERO, Vec3::splat(20.0)));
    let work = output_dir("agent-lab-judged-work");
    let bin = output_dir("agent-lab-judged-bin");
    let run = work.join("run.json");
    let encrust = env!("CARGO_BIN_EXE_encrust");
    let panel = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/project-panel.toml");
    let flags = serde_json::json!([
        model,
        format!("--profile={}", panel.display()),
        format!("--material={}", test_resin().display()),
        "--center",
        "--supports=medium",
    ]);
    let words: Vec<String> = flags
        .as_array()
        .expect("a list")
        .iter()
        .map(|flag| format!("'{}'", flag.as_str().expect("text")))
        .collect();
    // A client that keeps one revision as encrust-mcp would, without a server.
    let agent = script(
        &bin,
        "agent.sh",
        &format!(
            "mkdir -p jobs/j1\n'{encrust}' --json estimate {} > jobs/j1/1.json\n: > jobs/j1/1.toml\n\
             echo '{flags}' > jobs/j1/1.args.json\necho '{{\"job\": \"j1\"}}' > trace.jsonl",
            words.join(" ")
        ),
    );
    let judge = script(
        &bin,
        "judge.sh",
        "echo '{\"winner\": \"b\", \"why\": \"held up\"}'",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_agent-lab"))
        .arg(&model)
        .args(["--printer", panel.to_str().unwrap()])
        .arg("--resin")
        .arg(test_resin())
        .arg("--agent")
        .arg(format!("scripted={}", agent.display()))
        .arg("--judge")
        .arg(judge.display().to_string())
        .args(["--encrust", encrust, "--encrust-mcp", encrust])
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
    let drawn = |prepared: &Value| prepared["pictures"].as_array().map_or(0, Vec::len);
    assert_eq!(drawn(&cube["autopilot"]), 2, "iso and bottom: {cube}");
    let scripted = &cube["agents"][0];
    assert_eq!(drawn(&scripted["prepared"]), 2, "{scripted}");
    assert_eq!(scripted["verdict"]["winner"], "agent", "{scripted}");
    assert_eq!(scripted["verdict"]["why"], "held up");
}
