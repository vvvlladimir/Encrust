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
    serve_at_home(cli, jobs, extra, None).await
}

/// The server with `home` as the user's home, so the profiles it keeps are a test's own.
async fn serve_at_home(
    cli: &Path,
    jobs: &Path,
    extra: &[&std::ffi::OsStr],
    home: Option<&Path>,
) -> Client {
    let server = PathBuf::from(env!("CARGO_BIN_EXE_encrust-mcp"));
    let transport = TokioChildProcess::new(tokio::process::Command::new(server).configure(|cmd| {
        // Never the user's own agents' folder, which an open window lists.
        let agents = jobs.with_file_name("agents");
        cmd.arg("--encrust")
            .arg(cli)
            .arg("--jobs")
            .arg(jobs)
            .arg("--agents")
            .arg(agents)
            .args(extra);
        // Never the user's open window either, unless the test stands one of its own.
        if !extra.iter().any(|arg| *arg == "--window") {
            cmd.arg("--no-window");
        }
        if let Some(home) = home {
            cmd.env("HOME", home)
                .env("XDG_CONFIG_HOME", home.join(".config"))
                .env("XDG_DATA_HOME", home.join(".local/share"))
                .env("APPDATA", home)
                .env("LOCALAPPDATA", home);
        }
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
    serde_json::from_str(&answer_text(result)).expect("every answer is JSON")
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
            "compare_poses",
            "create_resin",
            "find_printers",
            "fix_issues",
            "import_resin",
            "inspect_model",
            "list_profiles",
            "list_recipes",
            "plate_jobs",
            "render",
            "report_print",
            "revert",
            "save_recipe",
            "send_to_printer",
            "set_drains",
            "set_hollow",
            "set_machine",
            "set_pose",
            "set_repair",
            "set_shape",
            "set_slicing",
            "set_supports",
            "show_in_window",
            "start_job",
            "use_recipe",
            "write_file",
            "write_test_print"
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
    let manifest =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bundle/manifest.json"))
            .expect("the bundle's manifest reads");
    let manifest: Value = serde_json::from_str(&manifest).expect("the manifest is JSON");
    let mut listed: Vec<&str> = (manifest["tools"].as_array().expect("a tool list").iter())
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    listed.sort_unstable();
    assert_eq!(
        listed, names,
        "the bundle lists every tool the server serves"
    );
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
async fn a_cut_a_slicing_and_supports_placed_by_hand_are_each_kept_as_the_command_line_takes_them()
{
    let dir = scratch("shape");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let cut = call(
        &client,
        "set_shape",
        json!({"job": job, "modelling": {"cut": {"axis": "z", "at_mm": 10.0, "keep": "lower"}, "arrange": true}}),
    )
    .await;
    assert!(!failed(&cut), "the cut was refused: {}", answer(&cut));
    let height = answer(&cut)["report"]["slicing"]["layers"]
        .as_u64()
        .expect("layers");
    let sliced = call(
        &client,
        "set_slicing",
        json!({"job": job, "slicing": {"layer_height_mm": 0.1}}),
    )
    .await;
    assert!(
        !failed(&sliced),
        "the slicing was refused: {}",
        answer(&sliced)
    );
    let thicker = answer(&sliced)["report"]["slicing"]["layers"]
        .as_u64()
        .expect("layers");
    assert_eq!(
        thicker * 2,
        height,
        "0.1 mm layers are half as many as the test resin's 0.05 mm ones"
    );
    let held = call(
        &client,
        "set_supports",
        json!({"job": job, "supports": {"profile": "medium", "at_mm": [[10.0, 10.0, 0.0]], "only_at": true}}),
    )
    .await;
    assert!(
        !failed(&held),
        "the supports were refused: {}",
        answer(&held)
    );
    assert_eq!(answer(&held)["revision"], 4);
    client.cancel().await.expect("the server stops");
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
            "prepare": true,
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
async fn a_revert_the_command_line_refuses_is_not_kept() {
    let dir = scratch("revert-refused");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    fs::remove_file(&model).expect("the cube was written");
    let refused = call(&client, "revert", json!({"job": job, "revision": 1})).await;
    assert!(
        failed(&refused),
        "a revert with its model gone: {}",
        answer(&refused)
    );
    let latest = answer(&call(&client, "check", json!({"job": job})).await);
    assert_eq!(latest["revision"], 1, "the refused revert became {latest}");
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
        json!({"model": "cube.stl", "printer": "elegoo-mars-4-ultra", "resin": "my-grey", "prepare": false}),
    )
    .await;
    assert!(failed(&relative));
    let dashed = call(
        &client,
        "start_job",
        json!({"model": model, "printer": "--profile=/etc/passwd", "resin": "my-grey", "prepare": false}),
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
    let found = call(&client, "find_printers", json!({"address": "--wait=999"})).await;
    assert!(failed(&found), "an address that reads as a flag is refused");
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
        json!({"model": model, "printer": "elegoo-mars-4-ultra", "resin": "my-grey", "prepare": false}),
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

#[tokio::test]
async fn the_ways_to_stand_a_model_are_compared_drawn_and_one_is_taken() {
    let dir = scratch("poses");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let pose = json!({"by": "search", "goal": "auto"});
    let compared = call(
        &client,
        "compare_poses",
        json!({"job": job, "pose": pose, "count": 2, "size_px": 64}),
    )
    .await;
    assert!(!failed(&compared), "{:?}", compared.content);
    let first = match &compared.content[0] {
        ContentBlock::Text(text) => serde_json::from_str::<Value>(&text.text).expect("JSON"),
        other => panic!("the answer opens with its list, found {other:?}"),
    };
    let choices = first["choices"].as_array().expect("the choices are listed");
    assert!(!choices.is_empty() && choices.len() <= 2, "{first}");
    let pictures = compared
        .content
        .iter()
        .filter(|block| matches!(block, ContentBlock::Image(_)))
        .count();
    assert_eq!(pictures, choices.len(), "one iso picture a choice");

    let picked = json!({"by": "search", "goal": "auto", "pick": 1});
    let taken = call(&client, "set_pose", json!({"job": job, "pose": picked})).await;
    assert!(!failed(&taken), "{}", answer(&taken));
    assert_eq!(
        answer(&taken)["report"]["models"][0]["oriented"]["rotate_deg"],
        choices[0]["rotate_deg"],
        "the pick stands the model as the first choice"
    );
}

#[tokio::test]
async fn an_edit_is_set_against_the_revision_before_and_the_autopilot() {
    let dir = scratch("against");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let supported = json!({"job": job, "supports": {"profile": "medium"}});
    let edited = answer(&call(&client, "set_supports", supported).await);
    let compared = &edited["compared"];
    for name in ["previous", "autopilot"] {
        assert!(
            compared[name]["standing"].is_string(),
            "set against the {name}: {edited}"
        );
    }
}

#[tokio::test]
async fn a_pose_the_search_chose_holds_when_the_lift_changes() {
    let dir = scratch("pose-held");
    let model = dir.join("bar.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::new(40.0, 10.0, 6.0)), &model);
    let client = serve(&encrust(), &dir.join("jobs")).await;
    let job = start(&client, &model).await;
    let pose = json!({"by": "search", "goal": "auto", "pick": 2});
    let posed = answer(&call(&client, "set_pose", json!({"job": job, "pose": pose})).await);
    let mut turns = Vec::new();
    for lift_mm in [3.0, 8.0] {
        let supports = json!({"profile": "auto", "lift_mm": lift_mm});
        let lifted = call(
            &client,
            "set_supports",
            json!({"job": job, "supports": supports}),
        );
        let lifted = answer(&lifted.await);
        turns.push(lifted["recipe"]["pose"]["stood_deg"].clone());
    }
    client.cancel().await.expect("the server stops");

    let chosen = &posed["recipe"]["pose"]["stood_deg"];
    assert!(chosen.is_array(), "the turn chosen is kept: {posed}");
    assert_eq!(turns, [chosen.clone(), chosen.clone()], "the pose holds");
    let flags = fs::read_to_string(dir.join("jobs").join(&job).join("4.args.json"))
        .expect("revision 4 keeps its flags");
    assert!(
        !flags.contains("--orient\""),
        "no search runs again: {flags}"
    );
}

#[tokio::test]
async fn points_kept_clear_are_kept_with_the_turn_they_were_seen_in() {
    let dir = scratch("keep-clear");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let pose = json!({"by": "search", "goal": "auto", "keep_clear_at_mm": [[40.0, 40.0, 20.0]]});
    let taken = answer(&call(&client, "set_pose", json!({"job": job, "pose": pose})).await);
    assert_eq!(
        taken["recipe"]["pose"]["keep_clear_seen_deg"],
        json!([0.0, 0.0, 0.0]),
        "revision one stands the cube as its file does: {taken}"
    );
}

#[tokio::test]
async fn each_revision_keeps_the_plate_its_check_wrote_and_hands_it_to_the_open_window() {
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

    let dir = scratch("window");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("bound").port();
    let file = dir.join("window.json");
    fs::write(
        &file,
        json!({"schema": 4, "port": port, "token": "t", "pid": 1}).to_string(),
    )
    .expect("writable");
    let window = tokio::spawn(async move {
        let mut lines = Vec::new();
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.expect("the server connects");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("a line");
            let reply = b"{\"shown\":true}\n";
            reader.get_mut().write_all(reply).await.expect("answered");
            lines.push(serde_json::from_str::<Value>(&line).expect("JSON"));
        }
        lines
    });
    let jobs = dir.join("jobs");
    let client = serve_with(&encrust(), &jobs, &["--window".as_ref(), file.as_os_str()]).await;
    let job = start(&client, &model).await;
    let edited = call(
        &client,
        "use_recipe",
        // The way steps are decided is no edit, so the window is shown the plate it prepared.
        json!({"job": job, "recipe": "miniatures"}),
    )
    .await;
    assert_eq!(answer(&edited)["window"], json!({"shown": true}));
    client.cancel().await.expect("the server stops");

    let kept = jobs.join(&job).join("2.encrust");
    assert!(
        kept.is_file(),
        "the revision keeps the plate its recipe prepared"
    );
    let lines = window.await.expect("the window's task ends");
    assert_eq!(
        (lines[1]["schema"].as_u64(), lines[1]["revision"].as_u64()),
        (Some(4), Some(2))
    );
    assert_eq!(
        lines[1]["plate"].as_str(),
        kept.to_str(),
        "the window is shown that plate"
    );
}

/// A window answering `lines` connections by kind: shown, the job's latest plate at version 5
/// with its model object 1, edits applied; it hands back the lines read.
async fn plate_window(dir: &Path, lines: usize) -> (PathBuf, tokio::task::JoinHandle<Vec<Value>>) {
    plate_window_moving(dir, lines, 0).await
}

/// A window whose plate the user changes under the first `moves` edits handed to it, each
/// answered `stale` with the version raised.
async fn plate_window_moving(
    dir: &Path,
    lines: usize,
    moves: u64,
) -> (PathBuf, tokio::task::JoinHandle<Vec<Value>>) {
    window_answering(dir, lines, moves, true).await
}

/// As `plate_window_moving`, the edits that are not stale `landed` or refused.
async fn window_answering(
    dir: &Path,
    lines: usize,
    moves: u64,
    landed: bool,
) -> (PathBuf, tokio::task::JoinHandle<Vec<Value>>) {
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("bound").port();
    let file = dir.join("window.json");
    fs::write(
        &file,
        json!({"schema": 4, "port": port, "token": "t", "pid": 1}).to_string(),
    )
    .expect("writable");
    let jobs = dir.join("jobs");
    let window = tokio::spawn(async move {
        let mut read = Vec::new();
        let mut moved = 0;
        for _ in 0..lines {
            let (stream, _) = listener.accept().await.expect("the server connects");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("a line");
            let line: Value = serde_json::from_str(&line).expect("JSON");
            let reply = if line["want"] == "plate" {
                let job = jobs.join(line["job"].as_str().expect("a job"));
                let version = 5 + moved;
                json!({"plate": latest_plate(&job), "version": version, "objects": [1], "job_object": 1})
            } else if line.get("apply").is_some() && moved < moves {
                moved += 1;
                json!({"applied": false, "stale": true, "version": 5 + moved})
            } else if line.get("apply").is_some() {
                json!({"applied": landed, "why_not": (!landed).then_some("a tool failed")})
            } else {
                json!({"shown": true})
            };
            let reply = format!("{reply}\n");
            reader
                .get_mut()
                .write_all(reply.as_bytes())
                .await
                .expect("answered");
            read.push(line);
        }
        read
    });
    (file, window)
}

/// The plate a job's latest revision left, `N.encrust` with the highest N.
fn latest_plate(job: &Path) -> PathBuf {
    let plates = fs::read_dir(job).expect("the job's folder reads").flatten();
    let numbered = plates.filter_map(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        let number: u32 = name.strip_suffix(".encrust")?.parse().ok()?;
        Some((number, entry.path()))
    });
    numbered.max().expect("the job left a plate").1
}

#[tokio::test]
async fn a_cut_reaches_the_window_as_an_edit_and_an_undo_there_reaches_the_next_revision() {
    let dir = scratch("window-cut");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    // The revision shown, the plate and the cut, then the plate and the next revision shown.
    let (file, window) = plate_window(&dir, 5).await;
    let jobs = dir.join("jobs");
    let client = serve_with(&encrust(), &jobs, &["--window".as_ref(), file.as_os_str()]).await;
    let job = start(&client, &model).await;

    let cut = call(
        &client,
        "set_shape",
        json!({"job": job, "modelling": {"cut": {"axis": "z", "at_mm": 10.0}, "arrange": true}}),
    )
    .await;
    assert_eq!(
        answer(&cut)["window"],
        json!({"applied": true}),
        "{}",
        answer(&cut)
    );
    // The fake window still has object 1, the model whole: the user undid the cut there.
    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    client.cancel().await.expect("the server stops");

    let lines = window.await.expect("the window's task ends");
    let applied = &lines[2];
    assert_eq!(applied["base_version"], 5);
    assert_eq!(
        applied["apply"][0]["object"], 1,
        "the cut names the job's model"
    );
    assert!(
        answer(&hollowed)["note"]
            .as_str()
            .is_some_and(|note| note.contains("undone")),
        "{}",
        answer(&hollowed)
    );
    let second = fs::read_to_string(jobs.join(&job).join("2.toml")).expect("revision 2 is kept");
    assert!(
        second.contains("[modelling.cut]"),
        "the cut was kept: {second}"
    );
    let third = fs::read_to_string(jobs.join(&job).join("3.toml")).expect("revision 3 is kept");
    assert!(
        !third.contains("[modelling.cut]"),
        "the undone cut is gone: {third}"
    );
}

#[tokio::test]
async fn a_job_prepared_by_another_recipe_keeps_that_recipe_as_its_steps() {
    let dir = scratch("recipes");
    let (model, client) = cube_and_server(&dir).await;
    let listed = answer(&call(&client, "list_recipes", json!({})).await);
    let recipes = listed["recipes"].as_array().expect("recipes are listed");
    let miniatures = recipes
        .iter()
        .find(|recipe| recipe["id"] == "miniatures")
        .expect("miniatures is built in");
    assert!(
        miniatures["toml"]
            .as_str()
            .is_some_and(|text| text.contains("[[step]]"))
    );
    let job = start(&client, &model).await;
    let used = call(
        &client,
        "use_recipe",
        json!({"job": job, "recipe": "miniatures"}),
    )
    .await;
    assert!(!failed(&used), "{}", answer(&used));
    client.cancel().await.expect("the server stops");
    let steps = fs::read_to_string(dir.join("jobs").join(&job).join("2.recipe.toml"))
        .expect("the revision keeps its steps");
    assert!(steps.contains("Miniatures"), "{steps}");
}

#[tokio::test]
async fn a_resin_taken_in_from_its_file_is_listed_as_the_user_s_own() {
    let dir = scratch("resin");
    let home = dir.join("home");
    let client = serve_at_home(&encrust(), &dir.join("jobs"), &[], Some(&home)).await;
    let imported = call(
        &client,
        "import_resin",
        json!({"file": profile("test-resin.toml")}),
    )
    .await;
    assert!(!failed(&imported), "{}", answer(&imported));
    let id = answer(&imported)["id"].as_str().expect("an id").to_owned();
    let listed = answer(&call(&client, "list_profiles", json!({})).await);
    assert!(
        listed["resins"]
            .as_array()
            .is_some_and(|resins| resins.iter().any(|resin| resin["id"] == id.as_str())),
        "{id} among the user's resins: {}",
        listed["resins"]
    );
    let relative = call(&client, "import_resin", json!({"file": "resin.toml"})).await;
    assert!(
        failed(&relative),
        "a relative path never reaches the command line"
    );
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn the_exposure_check_is_written_and_prepared_like_any_model() {
    let dir = scratch("test-print");
    let client = serve(&encrust(), &dir.join("jobs")).await;
    let written = call(
        &client,
        "write_test_print",
        json!({"print": "exposure", "folder": dir}),
    )
    .await;
    assert!(!failed(&written), "{}", answer(&written));
    let path = PathBuf::from(answer(&written)["path"].as_str().expect("a path"));
    assert_eq!(path, dir.join("exposure-check.stl"));
    start(&client, &path).await;
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn edits_on_a_plate_that_moved_on_are_built_and_checked_again_on_it() {
    let dir = scratch("window-stale");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    // The revision shown, then the plate, a stale answer, the plate again and the edits landed.
    let (file, window) = plate_window_moving(&dir, 5, 1).await;
    let jobs = dir.join("jobs");
    let client = serve_with(&encrust(), &jobs, &["--window".as_ref(), file.as_os_str()]).await;
    let job = start(&client, &model).await;
    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    client.cancel().await.expect("the server stops");

    assert_eq!(
        answer(&hollowed)["window"],
        json!({"applied": true}),
        "{}",
        answer(&hollowed)
    );
    let lines = window.await.expect("the window's task ends");
    assert_eq!(
        lines[2]["base_version"], 5,
        "first on the plate handed over"
    );
    assert_eq!(lines[3]["want"], "plate", "the plate asked for again");
    assert_eq!(
        lines[4]["base_version"], 6,
        "then on the plate as it moved on"
    );
    let revisions = fs::read_dir(jobs.join(&job))
        .expect("the job's folder reads")
        .flatten();
    let kept = revisions.filter(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        name.strip_suffix(".toml")
            .is_some_and(|number| number.parse::<u32>().is_ok())
    });
    assert_eq!(kept.count(), 2, "the stale try kept no revision of its own");
}

#[tokio::test]
async fn edits_the_window_takes_back_keep_no_revision() {
    let dir = scratch("window-refused");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    // The revision shown, then the plate and the edits refused.
    let (file, window) = window_answering(&dir, 3, 0, false).await;
    let jobs = dir.join("jobs");
    let client = serve_with(&encrust(), &jobs, &["--window".as_ref(), file.as_os_str()]).await;
    let job = start(&client, &model).await;
    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    client.cancel().await.expect("the server stops");
    window.await.expect("the window's task ends");

    assert!(failed(&hollowed), "{}", answer(&hollowed));
    assert_eq!(answer(&hollowed)["latest_revision"], 1);
    assert_eq!(answer(&hollowed)["window"]["why_not"], "a tool failed");
    assert!(
        !jobs.join(&job).join("2.toml").exists(),
        "no revision kept that the window does not hold"
    );
}

#[tokio::test]
async fn a_checked_revision_whose_edits_the_window_takes_back_is_shown_by_its_plate() {
    let dir = scratch("window-refused-shown");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    // The revision shown; the turn checked by the flags, its edit refused, then its plate shown.
    let (file, window) = window_answering(&dir, 4, 0, false).await;
    let jobs = dir.join("jobs");
    let client = serve_with(&encrust(), &jobs, &["--window".as_ref(), file.as_os_str()]).await;
    let job = start(&client, &model).await;
    let pose = json!({"by": "search", "goal": "auto"});
    let posed = call(&client, "set_pose", json!({"job": job, "pose": pose})).await;
    client.cancel().await.expect("the server stops");
    let lines = window.await.expect("the window's task ends");

    assert!(!failed(&posed), "{}", answer(&posed));
    let told = &answer(&posed)["window"];
    assert_eq!(told["applied"], false, "{told}");
    assert_eq!(told["then_shown"], json!({"shown": true}), "{told}");
    assert_eq!(
        lines[3]["revision"], 2,
        "the window is shown revision 2's plate"
    );
    assert!(
        !jobs.join(&job).join("2.edits.json").exists(),
        "the edits not taken are forgotten"
    );
}

#[tokio::test]
async fn a_job_on_the_standard_machine_is_written_only_on_the_machine_the_user_names() {
    let dir = scratch("standard-machine");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let client = serve(&encrust(), &dir.join("jobs")).await;
    let started = call(
        &client,
        "start_job",
        json!({"model": model, "prepare": false}),
    )
    .await;
    assert!(!failed(&started), "{}", answer(&started));
    let job = answer(&started)["job"].as_str().expect("a job").to_owned();
    let output = dir.join("cube.goo");
    let unnamed = call(&client, "write_file", json!({"job": job, "output": output})).await;
    assert!(
        failed(&unnamed),
        "nothing written on a machine nobody named"
    );
    assert!(!output.exists(), "no file left behind");

    let named = call(
        &client,
        "write_file",
        json!({
            "job": job,
            "output": output,
            "accept_defects": true,
            "printer": profile("project-panel.toml"),
            "resin": profile("test-resin.toml"),
        }),
    )
    .await;
    assert!(!failed(&named), "{}", answer(&named));
    assert!(output.is_file(), "written on the machine named");
    let half = call(
        &client,
        "start_job",
        json!({"model": model, "printer": profile("project-panel.toml"), "prepare": false}),
    )
    .await;
    assert!(failed(&half), "a printer without its resin is refused");
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn with_no_window_a_revision_is_built_on_the_plate_the_last_one_kept() {
    let dir = scratch("kept-base");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let client = serve_at_home(&encrust(), &dir.join("jobs"), &[], Some(&dir.join("home"))).await;
    let job = start(&client, &model).await;
    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.5}}),
    )
    .await;
    assert!(!failed(&hollowed), "{}", answer(&hollowed));
    let kept = dir.join("jobs").join(&job);
    assert!(
        kept.join("2.edits.json").is_file() && kept.join("2.encrust").is_file(),
        "revision 2 is plate 1 and its edits"
    );
    let output = dir.join("cube.goo");
    let written = call(
        &client,
        "write_file",
        json!({"job": job, "output": output, "accept_defects": true}),
    )
    .await;
    assert!(!failed(&written), "{}", answer(&written));
    assert!(output.is_file(), "revision 2's plate is sliced");
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn with_no_window_a_cut_models_pieces_are_finished_on_the_plate_the_cut_kept() {
    let dir = scratch("kept-pieces");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let client = serve_at_home(&encrust(), &dir.join("jobs"), &[], Some(&dir.join("home"))).await;
    let job = start(&client, &model).await;
    let cut =
        json!({"job": job, "modelling": {"cut": {"axis": "z", "at_mm": 10.0}, "arrange": true}});
    assert!(!failed(&call(&client, "set_shape", cut).await));

    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    let supported = call(
        &client,
        "set_supports",
        json!({"job": job, "supports": {"profile": "auto", "cavity": true}}),
    )
    .await;

    for (reply, number) in [(&hollowed, 3), (&supported, 4)] {
        assert!(!failed(reply), "{}", answer(reply));
        let kept = dir.join("jobs").join(&job);
        assert!(
            kept.join(format!("{number}.edits.json")).is_file(),
            "revision {number} is the plate before it and its edits, not the cut staged anew"
        );
    }
    let models = answer(&supported)["report"]["models"].clone();
    let halves = models.as_array().expect("a model a half");
    assert_eq!(halves.len(), 2, "{models}");
    assert!(
        halves.iter().all(|half| !half["hollow"].is_null()),
        "both stay hollow under their supports: {models}"
    );
    let output = dir.join("cube.goo");
    let written = call(
        &client,
        "write_file",
        json!({"job": job, "output": output, "accept_defects": true}),
    )
    .await;
    assert!(!failed(&written), "{}", answer(&written));
    assert!(output.is_file(), "revision 4's plate is sliced");
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_point_given_with_no_object_goes_to_the_half_it_lies_on() {
    let dir = scratch("points-by-object");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let cut =
        json!({"job": job, "modelling": {"cut": {"axis": "z", "at_mm": 10.0}, "arrange": true}});
    assert!(!failed(&call(&client, "set_shape", cut).await));
    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    let first = answer(&hollowed)["report"]["models"][0].clone();
    let bounds = &first["fit"]["bounds_mm"];
    let mid = |axis: usize| {
        let low = bounds[0][axis].as_f64().expect("a low corner");
        let high = bounds[1][axis].as_f64().expect("a high corner");
        f64::midpoint(low, high)
    };
    let top = [mid(0), mid(1), bounds[1][2].as_f64().expect("a top")];

    let drained = call(
        &client,
        "set_drains",
        json!({"job": job, "drains": {"diameter_mm": 3.0, "auto": true, "at_mm": [top]}}),
    )
    .await;
    assert!(!failed(&drained), "{}", answer(&drained));
    let more = &answer(&drained)["recipe"]["more_drains"];
    assert_eq!(
        more[0]["objects"],
        json!([first["id"]]),
        "{}",
        answer(&drained)
    );

    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_jobs_steps_are_kept_as_a_recipe_of_the_users_and_one_that_would_not_run_is_not() {
    let dir = scratch("save-recipe");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let client = serve_at_home(&encrust(), &dir.join("jobs"), &[], Some(&dir.join("home"))).await;
    let job = start(&client, &model).await;
    call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.5}}),
    )
    .await;
    let saved = call(
        &client,
        "save_recipe",
        json!({"job": job, "name": "Hollow cubes"}),
    )
    .await;
    assert!(!failed(&saved), "{}", answer(&saved));
    let id = answer(&saved)["id"].as_str().expect("an id").to_owned();
    let listed = answer(&call(&client, "list_recipes", json!({})).await);
    let kept = listed["recipes"]
        .as_array()
        .and_then(|recipes| recipes.iter().find(|recipe| recipe["id"] == id.as_str()))
        .expect("the saved recipe is listed")
        .clone();
    assert_eq!(kept["name"], "Hollow cubes");
    assert!(
        kept["toml"]
            .as_str()
            .is_some_and(|text| text.contains("wall_mm = 2.5")),
        "the wall the job was given: {kept}"
    );

    let odd = "name = \"Odd\"\nformat = 1\n\n[[step]]\ndo = \"check\"\nwhen = \"no_such > 1\"\n";
    let refused = call(&client, "save_recipe", json!({"toml": odd, "name": "Odd"})).await;
    assert!(failed(&refused), "a recipe that would not run is refused");
    let both = call(
        &client,
        "save_recipe",
        json!({"job": job, "toml": odd, "name": "Both"}),
    )
    .await;
    assert!(failed(&both), "a job and a toml at once is refused");
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_project_the_window_saved_is_checked_drawn_written_and_shaped() {
    let dir = scratch("project");
    let client = serve(&encrust(), &dir.join("jobs")).await;
    let project = profile("cube.encrust");
    let started = call(
        &client,
        "start_job",
        json!({
            "model": project,
            "printer": profile("project-panel.toml"),
            "resin": profile("test-resin.toml"),
            "prepare": true,
        }),
    )
    .await;
    assert!(!failed(&started), "start_job failed: {}", answer(&started));
    let job = answer(&started)["job"]
        .as_str()
        .expect("a job id")
        .to_owned();
    let drawn = call(&client, "render", json!({"job": job, "views": ["iso"]})).await;
    assert!(!failed(&drawn), "render failed: {}", answer(&drawn));
    let output = dir.join("cube.goo");
    let written = call(
        &client,
        "write_file",
        json!({"job": job, "output": output, "accept_defects": true}),
    )
    .await;
    assert!(!failed(&written), "write_file failed: {}", answer(&written));
    assert!(output.is_file());
    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    assert!(
        !failed(&hollowed),
        "a project's objects are shaped: {}",
        answer(&hollowed)
    );
    let models = answer(&hollowed)["report"]["models"].clone();
    assert!(
        !models[0]["hollow"].is_null(),
        "its cube is hollowed: {models}"
    );
    let inspected = answer(&call(&client, "inspect_model", json!({"path": project})).await);
    assert!(
        inspected["objects"][0]["id"].as_u64().is_some(),
        "inspect_model lists a project's objects by id: {inspected}"
    );
    client.cancel().await.expect("the server stops");
}

fn answer_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_print_reported_is_read_against_the_revision_written_and_kept_with_the_job() {
    let dir = scratch("report-print");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    let written = call(
        &client,
        "write_file",
        json!({"job": job, "output": dir.join("cube.goo"), "accept_defects": true}),
    )
    .await;
    assert!(!failed(&written), "{}", answer(&written));
    call(
        &client,
        "set_supports",
        json!({"job": job, "supports": {"profile": "light"}}),
    )
    .await;

    let reported = call(
        &client,
        "report_print",
        json!({"job": job, "symptoms": ["resin-crack"], "note": "split along the side"}),
    )
    .await;
    assert!(!failed(&reported), "{}", answer(&reported));
    let reported = answer(&reported);
    assert_eq!(
        reported["revision"], 2,
        "the revision written, not the latest"
    );
    assert_eq!(reported["prints_reported"], 1);
    let first = &reported["diagnosis"]["symptoms"][0]["causes"][0];
    assert_eq!(first["step"], "drains", "{reported}");
    assert!(
        first["seen"].is_string(),
        "the trapped resin is seen: {first}"
    );

    let none = call(&client, "report_print", json!({"job": job, "symptoms": []})).await;
    assert!(failed(&none), "a print with no symptom is refused");
    client.cancel().await.expect("the server stops");
    let kept = fs::read_to_string(dir.join("jobs").join(&job).join("prints.jsonl"))
        .expect("the print is kept with the job");
    assert!(kept.contains("split along the side"), "{kept}");
}

#[tokio::test]
async fn a_resin_from_its_bottles_numbers_is_listed_as_the_user_s_own() {
    let dir = scratch("create-resin");
    let home = dir.join("home");
    let client = serve_at_home(&encrust(), &dir.join("jobs"), &[], Some(&home)).await;
    let numbers = json!({
        "printer": "elegoo-mars-4-ultra",
        "name": "Bottle grey",
        "exposure_s": 2.5,
        "bottom_exposure_s": 30.0,
        "bottom_layers": 5,
        "lift_speed_mm_min": 60.0,
    });
    let created = call(&client, "create_resin", numbers.clone()).await;
    assert!(!failed(&created), "{}", answer(&created));
    let id = answer(&created)["id"].as_str().expect("an id").to_owned();
    let listed = answer(&call(&client, "list_profiles", json!({})).await);
    assert!(
        listed["resins"]
            .as_array()
            .is_some_and(|resins| resins.iter().any(|resin| resin["id"] == id.as_str())),
        "{id} among the user's resins: {}",
        listed["resins"]
    );
    let mut none = numbers;
    none["exposure_s"] = json!(0.0);
    let refused = call(&client, "create_resin", none).await;
    assert!(failed(&refused), "an exposure of zero is refused");
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn the_server_lists_itself_as_an_agent_with_the_jobs_it_touched_and_ends_when_closed() {
    let dir = scratch("agent");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let agents = dir.join("agents");
    let file = fs::read_dir(&agents)
        .expect("the agents' folder")
        .filter_map(|entry| Some(entry.ok()?.path()))
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .expect("the server's file");
    let seen: Value = serde_json::from_slice(&fs::read(&file).expect("readable")).expect("JSON");
    assert_eq!(seen["jobs"], json!([job]));
    assert!(
        seen["client"].is_string(),
        "the client named itself: {seen}"
    );
    assert!(seen.get("working").is_none(), "{seen}");
    client.cancel().await.expect("the server stops");
    let ended = (0..50).find_map(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let seen: Value = serde_json::from_slice(&fs::read(&file).ok()?).ok()?;
        seen["ended_s"].as_u64()
    });
    assert!(ended.is_some(), "the file says the agent ended");
}

#[tokio::test]
async fn a_revision_asked_for_is_shown_in_the_window_marked_open_and_with_none_the_client_is_told()
{
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

    let dir = scratch("show-in-window");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("bound").port();
    let file = dir.join("window.json");
    fs::write(
        &file,
        json!({"schema": 4, "port": port, "token": "t", "pid": 1}).to_string(),
    )
    .expect("writable");
    let window = tokio::spawn(async move {
        let mut lines = Vec::new();
        for answer in [
            &b"{\"shown\":false,\"why_not\":\"not following\"}\n"[..],
            b"{\"shown\":true}\n",
        ] {
            let (stream, _) = listener.accept().await.expect("the server connects");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("a line");
            reader.get_mut().write_all(answer).await.expect("answered");
            lines.push(serde_json::from_str::<Value>(&line).expect("JSON"));
        }
        lines
    });
    let app = dir.join("no-such-encrust-gui");
    let extra = [
        "--window".as_ref(),
        file.as_os_str(),
        "--app".as_ref(),
        app.as_os_str(),
    ];
    let client = serve_with(&encrust(), &dir.join("jobs"), &extra).await;
    let job = start(&client, &model).await;
    let shown = call(&client, "show_in_window", json!({"job": job})).await;
    assert!(!failed(&shown), "{}", answer(&shown));
    assert_eq!(answer(&shown)["started"], false);
    let lines = window.await.expect("the window's task ends");
    assert_eq!(lines[0]["open"], false, "a revision kept is not marked");
    assert_eq!(lines[1]["open"], true, "a revision asked for is");

    fs::remove_file(&file).expect("the window closes");
    let closed = call(&client, "show_in_window", json!({"job": job})).await;
    assert!(failed(&closed));
    assert!(
        answer_text(&closed).contains("open Encrust"),
        "{}",
        answer_text(&closed)
    );
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn models_combined_are_one_model_the_command_line_takes() {
    let dir = scratch("combine");
    let (model, client) = cube_and_server(&dir).await;
    let other = dir.join("other.stl");
    write_stl(&cuboid(Vec3::splat(10.0), Vec3::splat(30.0)), &other);
    let job = start(&client, &model).await;
    let combine = json!({"op": "union", "with": [other]});
    let combined = call(
        &client,
        "set_shape",
        json!({"job": job, "modelling": {"combine": combine}}),
    )
    .await;
    assert!(!failed(&combined), "{}", answer(&combined));
}

#[tokio::test]
async fn two_jobs_are_laid_on_one_plate_apart_and_written_as_one_file() {
    let dir = scratch("plate-jobs");
    let (model, client) = cube_and_server(&dir).await;
    let first = start(&client, &model).await;
    let other = dir.join("other.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(10.0)), &other);
    let second = start(&client, &other).await;
    let output = dir.join("both.goo");

    let laid = call(
        &client,
        "plate_jobs",
        json!({"jobs": [first, second], "output": output, "accept_defects": true}),
    )
    .await;

    assert!(!failed(&laid), "plate_jobs failed: {}", answer(&laid));
    let laid = answer(&laid);
    assert_eq!(laid["written"], true, "{laid}");
    assert!(output.is_file(), "one file holds both");
    assert!(
        !laid["report"].to_string().contains("\"meets\""),
        "the two stand apart: {laid}"
    );
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_cut_model_is_hollowed_and_supported_half_by_half_and_one_half_alone_by_its_id() {
    let dir = scratch("cut-objects");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let cut =
        json!({"job": job, "modelling": {"cut": {"axis": "z", "at_mm": 10.0}, "arrange": true}});
    let cut = call(&client, "set_shape", cut).await;
    assert!(!failed(&cut), "the cut failed: {}", answer(&cut));

    let hollowed = call(
        &client,
        "set_hollow",
        json!({"job": job, "hollow": {"wall_mm": 2.0}}),
    )
    .await;
    assert!(
        !failed(&hollowed),
        "the hollow failed: {}",
        answer(&hollowed)
    );
    let models = answer(&hollowed)["report"]["models"].clone();
    let halves = models.as_array().expect("a model a half");
    assert_eq!(halves.len(), 2, "{models}");
    assert!(
        halves.iter().all(|half| !half["hollow"].is_null()),
        "both hollowed: {models}"
    );

    let id = halves[0]["id"].as_u64().expect("each half names its id");
    let walls = |reply: &Value| -> Vec<Option<f64>> {
        let models = reply["report"]["models"].as_array().expect("models");
        (models.iter())
            .map(|half| half["hollow"]["wall_mm"].as_f64())
            .collect()
    };
    let thicker = json!({"job": job, "hollow": {"wall_mm": 3.0, "objects": [id]}});
    let thicker = answer(&call(&client, "set_hollow", thicker).await);
    assert_eq!(
        walls(&thicker),
        [Some(3.0), Some(2.0)],
        "the other half stays hollow: {thicker}"
    );

    let solid = json!({"job": job, "hollow": null});
    assert!(!failed(&call(&client, "set_hollow", solid).await));
    let one = json!({"job": job, "hollow": {"wall_mm": 2.0, "objects": [id]}});
    let one = answer(&call(&client, "set_hollow", one).await);
    assert_eq!(walls(&one), [Some(2.0), None], "one half alone: {one}");
    client.cancel().await.expect("the server stops");
}

#[tokio::test]
async fn a_job_moves_to_another_printer_and_resin_and_is_checked_there() {
    let dir = scratch("set-machine");
    let (model, client) = cube_and_server(&dir).await;
    let job = start(&client, &model).await;
    let moved = call(
        &client,
        "set_machine",
        json!({"job": job, "printer": profile("test-panel.toml"), "resin": profile("test-resin.toml")}),
    )
    .await;
    assert!(!failed(&moved), "set_machine failed: {}", answer(&moved));
    assert_eq!(answer(&moved)["revision"], 2);
    let refused = call(
        &client,
        "set_machine",
        json!({"job": job, "printer": "-x", "resin": "r"}),
    )
    .await;
    assert!(failed(&refused), "a name read as a flag is refused");
    client.cancel().await.expect("the server stops");
}

/// A window whose plate holds the job's model, object 1, until a cut lands, then its halves,
/// 2 and 3; it answers `lines` connections by their kind and returns the lines read.
async fn cut_window(dir: &Path, lines: usize) -> (PathBuf, tokio::task::JoinHandle<Vec<Value>>) {
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("bound").port();
    let file = dir.join("window.json");
    fs::write(
        &file,
        json!({"schema": 4, "port": port, "token": "t", "pid": 1}).to_string(),
    )
    .expect("writable");
    let jobs = dir.join("jobs");
    let window = tokio::spawn(async move {
        let (mut read, mut cut) = (Vec::new(), false);
        for _ in 0..lines {
            let (stream, _) = listener.accept().await.expect("the server connects");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("a line");
            let line: Value = serde_json::from_str(&line).expect("JSON");
            let reply = if line["want"] == "plate" {
                let job = jobs.join(line["job"].as_str().expect("a job"));
                let (version, objects, object) = if cut {
                    (6, json!([2, 3]), Value::Null)
                } else {
                    (5, json!([1]), json!(1))
                };
                json!({"plate": latest_plate(&job), "version": version, "objects": objects, "job_object": object})
            } else if line.get("apply").is_some() {
                cut = true;
                json!({"applied": true})
            } else {
                json!({"shown": true})
            };
            let reply = format!("{reply}\n");
            (reader.get_mut().write_all(reply.as_bytes()).await).expect("answered");
            read.push(line);
        }
        read
    });
    (file, window)
}

#[tokio::test]
async fn supports_asked_after_a_cut_the_window_holds_reach_it_as_the_plate_checked() {
    let dir = scratch("window-cut-then-supports");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    // The first revision shown; the plate and the cut; then the plate, for an undo, and the
    // plate of pieces the supports were added on, shown.
    let (file, window) = cut_window(&dir, 5).await;
    let jobs = dir.join("jobs");
    let client = serve_with(&encrust(), &jobs, &["--window".as_ref(), file.as_os_str()]).await;
    let job = start(&client, &model).await;
    let cut = json!({"axis": "z", "at_mm": 10.0});
    call(
        &client,
        "set_shape",
        json!({"job": job, "modelling": {"cut": cut, "arrange": true}}),
    )
    .await;

    let supported = call(
        &client,
        "set_supports",
        json!({"job": job, "supports": {"profile": "auto"}}),
    )
    .await;
    client.cancel().await.expect("the server stops");

    assert_eq!(
        answer(&supported)["window"],
        json!({"shown": true}),
        "{}",
        answer(&supported)
    );
    let lines = window.await.expect("the window's task ends");
    assert_eq!(
        lines[4]["revision"].as_u64(),
        Some(3),
        "the window is shown the plate with the supports: {lines:?}"
    );
}
