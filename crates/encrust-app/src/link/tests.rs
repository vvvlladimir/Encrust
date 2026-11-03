use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::recipe::Recipe;
use super::*;
use crate::camera::OrbitCamera;
use crate::panels::Window;
use crate::prepare::{Choice, Orient};
use crate::state::{Doc, Machine, Tools, View};
use crate::undo::Frame;
use crate::workspace::{Mode, Tool};

fn recipe(value: Value) -> Recipe {
    serde_json::from_value(value).expect("a recipe as the server writes it")
}

fn cube() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.stl")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-link-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("window.json")
}

/// Sends one line to the window `file` names and reads its answer.
fn send(file: &Path, line: &Value) -> Value {
    let found: Value =
        serde_json::from_str(&std::fs::read_to_string(file).expect("the window wrote its file"))
            .expect("JSON");
    let port = found["port"].as_u64().expect("a port") as u16;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("the window listens");
    writeln!(stream, "{line}").expect("the line is sent");
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .expect("an answer");
    serde_json::from_str(&reply).expect("the answer is JSON")
}

fn token_of(file: &Path) -> String {
    let found: Value =
        serde_json::from_str(&std::fs::read_to_string(file).expect("written")).expect("JSON");
    found["token"].as_str().expect("a token").to_owned()
}

#[test]
fn a_recipe_of_the_autopilot_plans_every_step_decided() {
    let (plan, unshown) = recipe(json!({"model": "/m.stl", "prepare": true})).plan();
    assert_eq!(plan, Plan::autopilot());
    assert!(unshown.is_empty());
}

#[test]
fn what_a_recipe_sets_is_given_and_what_it_leaves_out_is_skipped_without_prepare() {
    let (plan, _) = recipe(json!({
        "model": "/m.stl",
        "hollow": {"wall_mm": 1.5, "open_floor": true, "infill": "grid", "infill_density": 0.4},
        "supports": {"profile": "light", "cavity": true},
    }))
    .plan();
    let Choice::Given(hollow) = &plan.hollow else {
        panic!("the hollow is given: {plan:?}");
    };
    assert_eq!(hollow.thickness_mm, 1.5);
    assert!(hollow.open_floor && hollow.infill_on);
    assert_eq!(hollow.infill.pattern, core_volume::InfillPattern::Grid);
    assert_eq!(hollow.infill.density, 0.4);
    assert_eq!(plan.orient, Choice::Skip);
    assert_eq!(plan.repair, Choice::Skip);
    assert!(
        plan.drains.is_none(),
        "no drains asked and no autopilot to decide them"
    );
    assert_eq!(
        plan.supports.and_then(|stand| stand.profile),
        Some("light".to_owned())
    );
    assert!(plan.cavity);
}

#[test]
fn a_turn_is_given_as_the_command_line_turns_by_it() {
    let (plan, unshown) = recipe(json!({
        "model": "/m.stl",
        "pose": {"by": "rotate", "degrees": [90.0, 0.0, 30.0]},
    }))
    .plan();
    let turn = core_geometry::rotation_xyz_deg(core_geometry::Vec3::new(90.0, 0.0, 30.0));
    assert_eq!(plan.orient, Choice::Given(Orient::Turn(turn)));
    assert!(unshown.is_empty());
}

#[test]
fn a_pick_and_places_kept_clear_reach_the_search_with_the_turn_they_were_seen_in() {
    let (plan, unshown) = recipe(json!({
        "model": "/m.stl",
        "pose": {"by": "search", "goal": "auto", "pick": 2, "keep_clear_at_mm": [[1.0, 2.0, 3.0]],
                 "keep_clear_seen_deg": [0.0, 90.0, 0.0]},
    }))
    .plan();
    let Choice::Given(Orient::Search { wish, .. }) = plan.orient else {
        panic!("a search is given: {:?}", plan.orient);
    };
    assert_eq!(wish.pick, Some(2));
    let kept = wish.keep_clear.expect("the point is kept clear");
    assert_eq!(kept.at_mm, [core_geometry::Vec3::new(1.0, 2.0, 3.0)]);
    let seen = core_geometry::rotation_xyz_deg(core_geometry::Vec3::new(0.0, 90.0, 0.0));
    assert!(kept.seen.abs_diff_eq(seen, 1e-6));
    assert!(
        (kept.radius_mm - 4.0).abs() < 1e-6,
        "the command line's 4 mm"
    );
    assert!(unshown.is_empty());
}

