//! Schema 3: the plate handed to the server, and edits applied while it is the version they
//! were built on (ADR 0466).

use std::sync::Arc;

use core_engine::project::HollowState;
use core_geometry::{Transform, Vec3, cuboid, diagnose, signed_volume, transform_mesh};
use core_volume::HoleSize;

use super::*;
use crate::scene::{ImportSummary, Imported, ObjectId};
use crate::supports::SupportGroup;
use printer_profiles::SupportProfile;

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
fn a_drain_on_a_scaled_cube_drills_a_hole_of_the_size_asked_on_the_plate() {
    let file = scratch("drain-scaled");
    let mut bench = Bench::new(file.clone());
    let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let summary = ImportSummary::made(diagnose(&mesh));
    let scaled = Transform {
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };
    let cube = bench.doc.scene.insert(Imported::new(
        "cube".into(),
        Arc::new(mesh),
        scaled,
        summary,
    ));
    bench.run_to_end();
    let version = bench.doc.history.plate_version();
    let size = HoleSize {
        diameter_mm: 3.0,
        ..HoleSize::default()
    };
    let drain = json!({"base_version": version,
        "apply": [{"edit": "drain", "object": cube.0, "size": size,
                   "at_mm": [[10.0, 10.0, 20.0]]}]});

    let reply = bench.hand_over_running(&file, asking(&file, drain));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("a drain keeps the id");
    let [hole] = object.hollow.drains() else {
        panic!("one hole in the cube, got {:?}", object.hollow.drains());
    };
    let on_plate_mm = hole.diameter_mm * object.transform.least_scale();
    assert!(
        (on_plate_mm - 3.0).abs() < 1e-4,
        "the hole measures 3 mm on the plate, got {on_plate_mm}"
    );
}

#[test]
fn a_drain_takes_away_the_supports_it_drills_out_and_leaves_the_rest() {
    let file = scratch("drain-supports");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let stood = json!({"base_version": version,
        "apply": [{"edit": "move", "object": cube.0, "by_mm": [0.0, 0.0, 5.0]},
                  {"edit": "supports", "object": cube.0, "profile": "medium",
                   "at_mm": [[5.0, 5.0, 4.0], [1.0, 1.0, 4.0]], "only_at": true}]});
    bench.hand_over_running(&file, asking(&file, stood));
    bench.run_to_end();
    let version = bench.doc.history.plate_version();
    let size = HoleSize {
        diameter_mm: 3.0,
        ..HoleSize::default()
    };
    let drain = json!({"base_version": version,
        "apply": [{"edit": "drain", "object": cube.0, "size": size,
                   "at_mm": [[5.0, 5.0, 5.0]]}]});

    let reply = bench.hand_over_running(&file, asking(&file, drain));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("a drain keeps the id");
    let contacts = object.supports.contacts(object.transform);
    let [contact] = contacts[..] else {
        panic!("one of two contacts is left, got {contacts:?}");
    };
    assert!(
        contact.x < 2.0,
        "the contact at (1, 1) stays, got {contact}"
    );
}

#[test]
fn an_auto_drain_searches_a_hollow_cube_and_keeps_the_id() {
    let file = scratch("drain-auto");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let state = HollowState {
        thickness_mm: 1.0,
        ..bench.tools.hollow.state.clone()
    };
    let both = json!({"base_version": version,
        "apply": [{"edit": "hollow", "object": cube.0, "settings": state},
                  {"edit": "drain", "object": cube.0, "auto": true, "vent": true}]});

    let reply = bench.hand_over_running(&file, asking(&file, both));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("a drain keeps the id");
    assert!(
        !object.hollow.drains().is_empty(),
        "the search drilled holes"
    );
}

