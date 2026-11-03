use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::recipe::Recipe;
use super::*;
use crate::camera::OrbitCamera;
use crate::prepare::{Choice, Orient};
use crate::undo::Frame;

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

/// The jobs polled in the window's order, then the link, then the frame recorded.
fn frame(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine, link: &mut Link) -> bool {
    let mut camera = OrbitCamera::default();
    let (scene, status) = (&mut doc.scene, &mut machine.status);
    let mut working = doc.imports.poll(scene, &doc.plate, &mut camera, status);
    working |= doc.repairs.poll(scene, status);
    working |= doc.drilling.poll(scene, status);
    working |= doc.surfaces.poll(scene, status);
    working |= tools.supports.poll(scene, status);
    working |= tools.hollow.poll(scene, status);
    working |= tools.drain.poll(scene, status);
    working |= tools.orient.poll(scene, &doc.plate, status);
    working |= crate::prepare::poll(doc, tools, machine, true);
    working |= link.poll(doc, tools, machine);
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

#[test]
fn a_revision_is_read_in_hollowed_on_the_plate_and_undone_as_one_edit() {
    let file = scratch("replay");
    let mut link = Link::start(file.clone(), || {}).expect("a loopback port is free");
    let (mut doc, mut tools, mut machine) = (Doc::default(), Tools::default(), Machine::default());
    machine.slicing.material.layer_height_mm = 0.5;
    let line = json!({"schema": 1, "token": token_of(&file), "job": "cube-1", "revision": 1,
                      "recipe": {"model": cube(), "hollow": {"wall_mm": 1.5}}});
    let server = std::thread::spawn(move || send(&file, &line));
    while !server.is_finished() {
        link.poll(&mut doc, &mut tools, &mut machine);
        std::thread::yield_now();
    }
    let reply = server.join().expect("the server's thread ends");
    assert_eq!(reply["shown"], true, "{reply}");

    let (doc, tools, machine) = (&mut doc, &mut tools, &mut machine);
    while frame(doc, tools, machine, &mut link) || doc.surfaces.is_running() {
        std::thread::yield_now();
    }
    let objects = doc.scene.objects();
    assert_eq!(objects.len(), 1, "the job's model was read in once");
    let wall_mm = objects[0].hollow.asked().map(|asked| asked.thickness_mm);
    assert_eq!(wall_mm, Some(1.5), "hollowed at the recipe's wall");
    assert!(
        objects[0].supports.is_empty(),
        "no supports asked, none placed"
    );
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
fn a_cut_and_a_relief_are_named_as_not_shown_and_the_scale_is_replayed() {
    let (plan, unshown) = recipe(json!({
        "model": "/m.stl",
        "modelling": {"scale": [2.0, 2.0, 2.0], "cut": {"axis": "z", "at_mm": 5.0}},
        "relief": {"depth_mm": 0.5},
    }))
    .plan();
    assert_eq!(plan.scale, Some(core_geometry::Vec3::splat(2.0)));
    assert_eq!(unshown.len(), 2, "the cut and the relief: {unshown:?}");
    assert!(unshown[0].starts_with("cut") && unshown[1].starts_with("relief"));
}