#[test]
fn drain_points_are_drilled_and_a_missing_plug_is_named() {
    let (plan, unshown) = recipe(json!({
        "model": "/m.stl",
        "drains": {"diameter_mm": 4.0, "at_mm": [[1.0, 2.0, 3.0]], "auto": true, "vent": false},
    }))
    .plan();
    let drains = plan.drains.expect("drains asked");
    assert_eq!(drains.at_mm, [core_geometry::Vec3::new(1.0, 2.0, 3.0)]);
    assert!(drains.auto && !drains.vent);
    assert_eq!(drains.size.map(|size| size.diameter_mm), Some(4.0));
    assert_eq!(
        unshown.len(),
        1,
        "the window lays plugs anyway: {unshown:?}"
    );
}

#[test]
fn a_line_with_another_token_is_refused_and_never_reaches_the_window() {
    let file = scratch("token");
    let link = Link::start(file.clone(), || {}).expect("a loopback port is free");
    let reply = send(
        &file,
        &json!({"schema": 1, "token": "not-it", "job": "j", "revision": 1,
                "recipe": {"model": "/m.stl"}}),
    );
    assert_eq!(reply["shown"], false);
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

    /// Hands `recipe` over as revision `revision` of job `cube-1`, answering on frames.
    fn hand_over(&mut self, file: &Path, revision: u32, recipe: Value) -> Value {
        self.hand_over_job(file, "cube-1", revision, recipe)
    }

    /// Hands `recipe` over as revision `revision` of `job`, answering on frames.
    fn hand_over_job(&mut self, file: &Path, job: &str, revision: u32, recipe: Value) -> Value {
        let line = json!({"schema": 1, "token": token_of(file), "job": job,
                          "revision": revision, "recipe": recipe});
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
        working |= tools.supports.poll(scene, status);
        working |= tools.hollow.poll(scene, status);
        working |= tools.relief.poll(scene, status);
        working |= tools.boolean.poll(scene, status);
        working |= tools.text.poll(scene, status);
        working |= tools.cut.poll(scene, status);
        working |= tools.drain.poll(scene, status);
        working |= tools.orient.poll(scene, &doc.plate, status);
        working |= crate::prepare::poll(doc, tools, machine, true);
        working |= self.poll_link();
        let (doc, tools, machine) = (&mut self.doc, &mut self.tools, &mut self.machine);
        doc.surfaces.start(&mut doc.scene);
        tools.supports.settle_on_raft(&mut doc.scene);
        doc.drilling.start(&doc.scene, true);
        let settings = crate::tool_settings::ToolSettings::of(tools, &machine.slicing);
        let chosen = machine.slicing.chosen();
        let settled = Frame {
            settled: true,
            typing: false,
        };
        doc.history.observe(&doc.scene, &settings, chosen, settled);
        working
    }
}

