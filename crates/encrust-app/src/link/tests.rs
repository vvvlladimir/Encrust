use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use core_engine::project::save;
use core_geometry::{Vec3, transform_mesh};
use serde_json::{Value, json};

use super::*;
use crate::camera::OrbitCamera;
use crate::project::state::{Captured, capture};
use crate::scene::fixtures::scene_of_boxes;
use crate::slicing::Slicing;
use crate::state::{Doc, Machine, Tools, View};
use crate::tool_settings::ToolSettings;
use crate::undo::Frame;
use crate::workspace::{Mode, Tool};

/// A plate of one 10 mm box with its low corner at `low`, written as the server keeps a
/// revision's, at `path`.
fn plate_at(path: &Path, low: Vec3) -> PathBuf {
    write_plate(&scene_of_boxes(&[(low, low + Vec3::splat(10.0))]), path);
    path.to_owned()
}

/// `scene` written as the server keeps a revision's plate, at `path`.
fn write_plate(scene: &crate::scene::Scene, path: &Path) {
    let (slicing, tools) = (Slicing::default(), Tools::default());
    let plate = capture(Captured {
        scene,
        slicing: &slicing,
        tools: &tools,
    });
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("writable");
    save(path, &plate).expect("the plate is written");
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

/// Revision `revision` of job `cube-1` shown by `plate`, as `encrust-mcp` writes it.
fn line(file: &Path, revision: u32, plate: &Path) -> Value {
    json!({"schema": agent_link::SHOW_SCHEMA, "token": found(file)["token"], "job": "cube-1",
           "revision": revision, "model": cube(), "printer": "", "resin": "", "plate": plate})
}

/// The plate of revision `revision` with its box at `x` mm, beside the window's file.
fn revision_plate(file: &Path, revision: u32, x: f32) -> PathBuf {
    let path = file.with_file_name(format!("plates/{revision}-{x}.encrust"));
    plate_at(&path, Vec3::new(x, 0.0, 0.0))
}

#[test]
fn a_line_with_another_token_is_refused_and_never_reaches_the_window() {
    let file = scratch("token");
    let link = Link::start(file.clone(), || {}).expect("a loopback port is free");
    let mut refused = line(&file, 1, &revision_plate(&file, 1, 0.0));
    refused["token"] = json!("not-it");
    assert_eq!(send(&file, &refused)["shown"], false);
    assert!(
        link.listening.asked.try_recv().is_err(),
        "nothing was handed on"
    );
    drop(link);
    assert!(!file.exists(), "a closed window takes its file away");
}

#[test]
fn a_line_is_answered_while_another_still_waits_for_the_window() {
    let file = scratch("two-at-once");
    let _link = Link::start(file.clone(), || {}).expect("a loopback port is free");
    let port = found(&file)["port"].as_u64().expect("a port") as u16;
    let mut waiting = TcpStream::connect(("127.0.0.1", port)).expect("the window listens");
    let plate = json!({"schema": 3, "token": found(&file)["token"], "job": "cube-1",
                       "want": "plate"});
    writeln!(waiting, "{plate}").expect("the line is sent");

    let mut refused = line(&file, 1, &revision_plate(&file, 1, 0.0));
    refused["token"] = json!("not-it");
    let started = std::time::Instant::now();
    let answer = send(&file, &refused);

    assert_eq!(answer["shown"], false, "{answer}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(3),
        "answered without waiting on the first line, in {:?}",
        started.elapsed()
    );
}

#[test]
fn a_revision_is_taken_before_its_plate_is_read() {
    let file = scratch("taken-unread");
    let mut bench = Bench::new(file.clone());
    let plate = file.with_file_name("plates/broken.encrust");
    std::fs::create_dir_all(plate.parent().expect("a folder")).expect("writable");
    std::fs::write(&plate, b"not a project").expect("writable");

    let answer = bench.hand_over(&file, line(&file, 1, &plate));
    bench.run_to_end();

    assert_eq!(answer, json!({"shown": true}));
    assert!(
        bench.machine.status.is_error(),
        "the plate that does not read is told"
    );
    assert!(bench.doc.scene.objects().is_empty());
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

    /// Runs `ask` on the window the bench holds.
    fn with_window(&mut self, ask: impl FnOnce(&mut Window<'_>)) {
        ask(&mut Window {
            mode: &mut self.mode,
            tool: &mut self.tool,
            doc: &mut self.doc,
            view: &mut self.view,
            tools: &mut self.tools,
            machine: &mut self.machine,
        });
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

    /// Hands `line` over, running whole frames, so a tool's job lands meanwhile.
    fn hand_over_running(&mut self, file: &Path, line: Value) -> Value {
        let file = file.to_owned();
        let server = std::thread::spawn(move || send(&file, &line));
        while !server.is_finished() {
            self.frame();
            std::thread::yield_now();
        }
        server.join().expect("the server's thread ends")
    }

    fn undo(&mut self) -> bool {
        let (doc, tools, machine) = (&mut self.doc, &mut self.tools, &mut self.machine);
        doc.history
            .undo(&mut doc.scene, tools, &mut machine.slicing)
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
        let landing = crate::drain::land_ended(doc, &mut tools.drain, &mut machine.status);
        let (scene, status) = (&mut doc.scene, &mut machine.status);
        let mut working = doc.imports.poll(scene, &doc.plate, &mut camera, status);
        working |= doc.repairs.poll(scene, status);
        working |= landing;
        working |= doc.drilling.poll(scene, status);
        working |= doc.surfaces.poll(scene, status);
        working |= tools.hollow.poll(scene, status);
        working |= tools.drain.poll(scene, status);
        working |= tools.supports.poll(scene, status);
        working |= tools.orient.poll(scene, &doc.plate, status);
        working |= tools.cut.poll(scene, status);
        working |= tools.relief.poll(scene, status);
        working |= tools.boolean.poll(scene, status);
        working |= tools.text.poll(scene, status);
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

    /// Where each box on the plate starts along x, mm, in plate order.
    fn lows_x(&self) -> Vec<f32> {
        (self.doc.scene.objects().iter())
            .filter_map(|object| transform_mesh(&object.mesh, object.transform).aabb())
            .map(|bounds| bounds.mins.x)
            .collect()
    }
}

#[test]
fn a_plate_that_does_not_read_is_refused_with_why() {
    let file = scratch("unread");
    let mut bench = Bench::new(file.clone());
    let missing = file.with_file_name("no-such.encrust");
    let reply = bench.hand_over(&file, line(&file, 1, &missing));
    assert_eq!(reply["shown"], false);
    assert!(
        reply["why_not"]
            .as_str()
            .is_some_and(|why| why.contains("plate")),
        "{reply}"
    );
}

#[test]
fn a_revision_stands_on_a_plate_of_its_own_and_is_undone_as_one_edit() {
    let file = scratch("shown");
    let mut bench = Bench::new(file.clone());
    bench.doc.scene = scene_of_boxes(&[(Vec3::new(50.0, 0.0, 0.0), Vec3::splat(60.0))]);
    bench.run_to_end();
    let plate = revision_plate(&file, 1, 0.0);
    assert_eq!(
        bench.hand_over(&file, line(&file, 1, &plate)),
        json!({"shown": true})
    );
    bench.run_to_end();
    let scene = &bench.doc.scene;
    assert_eq!(scene.plates().len(), 2, "the job's plate beside the user's");
    assert_eq!(scene.active_plate(), 1, "the job's plate is shown");
    assert_eq!(scene.plates()[1], "cube", "named for the job's model");
    assert_eq!(scene.on_plate(0).count(), 1, "the user's box is left alone");
    assert_eq!(bench.lows_x(), [50.0, 0.0]);
    assert!(bench.undo());
    assert_eq!(bench.lows_x(), [50.0], "one undo takes the revision off");
    assert_eq!(bench.doc.scene.plates().len(), 1, "and its plate");
}

#[test]
fn the_server_is_handed_the_jobs_plate_alone() {
    let file = scratch("plate-alone");
    let mut bench = Bench::new(file.clone());
    bench.doc.scene = scene_of_boxes(&[(Vec3::new(50.0, 0.0, 0.0), Vec3::splat(60.0))]);
    bench.run_to_end();
    bench.hand_over(&file, line(&file, 1, &revision_plate(&file, 1, 0.0)));
    bench.run_to_end();
    let job = bench
        .doc
        .scene
        .on_plate(1)
        .next()
        .expect("the job's box")
        .id;
    bench.doc.scene.show_plate(0);

    let asked = json!({"schema": 3, "token": found(&file)["token"], "job": "cube-1",
                       "want": "plate"});
    let reply = bench.hand_over(&file, asked);

    assert_eq!(reply["objects"], json!([job.0]), "{reply}");
    assert_eq!(reply["job_object"], json!(job.0), "{reply}");
}

#[test]
fn a_revision_shown_after_a_cut_made_by_edits_stands_in_place_of_the_halves() {
    let file = scratch("after-cut");
    let mut bench = Bench::new(file.clone());
    bench.hand_over(&file, line(&file, 1, &revision_plate(&file, 1, 0.0)));
    bench.run_to_end();
    let cube = bench.doc.scene.here().next().expect("the job's box").id;
    let cut = json!({"schema": 3, "token": found(&file)["token"], "job": "cube-1",
        "base_version": bench.doc.history.plate_version(),
        "apply": [{"edit": "cut", "object": cube.0, "axis": "z", "at_mm": 5.0}]});
    let applied = bench.hand_over_running(&file, cut);
    bench.run_to_end();
    assert_eq!(applied["applied"], true, "{applied}");
    assert_eq!(bench.doc.scene.objects().len(), 2, "the two halves");

    bench.hand_over(&file, line(&file, 3, &revision_plate(&file, 3, 20.0)));
    bench.run_to_end();

    assert_eq!(bench.lows_x(), [20.0], "revision 3 alone, the halves gone");
}

#[test]
fn the_next_revision_stands_in_place_of_the_last() {
    let file = scratch("again");
    let mut bench = Bench::new(file.clone());
    bench.hand_over(&file, line(&file, 1, &revision_plate(&file, 1, 0.0)));
    bench.run_to_end();
    bench.hand_over(&file, line(&file, 2, &revision_plate(&file, 2, 20.0)));
    bench.run_to_end();
    assert_eq!(
        bench.lows_x(),
        [20.0],
        "revision 2 alone, where its plate stands it"
    );
}

#[test]
fn the_next_revision_takes_the_plugs_laid_for_the_last_off_with_it() {
    let file = scratch("again-plugs");
    let mut bench = Bench::new(file.clone());
    bench.hand_over(&file, line(&file, 1, &revision_plate(&file, 1, 0.0)));
    bench.run_to_end();
    let shown = bench.doc.scene.objects()[0].id;
    let plug = (bench.doc.scene.duplicate(shown, Vec3::new(40.0, 0.0, 0.0)))
        .expect("the revision's box copies");
    bench
        .doc
        .scene
        .get_mut(plug)
        .expect("the copy is there")
        .plug_of = Some(shown);

    bench.hand_over(&file, line(&file, 2, &revision_plate(&file, 2, 20.0)));
    bench.run_to_end();

    assert_eq!(
        bench.lows_x(),
        [20.0],
        "revision 2 alone, the plug laid for revision 1 gone with it"
    );
}

#[test]
fn a_revision_handed_over_while_not_following_is_listed_only_unless_marked_open() {
    let file = scratch("not-following");
    let mut bench = Bench::new(file.clone());
    bench.machine.general.follow_agents = false;
    let plate = revision_plate(&file, 1, 0.0);
    let reply = bench.hand_over(&file, line(&file, 1, &plate));
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

    let mut asked = line(&file, 2, &plate);
    asked["open"] = json!(true);
    assert_eq!(bench.hand_over(&file, asked), json!({"shown": true}));
    bench.run_to_end();
    assert_eq!(bench.lows_x(), [0.0]);
}

#[test]
fn a_revision_the_user_asks_for_is_read_from_the_servers_jobs_and_shown() {
    let file = scratch("asked");
    let job = file.with_file_name("mcp-jobs").join("cube-1");
    std::fs::create_dir_all(&job).expect("writable");
    let flags = json!([cube(), "--printer=", "--resin="]).to_string();
    std::fs::write(job.join("3.args.json"), &flags).expect("writable");
    plate_at(&job.join("3.encrust"), Vec3::new(5.0, 0.0, 0.0));
    let mut bench = Bench::new(file);
    bench.machine.general.follow_agents = false;
    let ask = |revision| crate::agents::Asked {
        job: "cube-1".into(),
        revision,
    };
    bench.machine.agents.asked.push(ask(3));
    bench.run_to_end();
    assert_eq!(bench.lows_x(), [5.0]);
    assert_eq!(
        bench.doc.scene.plates()[1],
        "cube",
        "on the job's own plate"
    );

    bench.machine.agents.asked.push(ask(9));
    bench.run_to_end();
    assert!(
        matches!(bench.machine.status, Status::Error(_)),
        "a revision not kept is said, not shown"
    );

    std::fs::write(job.join("4.args.json"), &flags).expect("writable");
    std::fs::write(job.join("4.edits.json"), "[]").expect("writable");
    bench.machine.status = Status::default();
    bench.machine.agents.asked.push(ask(4));
    bench.run_to_end();
    assert!(
        matches!(&bench.machine.status, Status::Error(why)
            if *why == tr!("link-revision-on-plate", job = "cube-1", revision = 4)),
        "a revision made on this plate is there already: {:?}",
        bench.machine.status
    );

    let cube = bench.doc.scene.plates().iter().position(|p| p == "cube");
    let cube = u32::try_from(cube.expect("the job's plate")).expect("few plates");
    bench.doc.scene.remove_plate(cube);
    plate_at(&job.join("4.encrust"), Vec3::new(7.0, 0.0, 0.0));
    bench.machine.status = Status::default();
    bench.machine.agents.asked.push(ask(4));
    bench.run_to_end();
    assert_eq!(
        bench.lows_x(),
        [7.0],
        "a revision made on a plate since closed is shown again"
    );
}

#[test]
fn a_second_job_of_the_same_model_takes_the_first_ones_place_on_the_plate() {
    let file = scratch("same-model");
    let mut bench = Bench::new(file.clone());
    let first = line(&file, 1, &revision_plate(&file, 1, 0.0));
    assert_eq!(bench.hand_over(&file, first), json!({"shown": true}));
    bench.run_to_end();
    let mut second = line(&file, 1, &revision_plate(&file, 1, 30.0));
    second["job"] = json!("cube-2");
    assert_eq!(bench.hand_over(&file, second), json!({"shown": true}));
    bench.run_to_end();
    assert_eq!(bench.lows_x(), [30.0], "one model, shown by the second job");
    assert_eq!(
        bench.doc.scene.plates().len(),
        2,
        "on the one plate of the model"
    );
}

mod plate;