#[test]
fn edits_left_when_the_user_moves_the_model_under_a_running_hollow_are_refused() {
    let file = scratch("moved-under");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let state = HollowState {
        thickness_mm: 1.0,
        ..bench.tools.hollow.state.clone()
    };
    let both = json!({"base_version": version,
        "apply": [{"edit": "hollow", "object": cube.0, "settings": state},
                  {"edit": "drain", "object": cube.0, "auto": true}]});
    let line = asking(&file, both);
    let server = {
        let file = file.clone();
        std::thread::spawn(move || send(&file, &line))
    };
    let mut moved = false;
    while !server.is_finished() {
        bench.frame();
        if !moved && bench.tools.hollow.job.is_some() {
            let object = bench.doc.scene.get_mut(cube).expect("the cube is there");
            object.transform.translation.x += 5.0;
            moved = true;
        }
        std::thread::yield_now();
    }
    let reply = server.join().expect("the server's thread ends");
    bench.run_to_end();

    assert!(moved, "the hollow ran for a frame at least");
    assert_eq!(reply["applied"], false, "{reply}");
    assert!(
        reply["why_not"]
            .as_str()
            .is_some_and(|why| why.contains("changed")),
        "{reply}"
    );
}

#[test]
fn an_auto_drain_on_a_solid_cube_is_refused_for_want_of_a_cavity() {
    let file = scratch("drain-solid");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let drain = json!({"base_version": version,
        "apply": [{"edit": "drain", "object": cube.0, "auto": true}]});

    let reply = bench.hand_over_running(&file, asking(&file, drain));

    assert_eq!(reply["applied"], false, "{reply}");
}

#[test]
fn a_channel_digs_one_channel_of_the_points_given() {
    let file = scratch("channel");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let channel = json!({"base_version": version,
        "apply": [{"edit": "channel", "object": cube.0, "diameter_mm": 2.0,
                   "points_mm": [[2.0, 5.0, 5.0], [8.0, 5.0, 5.0]]}]});

    let reply = bench.hand_over_running(&file, asking(&file, channel));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("a channel keeps the id");
    let [dug] = object.hollow.channels() else {
        panic!("one channel, got {:?}", object.hollow.channels());
    };
    assert_eq!(dug.points.len(), 2);
}

#[test]
fn plugs_for_two_holes_stand_as_two_objects_naming_the_cube() {
    let file = scratch("plugs");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let plugs = json!({"base_version": version,
        "apply": [{"edit": "drain", "object": cube.0,
                   "at_mm": [[3.0, 5.0, 10.5], [7.0, 5.0, 10.5]]},
                  {"edit": "plugs", "object": cube.0, "gap_mm": 0.1}]});

    let reply = bench.hand_over_running(&file, asking(&file, plugs));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let laid = (bench.doc.scene.objects().iter())
        .filter(|object| object.plug_of == Some(cube))
        .count();
    assert_eq!(laid, 2, "a plug for each hole");
}

/// How wide, mm, the plug laid for a 3 mm hole is when `gap_mm` is asked on a resin whose fit
/// drifts by `drift_mm`.
fn plug_width_mm(gap_mm: f32, drift_mm: f32) -> f32 {
    let file = scratch("plug-gap");
    let mut bench = Bench::new(file.clone());
    let compensation = &mut bench.machine.slicing.material.compensation;
    compensation.tolerance_on = false;
    compensation.hole_offset_mm = drift_mm;
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let plugs = json!({"base_version": version,
        "apply": [{"edit": "drain", "object": cube.0, "at_mm": [[5.0, 5.0, 10.5]]},
                  {"edit": "plugs", "object": cube.0, "gap_mm": gap_mm}]});
    let reply = bench.hand_over_running(&file, asking(&file, plugs));
    bench.run_to_end();
    assert_eq!(reply["applied"], true, "{reply}");
    let plug = (bench.doc.scene.objects().iter())
        .find(|object| object.plug_of == Some(cube))
        .expect("a plug for the hole");
    let bounds = plug.world_bounds().expect("a plug has faces");
    bounds.maxs.x - bounds.mins.x
}

#[test]
fn a_plug_gap_is_the_final_gap_whatever_the_resin_drifts_by() {
    let drifted = plug_width_mm(0.1, 0.05);
    let plain = plug_width_mm(0.1, 0.0);

    assert!(
        (drifted - plain).abs() < 1e-3,
        "the same plug on either resin, got {drifted} and {plain}"
    );
    assert!(
        (drifted - (3.0 - 2.0 * 0.1)).abs() < 0.02,
        "3 mm less 2 gaps of 0.1 is 2.8 across, got {drifted}"
    );
}

