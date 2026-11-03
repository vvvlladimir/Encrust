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

type Client = RunningService<RoleClient, ()>;

/// The `encrust` built beside this server; `cargo test --workspace` builds both.
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

/// The panel and resin the command line's own tests print on; `project-panel` lights its whole
/// build volume, so a centred model is drawn whole.
fn profile(name: &str) -> PathBuf {
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

/// A 20 mm cube in `dir`, and a server keeping its jobs there.
async fn cube_and_server(dir: &Path) -> (PathBuf, Client) {
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    (model, serve(&encrust(), &dir.join("jobs")).await)
}

async fn serve(cli: &Path, jobs: &Path) -> Client {
    serve_with(cli, jobs, &[]).await
}

async fn serve_with(cli: &Path, jobs: &Path, extra: &[&std::ffi::OsStr]) -> Client {
    let server = PathBuf::from(env!("CARGO_BIN_EXE_encrust-mcp"));
    let transport = TokioChildProcess::new(tokio::process::Command::new(server).configure(|cmd| {
        cmd.arg("--encrust")
            .arg(cli)
            .arg("--jobs")
            .arg(jobs)
            .args(extra);
    }))
    .expect("the server binary starts");
    ().serve(transport)
        .await
        .expect("the server answers the handshake")
}

async fn call(client: &Client, tool: &'static str, args: Value) -> CallToolResult {
    let Value::Object(arguments) = args else {
        panic!("arguments are an object");
    };
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .expect("the server answers every call")
}

fn answer(result: &CallToolResult) -> Value {
    let text: String = result
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect();
    serde_json::from_str(&text).expect("every answer is JSON")
}

fn failed(result: &CallToolResult) -> bool {
    result.is_error == Some(true)
}

async fn start(client: &Client, model: &Path) -> String {
    let started = call(
        client,
        "start_job",
        json!({
            "model": model,
            "printer": profile("project-panel.toml"),
            "resin": profile("test-resin.toml"),
            "prepare": false,
        }),
    )
    .await;
    assert!(!failed(&started), "start_job failed: {}", answer(&started));
    let started = answer(&started);
    assert_eq!(started["revision"], 1);
    started["job"].as_str().expect("a job id").to_owned()
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
            "render",
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
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;

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
    let written = call(&client, "write_file", json!({"job": job, "output": output})).await;
    assert!(!failed(&written), "write_file failed: {}", answer(&written));
    assert_eq!(answer(&written)["revision"], 3);
    assert!(output.is_file(), "{} was not written", output.display());
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_traced_server_writes_a_line_per_call_and_keeps_each_revisions_check() {
    let dir = scratch("trace");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let trace = dir.join("trace.jsonl");
    let jobs = dir.join("jobs");
    let client = serve_with(&encrust(), &jobs, &["--trace".as_ref(), trace.as_os_str()]).await;
    let job = start(&client, &model).await;
    let refused = call(&client, "check", json!({"job": "no-such-job"})).await;
    assert!(failed(&refused));
    client.cancel().await.expect("the server stops");

    let lines: Vec<Value> = fs::read_to_string(&trace)
        .expect("the trace is written")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is JSON"))
        .collect();
    assert_eq!(lines.len(), 2, "one line per call: {lines:?}");
    assert_eq!(lines[0]["tool"], "start_job");
    assert_eq!(lines[0]["job"], job.as_str());
    assert_eq!(lines[0]["revision"], 1);
    assert_eq!(lines[0]["failed"], false);
    assert_eq!(lines[1]["tool"], "check");
    assert_eq!(lines[1]["failed"], true);

    let kept: Value = serde_json::from_str(
        &fs::read_to_string(jobs.join(&job).join("1.json")).expect("the check is kept"),
    )
    .expect("JSON");
    assert!(kept["slicing"].is_object(), "the estimate's report: {kept}");
}

#[tokio::test]
async fn a_job_the_autopilot_prepares_is_hollowed_drained_and_ready_to_write() {
    let dir = scratch("autopilot");
    let model = dir.join("block.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(40.0)), &model);
    let client = serve(&encrust(), &dir.join("jobs")).await;
    let started = call(
        &client,
        "start_job",
        json!({
            "model": model,
            "printer": profile("project-panel.toml"),
            "resin": profile("test-resin.toml"),
        }),
    )
    .await;
    assert!(!failed(&started), "start_job failed: {}", answer(&started));
    let started = answer(&started);
    let prepared = &started["report"]["models"][0]["prepared"];
    assert_eq!(
        prepared["hollowed"]["wall_mm"], 2.0,
        "a 40 mm block is worth hollowing: {started}"
    );
    assert_eq!(prepared["drained"], true);
    assert_eq!(started["report"]["slicing"]["trapped_pockets"], 0);
    let job = started["job"].as_str().expect("a job id");
    let output = dir.join("block.goo");
    let written = call(&client, "write_file", json!({"job": job, "output": output})).await;
    assert!(
        !failed(&written),
        "the autopilot's block is clean: {}",
        answer(&written)
    );
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_write_the_check_refuses_is_a_tool_error_and_leaves_no_file() {
    let dir = scratch("refused");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    assert!(
        !failed(&hollowed),
        "a sealed cavity is a finding: {}",
        answer(&hollowed)
    );

    let output = dir.join("cube.goo");
    let written = call(&client, "write_file", json!({"job": job, "output": output})).await;
    assert!(
        failed(&written),
        "the trapped resin was written: {}",
        answer(&written)
    );
    assert_eq!(answer(&written)["written"], false);
    assert!(!output.exists(), "{} is on disk", output.display());
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_render_shows_the_plate_from_each_side_with_the_trapped_resin_marked() {
    let dir = scratch("render");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    let drawn = call(&client, "render", json!({"job": job, "size_px": 128})).await;
    assert!(!failed(&drawn), "render failed: {:?}", drawn.content);
    let images = drawn
        .content
        .iter()
        .filter(
            |block| matches!(block, ContentBlock::Image(image) if image.mime_type == "image/png"),
        )
        .count();
    assert_eq!(images, 2, "iso and bottom were asked for");
    let listing = answer(&drawn);
    assert_eq!(
        listing["findings"][0]["kind"], "trapped-resin",
        "said {listing}"
    );
    let views: Vec<&str> = listing["pictures"]
        .as_array()
        .expect("pictures are listed")
        .iter()
        .filter_map(|picture| picture["view"].as_str())
        .collect();
    assert_eq!(views, ["iso", "bottom"]);
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_hollow_model_drained_by_the_engine_leaves_no_resin_trapped() {
    let dir = scratch("auto-drain");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let sealed = answer(
        &call(
            &client,
            "set_hollow",
            json!({"job": job, "hollow": {"wall_mm": 2.0}}),
        )
        .await,
    );
    assert_eq!(
        sealed["report"]["slicing"]["trapped_pockets"], 1,
        "said {sealed}"
    );
    let drained = call(
        &client,
        "set_drains",
        json!({"job": job, "drains": {"diameter_mm": 3.0, "auto": true}}),
    )
    .await;
    assert!(!failed(&drained), "set_drains failed: {}", answer(&drained));
    let drained = answer(&drained);
    assert_eq!(
        drained["report"]["slicing"]["trapped_pockets"], 0,
        "said {drained}"
    );
    let output = dir.join("cube.goo");
    let written = call(&client, "write_file", json!({"job": job, "output": output})).await;
    assert!(
        !failed(&written),
        "a drained cube is clean: {}",
        answer(&written)
    );
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn an_edit_the_command_line_refuses_is_not_kept() {
    let dir = scratch("not-kept");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let refused = call(
        &client,
        "set_supports",
        json!({"job": job, "supports": {"profile": "no-such-profile"}}),
    )
    .await;
    assert!(failed(&refused));
    let refused = answer(&refused);
    assert_eq!(refused["applied"], false);
    assert_eq!(refused["latest_revision"], 1);
    let latest = answer(&call(&client, "check", json!({"job": job})).await);
    assert_eq!(latest["revision"], 1, "the refused edit became {latest}");
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn two_edits_at_once_both_land_one_after_the_other() {
    let dir = scratch("race");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let (hollowed, supported) = tokio::join!(
        call(
            &client,
            "set_hollow",
            json!({"job": job, "hollow": {"wall_mm": 2.0, "open_floor": true}})
        ),
        call(
            &client,
            "set_supports",
            json!({"job": job, "supports": {"profile": "light"}})
        ),
    );
    let mut numbers = [
        answer(&hollowed)["revision"].clone(),
        answer(&supported)["revision"].clone(),
    ];
    numbers.sort_by_key(Value::as_u64);
    assert_eq!(numbers, [json!(2), json!(3)]);
    let latest = answer(&call(&client, "check", json!({"job": job})).await);
    assert!(
        !latest["recipe"]["hollow"].is_null(),
        "the hollow was lost: {latest}"
    );
    assert!(
        !latest["recipe"]["supports"].is_null(),
        "the supports were lost: {latest}"
    );
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_name_from_the_client_never_reaches_the_command_line_as_a_flag() {
    let dir = scratch("flags");
    let (model, client) = cube_and_server(&dir).await;
    let relative = call(
        &client,
        "start_job",
        json!({"model": "cube.stl", "printer": "elegoo-mars-4-ultra", "resin": "my-grey"}),
    )
    .await;
    assert!(failed(&relative));
    let dashed = call(
        &client,
        "start_job",
        json!({"model": model, "printer": "--profile=/etc/passwd", "resin": "my-grey"}),
    )
    .await;
    assert!(failed(&dashed));
    let escaping = call(
        &client,
        "check",
        json!({"job": "../outside", "revision": 1}),
    )
    .await;
    assert!(failed(&escaping));
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
    assert!(failed(&result));
    assert!(
        answer(&result).to_string().contains("no-encrust"),
        "said {}",
        answer(&result)
    );
    client.cancel().await.expect("the server stops");
}
