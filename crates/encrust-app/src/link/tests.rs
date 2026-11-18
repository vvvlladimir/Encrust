use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::*;
use crate::camera::OrbitCamera;
use crate::state::{Doc, Machine, Tools, View};
use crate::tool_settings::ToolSettings;
use crate::undo::Frame;
use crate::workspace::{Mode, Tool};

/// A hollow step alone, at a wall `wall_mm` thick.
fn hollowing(wall_mm: f32) -> String {
    format!(
        "name = \"r\"\nformat = 1\n\n[[step]]\ndo = \"hollow\"\nset = {{ wall_mm = {wall_mm} }}\n"
    )
}

fn cube() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.stl")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-link-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("window.json")
}

fn found(file: &Path) -> Value {
    let text = std::fs::read_to_string(file).expect("the window wrote its file");
    serde_json::from_str(&text).expect("JSON")
}

/// Sends one line to the window `file` names and reads its answer.
fn send(file: &Path, line: &Value) -> Value {
    let port = found(file)["port"].as_u64().expect("a port") as u16;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("the window listens");
    writeln!(stream, "{line}").expect("the line is sent");
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .expect("an answer");
    serde_json::from_str(&reply).expect("the answer is JSON")
}

/// Revision `revision` of job `cube-1` with `recipe`, as `encrust-mcp` writes it.
fn line(file: &Path, revision: u32, recipe: &str) -> Value {
    json!({"schema": request::SCHEMA, "token": found(file)["token"], "job": "cube-1",
           "revision": revision, "model": cube(), "printer": "", "resin": "", "recipe": recipe})
}

#[test]
fn a_line_with_another_token_is_refused_and_never_reaches_the_window() {
    let file = scratch("token");
    let link = Link::start(file.clone(), || {}).expect("a loopback port is free");
    let mut refused = line(&file, 1, &hollowing(2.0));
    refused["token"] = json!("not-it");
    assert_eq!(send(&file, &refused)["shown"], false);
    assert!(
        link.listening.asked.try_recv().is_err(),
        "nothing was handed on"
    );
    drop(link);
    assert!(!file.exists(), "a closed window takes its file away");
}

/// The window's state, run a frame at a time as the window runs it.
struct Bench {
    mode: Mode,
    tool: Tool,
    doc: Doc,
    view: View,
    tools: Tools,
    machine: Machine,
    link: Link,
}

impl Bench {
    fn new(file: PathBuf) -> Self {
        let doc = Doc::default();
        let mut machine = Machine::default();
        machine.slicing.material.layer_height_mm = 0.5;
        machine.general.follow_agents = true;
        Self {
            mode: Mode::default(),
            tool: Tool::default(),
            view: View::framing(&doc.plate),
            doc,
            tools: Tools::default(),
            machine,
            link: Link::start(file, || {}).expect("a loopback port is free"),
        }
    }

    fn poll_link(&mut self) -> bool {
        let mut window = Window {
            mode: &mut self.mode,
            tool: &mut self.tool,
            doc: &mut self.doc,
            view: &mut self.view,
            tools: &mut self.tools,
            machine: &mut self.machine,
        };
        self.link.poll(&mut window)
    }

    /// Hands `line` over, answering on frames, and the window's answer.
    fn hand_over(&mut self, file: &Path, line: Value) -> Value {
        let file = file.to_owned();
        let server = std::thread::spawn(move || send(&file, &line));
        while !server.is_finished() {
            self.poll_link();
            std::thread::yield_now();
        }
        server.join().expect("the server's thread ends")
    }

    /// Frames until nothing runs.
    fn run_to_end(&mut self) {
        while self.frame() || self.doc.surfaces.is_running() || self.doc.drilling.is_running() {
            std::thread::yield_now();
        }
    }

    /// The jobs polled in the window's order, then the link, then the frame recorded.
    fn frame(&mut self) -> bool {
        let mut camera = OrbitCamera::default();
        let (doc, tools, machine) = (&mut self.doc, &mut self.tools, &mut self.machine);
        let (scene, status) = (&mut doc.scene, &mut machine.status);
        let mut working = doc.imports.poll(scene, &doc.plate, &mut camera, status);
        working |= doc.repairs.poll(scene, status);
        working |= doc.drilling.poll(scene, status);
        working |= doc.surfaces.poll(scene, status);
        working |= tools.hollow.poll(scene, status);
        working |= tools.drain.poll(scene, status);
        working |= tools.supports.poll(scene, status);
        working |= tools.orient.poll(scene, &doc.plate, status);
        working |= crate::prepare::poll(doc, tools, machine, true);
        working |= self.poll_link();
        let (doc, tools, machine) = (&mut self.doc, &mut self.tools, &mut self.machine);
        doc.drilling.start(&doc.scene, true);
        let settings = ToolSettings::of(tools, &machine.slicing);
        let chosen = machine.slicing.chosen();
        let settled = Frame {
            settled: true,
            typing: false,
        };
        doc.history.observe(&doc.scene, &settings, chosen, settled);
        working
    }