#[test]
fn an_arrange_spreads_two_stacked_cubes_apart_on_the_plate_it_names() {
    let file = scratch("arrange");
    let mut bench = Bench::new(file.clone());
    let first = with_cube(&mut bench);
    let second = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let spread = json!({"base_version": version,
        "apply": [{"edit": "arrange", "plate_mm": [100.0, 100.0], "clearance_mm": 2.0}]});

    let reply = bench.hand_over_running(&file, asking(&file, spread));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let at = |id| {
        bench
            .doc
            .scene
            .get(id)
            .and_then(|object| object.world_bounds())
    };
    let (one, two) = (at(first).expect("kept"), at(second).expect("kept"));
    let apart = (one.mins.x - two.maxs.x)
        .max(two.mins.x - one.maxs.x)
        .max((one.mins.y - two.maxs.y).max(two.mins.y - one.maxs.y));
    assert!(
        apart >= 2.0 - 1e-3,
        "the cubes keep 2 mm apart, got {apart}"
    );
}

#[test]
fn an_arrange_keeps_every_model_inside_the_area_it_names() {
    let file = scratch("arrange-area");
    let mut bench = Bench::new(file.clone());
    let ids = [with_cube(&mut bench), with_cube(&mut bench)];
    let version = bench.doc.history.plate_version();
    let spread = json!({"base_version": version,
        "apply": [{"edit": "arrange", "low_mm": [60.0, 0.0],
                   "plate_mm": [30.0, 90.0], "clearance_mm": 2.0}]});

    let reply = bench.hand_over_running(&file, asking(&file, spread));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    for id in ids {
        let at = bench.doc.scene.get(id).and_then(|o| o.world_bounds());
        let at = at.expect("kept");
        assert!(
            at.mins.x >= 60.0 - 1e-3 && at.maxs.x <= 90.0 + 1e-3,
            "x within 60..90, got {at:?}"
        );
        assert!(
            at.mins.y >= -1e-3 && at.maxs.y <= 90.0 + 1e-3,
            "y within 0..90, got {at:?}"
        );
    }
}

