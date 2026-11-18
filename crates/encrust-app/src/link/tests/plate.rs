//! Schema 3: the plate handed to the server, and edits applied while it is the version they
//! were built on (ADR 0466).

use std::sync::Arc;

use core_geometry::{Transform, Vec3, cuboid, diagnose};

use super::*;
use crate::scene::{ImportSummary, Imported, ObjectId};

/// A 10 mm cube standing on the plate, recorded as the plate's first edit.
fn with_cube(bench: &mut Bench) -> ObjectId {
    let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let summary = ImportSummary::made(diagnose(&mesh));
    let imported = Imported::new("cube".into(), Arc::new(mesh), Transform::default(), summary);
    let id = bench.doc.scene.insert(imported);
    bench.run_to_end();
    id
}

fn asking(file: &Path, rest: Value) -> Value {
    let mut line = json!({"schema": 3, "token": found(file)["token"], "job": "cube-1"});
    if let (Some(line), Some(rest)) = (line.as_object_mut(), rest.as_object()) {
        line.extend(rest.clone());
    }
    line
}

fn cut_at(object: ObjectId, base_version: u64) -> Value {
    json!({"base_version": base_version,
           "apply": [{"edit": "cut", "object": object.0, "axis": "z", "at_mm": 5.0}]})
}

impl Bench {
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
}

#[test]
fn the_plate_is_handed_over_at_its_version_with_the_ids_the_window_knows_its_objects_by() {
    let file = scratch("plate");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);

    let reply = bench.hand_over_running(&file, asking(&file, json!({"want": "plate"})));

    assert_eq!(
        reply["version"],
        bench.doc.history.plate_version(),
        "{reply}"
    );
    let path = reply["plate"].as_str().expect("the plate's path");
    let project = core_engine::project::load(Path::new(path)).expect("the plate reads");
    let ids: Vec<ObjectId> = (project.manifest.objects.iter()).map(|o| o.id).collect();
    assert_eq!(ids, [cube]);
}

#[test]
fn a_cut_on_the_version_it_was_built_on_leaves_two_halves_which_one_undo_takes_back() {
    let file = scratch("apply");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();

    let reply = bench.hand_over_running(&file, asking(&file, cut_at(cube, version)));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    assert_eq!(bench.doc.scene.objects().len(), 2, "the cube's two halves");
    assert!(bench.doc.history.plate_version() > version);
    assert!(bench.undo());
    let names: Vec<&str> = (bench.doc.scene.objects().iter())
        .map(|o| o.name.as_str())
        .collect();
    assert_eq!(names, ["cube"], "one undo puts the cube back whole");
}

#[test]
fn edits_built_on_a_plate_since_changed_are_answered_stale_and_not_applied() {
    let file = scratch("stale");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let older = bench.doc.history.plate_version() - 1;

    let reply = bench.hand_over_running(&file, asking(&file, cut_at(cube, older)));

    assert_eq!(reply["stale"], true, "{reply}");
    assert_eq!(reply["version"], bench.doc.history.plate_version());
    assert_eq!(bench.doc.scene.objects().len(), 1);
}

#[test]
fn a_plate_asked_for_while_not_following_is_refused() {
    let file = scratch("plate-unfollowed");
    let mut bench = Bench::new(file.clone());
    bench.machine.general.follow_agents = false;

    let reply = bench.hand_over_running(&file, asking(&file, json!({"want": "plate"})));

    assert!(
        reply["why_not"]
            .as_str()
            .is_some_and(|why| why.contains("following")),
        "{reply}"
    );
}

#[test]
fn an_edit_that_does_not_land_is_answered_with_why_and_leaves_the_plate() {
    let file = scratch("missed");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let above = json!({"base_version": version,
        "apply": [{"edit": "cut", "object": cube.0, "axis": "z", "at_mm": 50.0}]});

    let reply = bench.hand_over_running(&file, asking(&file, above));

    assert_eq!(reply["applied"], false, "{reply}");
    assert!(
        reply["why_not"].as_str().is_some_and(|why| !why.is_empty()),
        "{reply}"
    );
    assert!(
        bench.doc.scene.get(cube).is_some(),
        "the cube is left whole"
    );
}

#[test]
fn edits_handed_over_while_others_still_land_are_refused() {
    let file = scratch("under-way");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let cut = |at_mm: f32| Line::Apply {
        edits: vec![core_edit::Edit::Cut {
            object: cube,
            axis: core_edit::Axis::Z,
            at_mm,
            keep: core_edit::Keep::Both,
            joints: core_edit::Joints::default(),
            lay_flat: false,
        }],
        base_version: version,
    };
    let (first, _first_answer) = std::sync::mpsc::channel();
    let (second, second_answer) = std::sync::mpsc::channel();
    let mut window = Window {
        mode: &mut bench.mode,
        tool: &mut bench.tool,
        doc: &mut bench.doc,
        view: &mut bench.view,
        tools: &mut bench.tools,
        machine: &mut bench.machine,
    };

    bench.link.answer(cut(5.0), first, true, &mut window);
    bench.link.answer(cut(3.0), second, true, &mut window);

    let refused = second_answer
        .try_recv()
        .expect("the second is answered at once");
    assert!(
        refused["why_not"]
            .as_str()
            .is_some_and(|why| why.contains("still applying")),
        "{refused}"
    );
}
