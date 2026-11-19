//! Schema 3: the plate handed to the server, and edits applied while it is the version they
//! were built on (ADR 0466).

use std::sync::Arc;

use core_engine::project::HollowState;
use core_geometry::{Transform, Vec3, cuboid, diagnose, signed_volume, transform_mesh};

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

fn mend_of(object: ObjectId, base_version: u64, drop_faces: &[usize]) -> Value {
    json!({"base_version": base_version,
           "apply": [{"edit": "mend", "object": object.0, "drop_faces": drop_faces}]})
}

#[test]
fn a_mend_drops_the_faces_it_names_off_the_object_which_keeps_its_id() {
    let file = scratch("mend");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();

    let reply = bench.hand_over_running(&file, asking(&file, mend_of(cube, version, &[0, 1])));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench
        .doc
        .scene
        .get(cube)
        .expect("a mend keeps the object's id");
    assert_eq!(object.mesh.faces.len(), 10, "the cube's 12 faces less two");
}

#[test]
fn a_mend_naming_a_face_the_object_lacks_is_answered_with_why_and_leaves_the_cube() {
    let file = scratch("mend-refused");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();

    let reply = bench.hand_over_running(&file, asking(&file, mend_of(cube, version, &[13])));
    bench.run_to_end();

    assert_eq!(reply["applied"], false, "{reply}");
    let object = bench.doc.scene.get(cube).expect("the cube is still there");
    assert_eq!(object.mesh.faces.len(), 12);
}

fn relief_of(object: ObjectId, base_version: u64, image: &Path) -> Value {
    json!({"base_version": base_version,
           "apply": [{"edit": "relief", "object": object.0, "depth_mm": 0.5,
                      "image": image, "tile_mm": 5.0}]})
}

#[test]
fn a_relief_presses_a_picture_into_the_cube_which_keeps_its_id() {
    let file = scratch("relief");
    let picture = file.with_extension("png");
    let checker = image::GrayImage::from_fn(16, 16, |x, y| {
        image::Luma([((x / 8 + y / 8) % 2 * 255) as u8])
    });
    std::fs::create_dir_all(picture.parent().expect("the scratch file has a folder"))
        .expect("the scratch folder is made");
    checker.save(&picture).expect("the picture is written");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let before = signed_volume(&bench.doc.scene.get(cube).expect("the cube is there").mesh);

    let reply = bench.hand_over_running(&file, asking(&file, relief_of(cube, version, &picture)));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench
        .doc
        .scene
        .get(cube)
        .expect("a relief keeps the object's id");
    let after = signed_volume(&object.mesh);
    assert!(
        (after - before).abs() > 1.0,
        "a 0.5 mm checker moves more than 1 mm³ of the cube's {before}, got {after}"
    );
}

fn hollow_of(object: ObjectId, base_version: u64, state: &HollowState) -> Value {
    json!({"base_version": base_version,
           "apply": [{"edit": "hollow", "object": object.0, "settings": state}]})
}

#[test]
fn a_hollow_shells_the_cube_by_numbers_that_become_its_own_and_keeps_its_id() {
    let file = scratch("hollow");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let state = HollowState {
        thickness_mm: 1.0,
        ..bench.tools.hollow.state.clone()
    };

    let reply = bench.hand_over_running(&file, asking(&file, hollow_of(cube, version, &state)));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench
        .doc
        .scene
        .get(cube)
        .expect("a hollow keeps the object's id");
    assert!(object.hollow.shell().is_some(), "the cube has a shell now");
    assert_eq!(
        object.own.hollow.as_ref(),
        Some(&state),
        "the numbers are its own"
    );
}

#[test]
fn a_wall_as_thick_as_the_cube_is_answered_not_applied() {
    let file = scratch("hollow-no-room");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let state = HollowState {
        thickness_mm: 10.0,
        ..bench.tools.hollow.state.clone()
    };

    let reply = bench.hand_over_running(&file, asking(&file, hollow_of(cube, version, &state)));
    bench.run_to_end();

    assert_eq!(reply["applied"], false, "{reply}");
}

#[test]
fn a_drain_drills_a_hole_into_the_cube_at_once_and_keeps_its_id() {
    let file = scratch("drain");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let drain = json!({"base_version": version,
        "apply": [{"edit": "drain", "object": cube.0, "at_mm": [[5.0, 5.0, 10.5]]}]});

    let reply = bench.hand_over_running(&file, asking(&file, drain));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("a drain keeps the id");
    assert_eq!(object.hollow.drains().len(), 1, "one hole in the cube");
}

