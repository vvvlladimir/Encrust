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
            "import_resin",
            "inspect_model",
            "list_profiles",
            "list_recipes",
            "render",
            "report_print",
            "revert",
            "save_recipe",
            "send_to_printer",
            "set_drains",
            "set_hollow",
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
async fn each_revision_keeps_the_steps_its_check_ran_and_hands_them_to_the_open_window() {
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

    let dir = scratch("window");
    let model = dir.join("cube.stl");
    write_stl(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), &model);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("bound").port();
    let file = dir.join("window.json");
    fs::write(&file, json!({"port": port, "token": "t"}).to_string()).expect("writable");
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
        // The way steps are decided is no edit, so the revision goes by its recipe.
        json!({"job": job, "recipe": "miniatures"}),
    )
    .await;
    assert_eq!(answer(&edited)["window"], json!({"shown": true}));
    client.cancel().await.expect("the server stops");

    let kept = fs::read_to_string(jobs.join(&job).join("2.recipe.toml"))
        .expect("the revision keeps its steps");
    let lines = window.await.expect("the window's task ends");
    assert_eq!(lines[1]["revision"], 2);
    assert_eq!(lines[1]["recipe"].as_str(), Some(kept.as_str()));
    assert!(
        kept.contains("Miniatures"),
        "the window runs the recipe's steps: {kept}"
    );
}

/// A window that answers each line by its kind for `lines` connections: a revision shown, the
/// plate — the job's latest one under `dir/jobs`, its model object 1 — at version 5, edits
/// applied; the lines read.
async fn plate_window(dir: &Path, lines: usize) -> (PathBuf, tokio::task::JoinHandle<Vec<Value>>) {
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("bound").port();
    let file = dir.join("window.json");
    fs::write(&file, json!({"port": port, "token": "t"}).to_string()).expect("writable");
    let jobs = dir.join("jobs");
    let window = tokio::spawn(async move {
        let mut read = Vec::new();
        for _ in 0..lines {
            let (stream, _) = listener.accept().await.expect("the server connects");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("a line");
            let line: Value = serde_json::from_str(&line).expect("JSON");
            let reply = if line["want"] == "plate" {
                let job = jobs.join(line["job"].as_str().expect("a job"));
                json!({"plate": latest_plate(&job), "version": 5, "objects": [1], "job_object": 1})
            } else if line.get("apply").is_some() {
                json!({"applied": true})
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
async fn a_project_the_window_saved_is_checked_drawn_and_written_as_it_stands() {
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
        failed(&hollowed),
        "a project's models are shaped in the window"
    );
    let inspected = call(&client, "inspect_model", json!({"path": project})).await;
    assert!(
        answer_text(&inspected).contains("start_job"),
        "inspect_model points a project to start_job: {}",
        answer_text(&inspected)
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
    fs::write(&file, json!({"port": port, "token": "t"}).to_string()).expect("writable");
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
