//! The server as a client starts it: over stdio, driving the real `encrust` binary.
#![expect(clippy::expect_used, reason = "an integration test fails by panicking")]

use std::fs;
use std::path::{Path, PathBuf};

use core_geometry::{Mesh, Vec3, cuboid};
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::{Value, json};

/// The `encrust` the workspace built beside this server; `cargo test --workspace` builds both.
fn encrust() -> PathBuf {
    let server = PathBuf::from(env!("CARGO_BIN_EXE_encrust-mcp"));
    let cli = server.with_file_name(format!("encrust{}", std::env::consts::EXE_SUFFIX));
    assert!(
        cli.is_file(),
        "{} is missing: run `cargo build -p encrust-cli --bin encrust` first",
        cli.display()
    );
    cli
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../encrust-cli/tests/fixtures")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-mcp-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the temp dir is writable");
    dir
}

/// Binary STL of `mesh`, three vertices a facet as an exporter writes them.
fn write_stl(mesh: &Mesh, path: &Path) {
    let mut bytes = vec![0u8; 80];
    bytes.extend_from_slice(&(mesh.faces.len() as u32).to_le_bytes());
    for face in &mesh.faces {
        bytes.extend_from_slice(&[0u8; 12]);
        for &index in face {
            for component in mesh.vertices[index as usize].to_array() {
                bytes.extend_from_slice(&component.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0u8; 2]);
    }
    fs::write(path, bytes).expect("the scratch dir is writable");
}

async fn serve(cli: &Path, jobs: &Path) -> RunningService<RoleClient, ()> {
    let server = PathBuf::from(env!("CARGO_BIN_EXE_encrust-mcp"));
    let transport = TokioChildProcess::new(tokio::process::Command::new(server).configure(|cmd| {
        cmd.arg("--encrust").arg(cli).arg("--jobs").arg(jobs);
    }))
    .expect("the server binary starts");
    ().serve(transport)
        .await
        .expect("the server answers the handshake")
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    tool: &'static str,
    args: Value,
) -> CallToolResult {
    let Value::Object(arguments) = args else {
        panic!("arguments are an object");
    };
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .expect("the server answers every call")
}

fn body(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

fn answer(result: &CallToolResult) -> Value {
    serde_json::from_str(&body(result)).expect("a tool that ran answers JSON")
}

#[tokio::test]
async fn the_server_offers_the_preparation_tools_and_marks_what_writes() {
    let client = serve(&encrust(), &scratch("tools")).await;
    let tools = client.list_all_tools().await.expect("tools are listed");
    let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "check",
            "inspect_model",
            "list_profiles",
            "revert",
            "send_to_printer",
            "set_drains",
            "set_hollow",
            "set_pose",
            "set_supports",
            "start_job",
            "write_file"
        ]
    );
    let hint = |name: &str| {
        let tool = tools.iter().find(|tool| tool.name == name).expect("listed");
        tool.annotations
            .as_ref()
            .and_then(|notes| notes.destructive_hint)
    };
    assert_eq!(hint("write_file"), Some(true));
    assert_eq!(hint("send_to_printer"), Some(true));
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_job_is_prepared_revision_by_revision_and_written() {
    let dir = scratch("job");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let client = serve(&encrust(), &dir.join("jobs")).await;

    let started = call(
        &client,
        "start_job",
        json!({
            "model": model,
            "printer": fixture("test-panel.toml"),
            "resin": fixture("test-resin.toml"),
        }),
    )
    .await;
    assert_ne!(
        started.is_error,
        Some(true),
        "start_job failed: {}",
        body(&started)
    );
    let started = answer(&started);
    let job = started["job"].as_str().expect("a job id").to_owned();
    assert_eq!(started["revision"], 1);
    assert!(
        started["report"]["status"].is_string(),
        "no status in {started}"
    );

    let supported = answer(
        &call(
            &client,
            "set_supports",
            json!({"job": job, "supports": {"profile": "light"}}),
        )
        .await,
    );
    assert_eq!(supported["revision"], 2);
    assert_eq!(supported["recipe"]["supports"]["profile"], "light");

    let first = answer(&call(&client, "check", json!({"job": job, "revision": 1})).await);
    assert!(
        first["recipe"]["supports"].is_null(),
        "revision 1 had no supports: {first}"
    );

    let reverted = answer(&call(&client, "revert", json!({"job": job, "revision": 1})).await);
    assert_eq!(reverted["revision"], 3);
    assert!(reverted["recipe"]["supports"].is_null());

    let output = dir.join("cube.goo");
    let written = call(
        &client,
        "write_file",
        json!({"job": job, "output": output, "accept_defects": true}),
    )
    .await;
    assert_ne!(
        written.is_error,
        Some(true),
        "write_file failed: {}",
        body(&written)
    );
    assert!(output.is_file(), "{} was not written", output.display());
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_job_that_does_not_exist_is_a_tool_error_naming_it() {
    let client = serve(&encrust(), &scratch("unknown")).await;
    let result = call(&client, "check", json!({"job": "no-such-job"})).await;
    assert_eq!(result.is_error, Some(true));
    assert!(
        body(&result).contains("no-such-job"),
        "said {}",
        body(&result)
    );
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_missing_command_line_is_a_tool_error_naming_it() {
    let dir = scratch("missing");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(10.0)), &model);
    let client = serve(&dir.join("no-encrust"), &dir.join("jobs")).await;
    let result = call(
        &client,
        "start_job",
        json!({"model": model, "printer": "elegoo-mars-4-ultra", "resin": "my-grey"}),
    )
    .await;
    assert_eq!(result.is_error, Some(true));
    assert!(
        body(&result).contains("no-encrust"),
        "said {}",
        body(&result)
    );
    client.cancel().await.expect("the server stops");
}