#[test]
fn supports_at_a_point_alone_stand_one_contact_on_the_surface_and_keep_the_id() {
    let file = scratch("supports-at");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let supports = json!({"base_version": version,
        "apply": [{"edit": "supports", "object": cube.0, "profile": "medium",
                   "at_mm": [[5.0, 5.0, -1.0]], "only_at": true}]});

    let reply = bench.hand_over_running(&file, asking(&file, supports));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("supports keep the id");
    let contacts = object.supports.contacts(object.transform);
    let [contact] = contacts[..] else {
        panic!("one contact placed, got {contacts:?}");
    };
    assert!(
        contact.abs_diff_eq(Vec3::new(5.0, 5.0, 0.0), 1e-3),
        "the contact is at (5, 5, 0) within 1e-3, got {contact}"
    );
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

#[test]
fn a_rotation_on_the_version_it_was_built_on_turns_the_object_about_z() {
    let file = scratch("rotate");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let turn = json!({"base_version": version,
        "apply": [{"edit": "rotate", "object": cube.0, "degrees": [0, 0, 90]}]});

    let reply = bench.hand_over_running(&file, asking(&file, turn));

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("the cube is still there");
    let (axis, angle) = object.transform.rotation.to_axis_angle();
    assert!(
        (angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4 && axis.dot(Vec3::Z) > 1.0 - 1e-4,
        "a quarter turn about Z, got {angle} about {axis}"
    );
}

#[test]
fn a_duplicate_on_the_version_it_was_built_on_puts_a_copy_beside_the_cube() {
    let file = scratch("duplicate");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let copy = json!({"base_version": version,
        "apply": [{"edit": "duplicate", "object": cube.0, "offset_mm": [20, 0, 0]}]});

    let reply = bench.hand_over_running(&file, asking(&file, copy));

    assert_eq!(reply["applied"], true, "{reply}");
    assert_eq!(bench.doc.scene.objects().len(), 2, "the cube and its copy");
    let xs: Vec<f32> = (bench.doc.scene.objects().iter())
        .map(|o| o.transform.translation.x)
        .collect();
    assert!(
        xs.contains(&0.0) && xs.contains(&20.0),
        "the copy stands 20 mm along x, got {xs:?}"
    );
}

#[test]
fn a_split_on_the_version_it_was_built_on_replaces_two_cubes_by_their_two_parts() {
    let file = scratch("split");
    let mut bench = Bench::new(file.clone());
    let mut mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    mesh.append(&cuboid(
        Vec3::new(15.0, 0.0, 0.0),
        Vec3::new(25.0, 10.0, 10.0),
    ));
    let summary = ImportSummary::made(diagnose(&mesh));
    let imported = Imported::new(
        "cubes".into(),
        Arc::new(mesh),
        Transform::default(),
        summary,
    );
    let cubes = bench.doc.scene.insert(imported);
    bench.run_to_end();
    let version = bench.doc.history.plate_version();
    let split = json!({"base_version": version,
        "apply": [{"edit": "split", "object": cubes.0}]});

    let reply = bench.hand_over_running(&file, asking(&file, split));

    assert_eq!(reply["applied"], true, "{reply}");
    assert!(bench.doc.scene.get(cubes).is_none(), "the pair is replaced");
    assert_eq!(bench.doc.scene.objects().len(), 2, "one object per cube");
}

#[test]
fn a_split_of_one_body_leaves_it_in_place_and_is_answered_applied() {
    let file = scratch("split-whole");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let split = json!({"base_version": version,
        "apply": [{"edit": "split", "object": cube.0}]});

    let reply = bench.hand_over_running(&file, asking(&file, split));

    assert_eq!(reply["applied"], true, "{reply}");
    assert!(
        bench.doc.scene.get(cube).is_some(),
        "the cube is left whole"
    );
}

#[test]
fn a_union_on_the_version_it_was_built_on_replaces_two_overlapping_cubes_by_one() {
    let file = scratch("combine");
    let mut bench = Bench::new(file.clone());
    let first = with_cube(&mut bench);
    let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let moved = Transform {
        translation: Vec3::new(5.0, 0.0, 0.0),
        ..Transform::default()
    };
    let summary = ImportSummary::made(diagnose(&mesh));
    let second = (bench.doc.scene).insert(Imported::new(
        "other".into(),
        Arc::new(mesh),
        moved,
        summary,
    ));
    bench.run_to_end();
    let version = bench.doc.history.plate_version();
    let union = json!({"base_version": version,
        "apply": [{"edit": "combine", "objects": [first.0, second.0], "op": "union"}]});

    let reply = bench.hand_over_running(&file, asking(&file, union));

    assert_eq!(reply["applied"], true, "{reply}");
    assert!(bench.doc.scene.get(first).is_none() && bench.doc.scene.get(second).is_none());
    let [united] = bench.doc.scene.objects() else {
        panic!("one object is left on the plate");
    };
    let bounds = united.world_bounds().expect("the union has faces");
    assert!(
        (bounds.maxs.x - 15.0).abs() < 1e-4,
        "the union spans x 0 to 15, got {}",
        bounds.maxs.x
    );
}

#[test]
fn a_label_on_the_version_it_was_built_on_raises_text_from_the_cube() {
    let file = scratch("label");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let label = json!({"base_version": version,
        "apply": [{"edit": "label", "object": cube.0, "text": "I",
                   "at_mm": [5, 5, 10], "depth_mm": 1.0}]});

    let reply = bench.hand_over_running(&file, asking(&file, label));

    assert_eq!(reply["applied"], true, "{reply}");
    assert!(bench.doc.scene.get(cube).is_none());
    let [lettered] = bench.doc.scene.objects() else {
        panic!("one object is left on the plate");
    };
    let volume = signed_volume(&transform_mesh(&lettered.mesh, lettered.transform));
    assert!(
        volume > 1_001.0,
        "the letter adds to the 1000 mm³ cube; got {volume}"
    );
}