#[test]
fn a_revision_is_read_in_hollowed_on_the_plate_and_undone_as_one_edit() {
    let file = scratch("replay");
    let mut bench = Bench::new(file.clone());
    let reply = bench.hand_over(
        &file,
        1,
        json!({"model": cube(), "hollow": {"wall_mm": 1.5}}),
    );
    assert_eq!(reply["shown"], true, "{reply}");
    bench.run_to_end();
    let objects = bench.doc.scene.objects();
    assert_eq!(objects.len(), 1, "the job's model was read in once");
    let wall_mm = objects[0].hollow.asked().map(|asked| asked.thickness_mm);
    assert_eq!(wall_mm, Some(1.5), "hollowed at the recipe's wall");
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
fn a_cut_lands_its_halves_and_the_next_revision_takes_them_back() {
    let file = scratch("cut");
    let mut bench = Bench::new(file.clone());
    let cut = json!({"model": cube(), "modelling": {"cut": {"axis": "z", "at_mm": 5.0}}});
    bench.hand_over(&file, 1, cut);
    bench.run_to_end();
    assert_eq!(bench.doc.scene.objects().len(), 2, "two halves of the cube");
    bench.hand_over(&file, 2, json!({"model": cube()}));
    bench.run_to_end();
    let objects = bench.doc.scene.objects();
    assert_eq!(objects.len(), 1, "the halves gone, the cube back whole");
}

#[test]
fn a_cut_a_relief_and_the_scale_are_replayed_with_nothing_unshown() {
    let (plan, unshown) = recipe(json!({
        "model": "/m.stl",
        "modelling": {"scale": [2.0, 2.0, 2.0], "cut": {"axis": "z", "at_mm": 5.0, "keep": "lower"}},
        "relief": {"depth_mm": 0.5},
    }))
    .plan();
    assert_eq!(plan.scale, Some(core_geometry::Vec3::splat(2.0)));
    let cut = plan.shape.cut.expect("the cut is replayed");
    assert_eq!(cut.keep, core_engine::project::Keep::Below);
    assert_eq!(plan.shape.relief.map(|relief| relief.depth_mm), Some(0.5));
    assert!(unshown.is_empty(), "{unshown:?}");
}

#[test]
fn a_revision_puts_the_window_on_its_printer_resin_and_layer_height() {
    let file = scratch("machine");
    let mut bench = Bench::new(file.clone());
    let resin = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../encrust-cli/tests/fixtures/test-resin.toml");
    bench.hand_over(
        &file,
        1,
        json!({"model": cube(), "printer": "elegoo-mars-4-ultra", "resin": resin,
               "slicing": {"layer_height_mm": 0.1}}),
    );
    bench.run_to_end();
    let slicing = &bench.machine.slicing;
    assert_eq!(slicing.printer_id.as_deref(), Some("elegoo-mars-4-ultra"));
    assert_eq!(slicing.material.name, "Test resin");
    assert!(
        (slicing.layer_height_mm() - 0.1).abs() < 1e-6,
        "0.1 mm layers as asked, found {}",
        slicing.layer_height_mm()
    );
}

/// Two 10 mm boxes 5 mm apart, as binary STL in a file of the test's own.
fn two_boxes(name: &str) -> PathBuf {
    let mut mesh =
        core_geometry::cuboid(core_geometry::Vec3::ZERO, core_geometry::Vec3::splat(10.0));
    mesh.append(&core_geometry::cuboid(
        core_geometry::Vec3::new(15.0, 0.0, 0.0),
        core_geometry::Vec3::new(25.0, 10.0, 10.0),
    ));
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
    let path = scratch(&format!("{name}-model")).with_file_name("boxes.stl");
    std::fs::create_dir_all(path.parent().expect("in a directory")).expect("writable");
    std::fs::write(&path, bytes).expect("writable");
    path
}

#[test]
fn a_split_puts_each_body_on_the_plate_as_a_model_of_its_own() {
    let file = scratch("split");
    let mut bench = Bench::new(file.clone());
    let recipe = json!({"model": two_boxes("split"), "modelling": {"split_smallest_mm3": 1.0}});
    bench.hand_over(&file, 1, recipe);
    bench.run_to_end();
    assert_eq!(bench.doc.scene.objects().len(), 2, "one model a box");
}

#[test]
fn a_shell_dropped_by_hand_leaves_the_other() {
    let file = scratch("mend");
    let mut bench = Bench::new(file.clone());
    let recipe = json!({"model": two_boxes("mend"), "mend": {"drop_shells": [2]}});
    bench.hand_over(&file, 1, recipe);
    bench.run_to_end();
    let objects = bench.doc.scene.objects();
    assert_eq!(objects.len(), 1);
    assert_eq!(
        objects[0].mesh.faces.len(),
        12,
        "one box's twelve faces left"
    );
}

#[test]
fn a_combine_reads_the_other_model_in_and_leaves_one() {
    let file = scratch("combine");
    let mut bench = Bench::new(file.clone());
    let other = two_boxes("combine");
    let recipe =
        json!({"model": cube(), "modelling": {"combine": {"op": "union", "with": [other]}}});
    bench.hand_over(&file, 1, recipe);
    bench.run_to_end();
    assert_eq!(
        bench.doc.scene.objects().len(),
        1,
        "the two files joined into one model"
    );
    let bounds = bench.doc.scene.objects()[0]
        .world_bounds()
        .expect("a model with faces");
    let wide_mm = bounds.maxs.x - bounds.mins.x;
    assert!(
        wide_mm > 20.0,
        "the union reaches across both boxes, 25 mm, found {wide_mm}"
    );
}

#[test]
fn a_second_job_waits_for_the_first_jobs_replay_instead_of_undoing_it() {
    let file = scratch("two-jobs");
    let mut bench = Bench::new(file.clone());
    let supported = json!({"model": cube(), "pose": {"by": "rotate", "degrees": [35.0, 35.0, 0.0]},
                           "supports": {"profile": "auto"}});
    bench.hand_over_job(&file, "cube-1", 1, supported);
    while !bench.tools.prepare.is_running() {
        bench.frame();
    }
    let cut = json!({"model": two_boxes("two-jobs"),
                     "modelling": {"cut": {"axis": "z", "at_mm": 5.0}}});
    bench.hand_over_job(&file, "boxes-1", 1, cut);
    bench.run_to_end();
    let objects = bench.doc.scene.objects();
    assert_eq!(objects.len(), 3, "the cube and the two halves of the boxes");
    assert!(
        objects.iter().any(|object| !object.supports.is_empty()),
        "the first job's cube still stands on its supports"
    );
}
