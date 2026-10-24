use std::io::Cursor;

use core_geometry::{Mesh, Quat, Transform, Vec3, diagnose};
use core_supports::{ModelSupports, SupportPoint};
use core_volume::{Blocker, Channel, HollowMode, ModelHollow};

use crate::cut::Keep;

use super::*;
use crate::scene::ImportSummary;
use core_engine::project::Array;
use core_engine::project::{read_from, write_to};

/// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
fn unit_cube() -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::ONE)
}

fn summary() -> ImportSummary {
    ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: core_geometry::Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: diagnose(&unit_cube()),
    }
}

/// A plate with one model on it carrying something from every tool the file records.
#[allow(
    clippy::field_reassign_with_default,
    reason = "Slicing keeps a private field"
)]
struct Bench {
    scene: Scene,
    slicing: Slicing,
    tools: Tools,
}

impl Bench {
    fn plate(&self) -> Captured<'_> {
        Captured {
            scene: &self.scene,
            slicing: &self.slicing,
            tools: &self.tools,
        }
    }

    fn plate_mut(&mut self) -> CapturedMut<'_> {
        CapturedMut {
            scene: &mut self.scene,
            slicing: &mut self.slicing,
            tools: &mut self.tools,
        }
    }
}

fn bench() -> Bench {
    let mesh = Arc::new(unit_cube());
    let mut scene = Scene::default();
    let id = scene.insert(Imported::new(
        "cube".to_owned(),
        Arc::clone(&mesh),
        Transform {
            translation: Vec3::new(12.0, 34.0, 0.0),
            rotation: Quat::from_rotation_z(0.5),
            scale: Vec3::splat(2.0),
            ..Transform::default()
        },
        ImportSummary {
            vertices_merged: 3,
            faces_removed: 1,
            orientation: core_geometry::Orientation {
                flipped_faces: 4,
                inverted_shells: 1,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ));

    let object = scene.get_mut(id).expect("the cube was just inserted");
    object.visible = false;
    let mut painted = core_geometry::Region::default();
    painted.set(5, true);
    painted.set(9, true);
    let mut blocked = core_geometry::Region::default();
    blocked.set(2, true);
    object.supports = ModelSupports::restore(
        vec![
            SupportPoint::new(Vec3::new(0.5, 0.5, 1.0)),
            SupportPoint::new(Vec3::new(0.2, 0.7, 1.0)).in_group(1),
        ],
        painted,
        blocked,
        Vec::new(),
    );
    object.supports.set_raft_lift_mm(0.5);
    object.relief_painted.set(7, true);
    object.hollow = ModelHollow::restored(
        vec![Blocker::ball(Vec3::splat(0.5), 0.2)],
        Vec::new(),
        vec![Channel {
            points: vec![Vec3::ZERO, Vec3::Z],
            diameter_mm: 1.5,
            ..Channel::default()
        }],
    );

    let mut slicing = Slicing::default();
    let printer = slicing
        .catalogue
        .printer(SATURN)
        .expect("ships")
        .profile
        .clone();
    slicing.set_printer(printer, Some(SATURN.to_owned()));
    slicing.set_layer_height(0.035);
    slicing.anti_alias = false;
    slicing.grey_levels = std::num::NonZeroU8::new(4);
    slicing.blur_px = 2;
    slicing.remove_islands = true;
    slicing.hanging_passes = 2;
    slicing.samples = std::num::NonZeroU8::new(3).expect("three is not zero");
    slicing.exposure = vec![core_format::ExposureRange::new(0.0, 4.0, 9.5)];
    slicing.adaptive = Some(core_slicer::AdaptiveSettings::default());

    let mut tools = Tools::default();
    tools.supports.brush_radius_mm = 4.5;
    tools.supports.flood_angle_deg = 25.0;
    tools.supports.add_group();
    tools.hollow.state.thickness_mm = 1.75;
    tools.hollow.state.mode = HollowMode::External;
    tools.hollow.state.infill_on = true;
    tools.drain.state.diameter_mm = 4.25;
    tools.drain.state.depth_mm = 6.0;
    tools.drain.state.taper = 0.5;
    tools.cut.state.axis = crate::scene::Axis::X;
    tools.cut.state.height_mm = 17.5;
    tools.cut.state.keep = Keep::Above;
    tools.relief.state.amplitude_mm = 0.8;
    tools.array = Array {
        columns: 3,
        rows: 4,
        gap_mm: 7.5,
    };

    Bench {
        scene,
        slicing,
        tools,
    }
}

const SATURN: &str = "elegoo-saturn-4-ultra";

/// Writes a plate and reads it back into an empty window.
fn round_trip(saved: &Bench) -> Bench {
    let (back, not_here) = reopened(capture(saved.plate()));
    assert_eq!(not_here, NotHere::default(), "the shipped printer is here");
    back
}

/// Writes `project` and reads it back into an empty window.
fn reopened(project: Project) -> (Bench, NotHere) {
    let mut bytes = Cursor::new(Vec::new());
    write_to(&mut bytes, &project).expect("a project in memory writes");
    bytes.set_position(0);
    let read = read_from(bytes).expect("what was just written reads");

    let mut back = Bench {
        scene: Scene::default(),
        slicing: Slicing::default(),
        tools: Tools::default(),
    };
    let not_here = apply(read, back.plate_mut());
    (back, not_here)
}

#[test]
fn a_project_names_its_printer_and_resin_and_carries_neither() {
    let mut saved = bench();
    saved.slicing.resin_id = Some("grey-0a1b2c3d".to_owned());
    let manifest = capture(saved.plate()).manifest;
    assert_eq!(manifest.printer.as_deref(), Some(SATURN));
    assert_eq!(manifest.resin.as_deref(), Some("grey-0a1b2c3d"));
}

#[test]
fn a_printer_not_here_leaves_the_windows_printer_and_slicing() {
    let mut project = capture(bench().plate());
    project.manifest.printer = Some("someone-elses-0a1b2c3d".to_owned());
    project.manifest.resin = Some("their-grey-4e5f6a7b".to_owned());
    let window = Slicing::default();

    let (back, not_here) = reopened(project);

    assert_eq!(not_here.printer.as_deref(), Some("someone-elses-0a1b2c3d"));
    assert_eq!(not_here.resin.as_deref(), Some("their-grey-4e5f6a7b"));
    assert_eq!(
        back.slicing.printer_id, window.printer_id,
        "the window's own stays"
    );
    assert_eq!(back.slicing.resin_id, window.resin_id);
    assert_eq!(
        back.slicing.layer_height_mm(),
        window.layer_height_mm(),
        "the slicing goes only with its printer"
    );
    assert_eq!(back.slicing.blur_px, window.blur_px);
    assert_eq!(
        back.scene.objects().len(),
        1,
        "the plate opens all the same"
    );
}

#[test]
fn the_status_names_what_was_not_here() {
    assert_eq!(NotHere::default().note(), None);
    let printer = NotHere {
        printer: Some("bench-0a1b2c3d".to_owned()),
        resin: None,
    };
    assert_eq!(
        printer.note().as_deref(),
        Some("the printer bench-0a1b2c3d it names is not among yours, so yours stays")
    );
}

#[test]
fn a_saved_plate_opens_as_the_plate_that_was_saved() {
    let saved = bench();
    let back = round_trip(&saved);

    assert_eq!(back.scene.objects().len(), 1);
    let before = &saved.scene.objects()[0];
    let after = &back.scene.objects()[0];
    assert_eq!(after.name, before.name);
    assert_eq!(after.transform, before.transform);
    assert_eq!(after.visible, before.visible, "a hidden model stays hidden");
    assert_eq!(*after.mesh, *before.mesh, "a face index is what a patch is");
    assert_eq!(after.summary.vertices_merged, 3, "repair is not re-run");
    assert_eq!(after.supports.points(), before.supports.points());
    assert_eq!(after.supports.painted(), before.supports.painted());
    assert_eq!(after.relief_painted, before.relief_painted);
    assert_eq!(after.supports.blocked(), before.supports.blocked());
    assert!(
        (after.supports.raft_lift_mm() - 0.5).abs() < f32::EPSILON,
        "a model reopened on its raft must not be stood up by it again"
    );
    assert_eq!(after.hollow.blockers(), before.hollow.blockers());
    assert_eq!(after.hollow.channels(), before.hollow.channels());
}

#[test]
fn every_tool_opens_set_the_way_it_was_saved() {
    let saved = bench();
    let back = round_trip(&saved);

    assert_eq!(
        back.slicing.layer_height_mm(),
        saved.slicing.layer_height_mm()
    );
    assert_eq!(back.slicing.anti_alias, saved.slicing.anti_alias);
    assert_eq!(back.slicing.grey_levels, saved.slicing.grey_levels);
    assert_eq!(back.slicing.blur_px, saved.slicing.blur_px);
    assert_eq!(back.slicing.remove_islands, saved.slicing.remove_islands);
    assert_eq!(back.slicing.hanging_passes, saved.slicing.hanging_passes);
    assert_eq!(back.slicing.samples, saved.slicing.samples);
    assert_eq!(back.slicing.exposure, saved.slicing.exposure);
    assert_eq!(back.slicing.adaptive, saved.slicing.adaptive);
    assert_eq!(
        back.tools.supports.groups.len(),
        saved.tools.supports.groups.len()
    );
    assert_eq!(back.tools.supports.active, saved.tools.supports.active);
    assert_eq!(
        back.tools.supports.brush_radius_mm,
        saved.tools.supports.brush_radius_mm
    );
    assert_eq!(
        back.tools.supports.flood_angle_deg,
        saved.tools.supports.flood_angle_deg
    );
    assert_eq!(
        back.tools.hollow.state.thickness_mm,
        saved.tools.hollow.state.thickness_mm
    );
    assert_eq!(back.tools.hollow.state.mode, saved.tools.hollow.state.mode);
    assert!(back.tools.hollow.state.infill_on);
    assert_eq!(
        back.tools.drain.state.diameter_mm,
        saved.tools.drain.state.diameter_mm
    );
    assert_eq!(
        back.tools.drain.state.depth_mm,
        saved.tools.drain.state.depth_mm
    );
    assert_eq!(back.tools.drain.state.taper, saved.tools.drain.state.taper);
    assert_eq!(back.tools.cut.state.axis, saved.tools.cut.state.axis);
    assert_eq!(
        back.tools.cut.state.height_mm,
        saved.tools.cut.state.height_mm
    );
    assert_eq!(back.tools.cut.state.keep, saved.tools.cut.state.keep);
    assert_eq!(back.tools.array, saved.tools.array);
}

#[test]
fn the_plates_and_what_stands_on_each_survive_a_round_trip() {
    let mut saved = bench();
    saved.scene.add_plate();
    saved.scene.rename_plate(1, "Small parts".to_owned());
    saved.scene.insert(Imported::new(
        "second".to_owned(),
        Arc::new(unit_cube()),
        Transform::default(),
        summary(),
    ));
    saved.scene.show_plate(1);

    let back = round_trip(&saved);

    assert_eq!(back.scene.plates(), ["Plate 1", "Small parts"]);
    assert_eq!(
        back.scene.active_plate(),
        1,
        "it opens on the plate it was left on"
    );
    assert_eq!(back.scene.here().count(), 1);
    let plates: Vec<u32> = back
        .scene
        .objects()
        .iter()
        .map(|object| object.plate)
        .collect();
    assert_eq!(plates, [0, 1]);
    let names: Vec<&str> = back
        .scene
        .objects()
        .iter()
        .map(|object| object.name.as_str())
        .collect();
    assert_eq!(names, ["cube", "second"]);
}

#[test]
fn moving_a_model_changes_what_the_close_guard_compares() {
    let mut saved = bench();
    let before = crate::project::digest(&capture(saved.plate()).manifest);
    assert_eq!(
        before,
        crate::project::digest(&capture(saved.plate()).manifest),
        "the same plate has to hash the same, or every close would ask"
    );

    saved.scene.objects_mut()[0].transform.translation.x += 1.0;
    assert_ne!(
        before,
        crate::project::digest(&capture(saved.plate()).manifest),
        "a model that moved is work nobody has written down"
    );
}

/// A shell as a run would leave it on the bench's only model: its own mesh, which no
/// hollowing would produce, so a rebuilt cavity could not be mistaken for this one.
fn hollowed(saved: &mut Bench) -> Arc<Mesh> {
    let asked = saved.tools.hollow.settings();
    let object = &mut saved.scene.objects_mut()[0];
    let settings = object.hollow.asking(&asked);
    let shell = Arc::new(unit_cube());
    object.hollow.take(core_volume::Shell {
        mesh: Arc::clone(&shell),
        cavity: 0..2,
        cavity_mm3: 1.5,
        voxel_mm: 0.1,
        coarsened: true,
        shape: object.transform.unturned(),
        settings,
    });
    shell
}

#[test]
fn a_hollow_models_supports_open_standing_against_its_shell() {
    let mut saved = bench();
    hollowed(&mut saved);
    let back = round_trip(&saved);
    let mut object = back.scene.objects()[0].clone();
    assert!(
        !object.keep_support_surface(),
        "the shell was found on opening, so the first frame grows nothing again"
    );
}

#[test]
fn a_models_own_hollow_numbers_are_saved_with_it() {
    let mut saved = bench();
    let mut own = saved.tools.hollow.state.clone();
    own.thickness_mm = 3.5;
    saved.scene.objects_mut()[0].own.hollow = Some(own.clone());

    let back = round_trip(&saved);
    assert_eq!(back.scene.objects()[0].own.hollow, Some(own));
    assert_eq!(back.scene.objects()[0].own.drain, None);
}

#[test]
fn the_plates_raft_and_a_models_own_are_saved_with_the_project() {
    let mut saved = bench();
    let plate = Raft {
        enabled: !saved.tools.supports.raft.enabled,
        thickness_mm: saved.tools.supports.raft.thickness_mm + 1.0,
        ..saved.tools.supports.raft
    };
    let own = Raft {
        thickness_mm: plate.thickness_mm + 1.0,
        ..plate
    };
    saved.tools.supports.raft = plate;
    saved.scene.objects_mut()[0].own.raft = Some(own);

    let back = round_trip(&saved);
    assert_eq!(back.tools.supports.raft, plate);
    assert_eq!(back.scene.objects()[0].own.raft, Some(own));
}

#[test]
fn an_opened_plate_stands_as_it_was_saved_and_builds_nothing() {
    let mut saved = bench();
    let shell = hollowed(&mut saved);

    let back = round_trip(&saved);
    let object = &back.scene.objects()[0];
    assert_eq!(
        object.hollow.shell().map(|mesh| &**mesh),
        Some(&*shell),
        "the shell is read out of the file"
    );
    assert_eq!(object.hollow.cavity_faces(), Some(0..2));
    assert_eq!(object.hollow.lattice(), Some((0.1, true)));
    assert!(
        back.tools.hollow.job.is_none(),
        "nothing is hollowed on opening"
    );
}

#[test]
fn the_wall_a_model_was_hollowed_to_is_what_the_panel_reads_back() {
    let mut saved = bench();
    let asked = saved.tools.hollow.settings();
    hollowed(&mut saved);

    let project = capture(saved.plate());
    let built = project.manifest.objects[0]
        .hollow
        .built
        .as_ref()
        .expect("the model was hollowed");
    assert_eq!(built.wall, Cavity::of(&asked));

    let back = round_trip(&saved);
    assert!(
        !back.scene.objects()[0].hollow.is_stale(
            &back.tools.hollow.settings(),
            back.scene.objects()[0].transform
        ),
        "a plate opened as it was saved is not stale"
    );
}

#[test]
fn a_shell_left_stale_by_a_resize_opens_stale() {
    let mut saved = bench();
    hollowed(&mut saved);
    let object = &mut saved.scene.objects_mut()[0];
    object.transform = object.transform.stretched(Vec3::splat(2.0));

    let back = round_trip(&saved);
    let object = &back.scene.objects()[0];
    assert!(
        object
            .hollow
            .is_stale(&back.tools.hollow.settings(), object.transform),
        "the wall was built at half the size the model now stands at"
    );
}

#[test]
fn the_trees_a_run_grew_stand_again_without_being_grown() {
    let mut saved = bench();
    let table = saved.tools.supports.table();
    let object = &mut saved.scene.objects_mut()[0];
    // Lifted off the plate, so the underside has room for a column to stand in.
    object.transform.translation.z = 5.0;
    let (mesh, bvh, transform) = (
        Arc::clone(&object.mesh),
        Arc::clone(&object.bvh),
        object.transform,
    );
    let under = transform
        .to_matrix()
        .transform_point3(Vec3::new(0.5, 0.5, 0.0));
    object.supports.add(under, transform, 0);
    object.supports.refresh(&mesh, &bvh, transform, &table);
    let standing = object.supports.trees().to_vec();
    assert!(!standing.is_empty(), "the points grew into trees");

    let back = round_trip(&saved);
    let object = &back.scene.objects()[0];
    let trees = object.supports.trees();
    assert_eq!(trees.len(), standing.len(), "every tree comes back");
    // A tree is kept in the model's own space and stood up again through the model's
    // placement, which is two matrices and the last digit of a float.
    for (tree, was) in trees.iter().zip(&standing) {
        assert_eq!(tree.group(), was.group());
        assert_eq!(tree.landing().on_model, was.landing().on_model);
        assert!(
            tree.landing().base.abs_diff_eq(was.landing().base, 1e-4),
            "a tree stands where it stood"
        );
        assert_eq!(tree.nodes().len(), was.nodes().len());
        for (node, old) in tree.nodes().iter().zip(was.nodes()) {
            assert_eq!((node.parent, node.point), (old.parent, old.point));
            assert!(
                node.position.abs_diff_eq(old.position, 1e-4),
                "every tree is the one saved: {} against {}",
                node.position,
                old.position
            );
        }
    }
    assert!(
        object
            .supports
            .meshes()
            .is_some_and(|groups| groups.iter().any(|mesh| !mesh.is_empty())),
        "and it is meshed to be seen and sliced"
    );
}