#[test]
fn an_arrange_of_a_model_with_no_surface_is_refused() {
    let file = scratch("arrange-empty");
    let mut bench = Bench::new(file.clone());
    let mesh = core_geometry::Mesh::default();
    let summary = ImportSummary::made(diagnose(&mesh));
    let imported = Imported::new("none".into(), Arc::new(mesh), Transform::default(), summary);
    bench.doc.scene.insert(imported);
    bench.run_to_end();
    let version = bench.doc.history.plate_version();
    let spread = json!({"base_version": version,
        "apply": [{"edit": "arrange", "plate_mm": [100.0, 100.0], "clearance_mm": 2.0}]});

    let reply = bench.hand_over_running(&file, asking(&file, spread));

    assert_eq!(reply["applied"], false, "{reply}");
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
fn auto_supports_stand_contacts_under_a_lifted_cube() {
    let file = scratch("supports-auto");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let lifted = json!({"base_version": version,
        "apply": [{"edit": "move", "object": cube.0, "by_mm": [0.0, 0.0, 5.0]},
                  {"edit": "supports", "object": cube.0, "profile": "auto"}]});

    let reply = bench.hand_over_running(&file, asking(&file, lifted));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("supports keep the id");
    let contacts = object.supports.contacts(object.transform);
    assert!(!contacts.is_empty(), "the lifted cube is held up");
    for contact in &contacts {
        assert!(
            (4.5..=5.0).contains(&contact.z),
            "every contact is at the underside, z 5 less the tip's 0.25 sink, got {contact}"
        );
    }
}

/// The contacts auto supports stand under a cube lifted 5 mm, on a machine printing at `layer_mm`.
fn lifted_contacts_at(layer_mm: f32) -> Vec<Vec3> {
    let file = scratch("supports-step");
    let mut bench = Bench::new(file.clone());
    bench.machine.slicing.material.layer_height_mm = layer_mm;
    let cube = with_cube(&mut bench);
    let version = bench.doc.history.plate_version();
    let lifted = json!({"base_version": version,
        "apply": [{"edit": "move", "object": cube.0, "by_mm": [0.0, 0.0, 5.0]},
                  {"edit": "supports", "object": cube.0, "profile": "auto"}]});
    bench.hand_over_running(&file, asking(&file, lifted));
    bench.run_to_end();
    let object = bench.doc.scene.get(cube).expect("supports keep the id");
    object.supports.contacts(object.transform)
}

#[test]
fn auto_supports_stand_the_same_contacts_whatever_layer_height_the_machine_prints_at() {
    let coarse = lifted_contacts_at(0.1);
    let fine = lifted_contacts_at(0.03);
    assert!(!coarse.is_empty(), "the lifted cube is held up");
    assert_eq!(coarse, fine, "analysis reads one step, not the machine's");
}

/// The contacts auto supports stand under a cube lifted 5 mm whose underside is painted shown
/// when `painted`, the edit asking for a clean surface when `clean_surface`.
fn lifted_contact_count(painted: bool, clean_surface: bool) -> usize {
    let file = scratch("supports-clean");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    if painted {
        let object = bench.doc.scene.get_mut(cube).expect("the cube stands");
        let mesh = Arc::clone(&object.mesh);
        for (at, face) in mesh.faces.iter().enumerate() {
            if face
                .iter()
                .all(|vertex| mesh.vertices[*vertex as usize].z == 0.0)
            {
                object.shown_painted.set(at, true);
            }
        }
    }
    bench.run_to_end();
    let version = bench.doc.history.plate_version();
    let lifted = json!({"base_version": version,
        "apply": [{"edit": "move", "object": cube.0, "by_mm": [0.0, 0.0, 5.0]},
                  {"edit": "supports", "object": cube.0, "profile": "medium",
                   "clean_surface": clean_surface}]});
    let reply = bench.hand_over_running(&file, asking(&file, lifted));
    bench.run_to_end();
    assert_eq!(reply["applied"], true, "{reply}");
    let object = bench.doc.scene.get(cube).expect("supports keep the id");
    object.supports.contacts(object.transform).len()
}

#[test]
fn supports_read_the_painted_shown_side_only_when_the_edit_asks_for_a_clean_surface() {
    let unpainted = lifted_contact_count(false, false);
    let ignored = lifted_contact_count(true, false);
    let read = lifted_contact_count(true, true);

    assert_eq!(ignored, unpainted, "unread: {unpainted} contacts");
    assert!(
        read < unpainted,
        "read: {read} contacts, fewer than {unpainted}"
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

/// A 10 x 10 x 30 mm bar standing on its end.
fn with_bar(bench: &mut Bench) -> ObjectId {
    let mesh = cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 30.0));
    let summary = ImportSummary::made(diagnose(&mesh));
    let imported = Imported::new("bar".into(), Arc::new(mesh), Transform::default(), summary);
    let id = bench.doc.scene.insert(imported);
    bench.run_to_end();
    id
}

fn orient_of(object: ObjectId, base_version: u64, pick: usize) -> Value {
    json!({"base_version": base_version,
           "apply": [{"edit": "orient", "object": object.0, "goal": "shortest-print",
                      "pick": pick}]})
}

#[test]
fn an_orient_lays_a_bar_standing_on_its_end_for_the_shortest_print_and_keeps_the_id() {
    let file = scratch("orient");
    let mut bench = Bench::new(file.clone());
    let bar = with_bar(&mut bench);
    let version = bench.doc.history.plate_version();

    let reply = bench.hand_over_running(&file, asking(&file, orient_of(bar, version, 0)));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let bounds = (bench.doc.scene.get(bar))
        .and_then(|object| object.world_bounds())
        .expect("the bar keeps its id");
    let height = bounds.maxs.z - bounds.mins.z;
    assert!(
        height <= 10.0 + 1e-3,
        "the 30 mm lies along the plate: {height}"
    );
}

#[test]
fn an_orient_with_no_printer_stands_the_bar_as_the_panel_does_on_the_standard_printer() {
    let file = scratch("orient-panel");
    let mut linked = Bench::new(file.clone());
    let bar = with_bar(&mut linked);
    let version = linked.doc.history.plate_version();
    linked.hand_over_running(&file, asking(&file, orient_of(bar, version, 0)));
    linked.run_to_end();
    let mut panel = Bench::new(file.clone());
    let same = with_bar(&mut panel);
    panel.with_window(|window| crate::panels::auto_orient(window, Some(same)));
    panel.run_to_end();

    for bench in [&linked, &panel] {
        assert!(
            bench.tools.orient.proven_on().is_some(),
            "a search with no printer chosen is proved on the standard one"
        );
    }
    let turn = |bench: &Bench, id| bench.doc.scene.get(id).expect("the bar stands").transform;
    let (linked, panel) = (turn(&linked, bar), turn(&panel, same));
    assert!(
        linked.rotation.angle_between(panel.rotation) < 1e-3,
        "the link and the panel stand the bar alike, got {:?} and {:?}",
        linked.rotation,
        panel.rotation
    );
}

#[test]
fn an_orient_asking_a_lift_keeps_it_as_the_lift_row_does() {
    let file = scratch("orient-lift");
    let mut bench = Bench::new(file.clone());
    let bar = with_bar(&mut bench);
    let version = bench.doc.history.plate_version();
    let ask = json!({"base_version": version,
        "apply": [{"edit": "orient", "object": bar.0, "lift_mm": 4.0}]});

    let reply = bench.hand_over_running(&file, asking(&file, ask));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let kept = bench.doc.scene.get(bar).and_then(|object| object.lift_mm);
    assert_eq!(kept, Some(4.0), "the 4 mm asked is the bar's lift");
}

#[test]
fn an_orient_naming_a_printer_the_catalogue_lacks_is_answered_with_why() {
    let file = scratch("orient-no-printer");
    let mut bench = Bench::new(file.clone());
    let bar = with_bar(&mut bench);
    let version = bench.doc.history.plate_version();
    let ask = json!({"base_version": version,
        "apply": [{"edit": "orient", "object": bar.0, "printer": "no-such-printer"}]});

    let reply = bench.hand_over_running(&file, asking(&file, ask));
    bench.run_to_end();

    assert_eq!(reply["applied"], false, "{reply}");
    assert!(reply.to_string().contains("no-such-printer"), "{reply}");
}

#[test]
fn an_orient_asking_for_a_pose_the_search_did_not_find_is_answered_with_why() {
    let file = scratch("orient-past");
    let mut bench = Bench::new(file.clone());
    let bar = with_bar(&mut bench);
    let version = bench.doc.history.plate_version();

    let reply = bench.hand_over_running(&file, asking(&file, orient_of(bar, version, 10_000)));
    bench.run_to_end();

    assert_eq!(reply["applied"], false, "{reply}");
}

#[test]
fn supports_named_for_a_group_of_the_plate_stand_in_that_group_which_stays_in_hand() {
    let file = scratch("supports-group");
    let mut bench = Bench::new(file.clone());
    let cube = with_cube(&mut bench);
    let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    "mine".clone_into(&mut profile.name);
    profile.contact_spacing_mm = 20.0;
    profile.steep_contact_spacing_mm = 20.0;
    let tool = &mut bench.tools.supports;
    tool.groups.push(SupportGroup {
        name: "mine".to_owned(),
        profile,
        hidden: false,
    });
    let version = bench.doc.history.plate_version();
    let supports = json!({"base_version": version,
        "apply": [{"edit": "move", "object": cube.0, "by_mm": [0.0, 0.0, 5.0]},
                  {"edit": "supports", "object": cube.0, "profile": "mine",
                   "at_mm": [[5.0, 5.0, 4.0]]}]});

    let reply = bench.hand_over_running(&file, asking(&file, supports));
    bench.run_to_end();

    assert_eq!(reply["applied"], true, "{reply}");
    let tool = &bench.tools.supports;
    assert_eq!(tool.groups[usize::from(tool.active)].name, "mine");
    let object = bench.doc.scene.get(cube).expect("supports keep the id");
    assert_eq!(
        object.supports.points()[0].group,
        tool.active,
        "the point stands in the plate's group"
    );
    assert!(
        object.supports.point_count() <= 2,
        "built to the group's 20 mm spacing the 10 mm underside holds at most 2 contacts (medium stands 10), got {}",
        object.supports.point_count()
    );
}