    fn wall_mm(&self) -> Option<f32> {
        let objects = self.doc.scene.objects();
        assert_eq!(objects.len(), 1, "the job's model was read in once");
        objects[0].hollow.asked().map(|asked| asked.thickness_mm)
    }
}

#[test]
fn a_recipe_that_does_not_read_is_refused_with_why() {
    let file = scratch("unread");
    let mut bench = Bench::new(file.clone());
    let reply = bench.hand_over(&file, line(&file, 1, "do = \"nothing\""));
    assert_eq!(reply["shown"], false);
    assert!(
        reply["why_not"]
            .as_str()
            .is_some_and(|why| why.contains("recipe")),
        "{reply}"
    );
}

#[test]
fn a_revision_runs_its_recipe_on_the_model_read_in_and_is_undone_as_one_edit() {
    let file = scratch("replay");
    let mut bench = Bench::new(file.clone());
    let reply = bench.hand_over(&file, line(&file, 1, &hollowing(1.5)));
    assert_eq!(reply, json!({"shown": true}));
    bench.run_to_end();
    assert_eq!(bench.wall_mm(), Some(1.5), "hollowed at the recipe's wall");
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    assert!(
        doc.history
            .undo(&mut doc.scene, tools, &mut machine.slicing)
    );
    let objects = doc.scene.objects();
    assert!(
        objects.len() == 1 && !objects[0].hollow.is_hollow(),
        "one undo takes the revision off and leaves the model as it came in"
    );
}

#[test]
fn the_next_revision_starts_again_from_the_model_as_it_came_in() {
    let file = scratch("again");
    let mut bench = Bench::new(file.clone());
    bench.hand_over(&file, line(&file, 1, &hollowing(1.5)));
    bench.run_to_end();
    bench.hand_over(&file, line(&file, 2, "name = \"r\"\nformat = 1\n"));
    bench.run_to_end();
    assert_eq!(
        bench.wall_mm(),
        None,
        "a recipe of no steps leaves the cube solid"
    );
}

#[test]
fn a_revision_handed_over_while_not_following_is_listed_only_unless_marked_open() {
    let file = scratch("not-following");
    let mut bench = Bench::new(file.clone());
    bench.machine.general.follow_agents = false;
    let reply = bench.hand_over(&file, line(&file, 1, &hollowing(1.5)));
    assert_eq!(reply["shown"], false);
    assert!(
        reply["why_not"]
            .as_str()
            .is_some_and(|why| why.contains("not following")),
        "{reply}"
    );
    bench.run_to_end();
    assert!(
        bench.doc.scene.objects().is_empty(),
        "the plate is left alone"
    );

    let mut asked = line(&file, 2, &hollowing(1.5));
    asked["open"] = json!(true);
    assert_eq!(bench.hand_over(&file, asked), json!({"shown": true}));
    bench.run_to_end();
    assert_eq!(bench.wall_mm(), Some(1.5));
}

#[test]
fn a_revision_the_user_asks_for_is_read_from_the_servers_jobs_and_shown() {
    let file = scratch("asked");
    let job = file.with_file_name("mcp-jobs").join("cube-1");
    std::fs::create_dir_all(&job).expect("writable");
    let flags = json!([cube(), "--printer=", "--resin="]).to_string();
    std::fs::write(job.join("3.args.json"), flags).expect("writable");
    std::fs::write(job.join("3.recipe.toml"), hollowing(1.5)).expect("writable");
    let mut bench = Bench::new(file);
    bench.machine.general.follow_agents = false;
    bench.machine.agents.asked.push(crate::agents::Asked {
        job: "cube-1".into(),
        revision: 3,
    });
    bench.run_to_end();
    assert_eq!(bench.wall_mm(), Some(1.5));
    assert_eq!(bench.machine.agents.on_plate, ["cube-1"]);

    bench.machine.agents.asked.push(crate::agents::Asked {
        job: "cube-1".into(),
        revision: 9,
    });
    bench.run_to_end();
    assert!(
        matches!(bench.machine.status, Status::Error(_)),
        "a revision not kept is said, not shown"
    );
}

#[test]
fn a_second_job_of_the_same_model_takes_the_first_ones_place_on_the_plate() {
    let file = scratch("same-model");
    let mut bench = Bench::new(file.clone());
    assert_eq!(
        bench.hand_over(&file, line(&file, 1, &hollowing(1.5))),
        json!({"shown": true})
    );
    bench.run_to_end();
    let mut second = line(&file, 1, &hollowing(2.5));
    second["job"] = json!("cube-2");
    assert_eq!(bench.hand_over(&file, second), json!({"shown": true}));
    bench.run_to_end();
    assert_eq!(
        bench.wall_mm(),
        Some(2.5),
        "one model, run by the second job"
    );
    assert_eq!(bench.machine.agents.on_plate, ["cube-2"]);
}
