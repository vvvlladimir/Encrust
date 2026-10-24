use super::*;
use crate::job::Placement;
use crate::scene::Scene;
use crate::status::Status;
use core_engine::project::FillProfiles;
use core_geometry::{Adjacency, Scalar, Vec3};
use core_geometry::{Bvh, Mesh, Transform};
use core_supports::{Need, Reason};
use core_supports::{ProjectSettings, SupportTree};
use printer_profiles::{Catalogue, Raft, SupportProfile};
use std::sync::Arc;

/// A scene holding one unit cube standing on the plate.
fn one_cube() -> Scene {
    use crate::scene::{ImportSummary, Imported};
    use core_geometry::{Orientation, diagnose};

    let cube = crate::supports::tests::cube();
    let summary = ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: diagnose(&cube),
    };

    let mut scene = Scene::default();
    scene.insert(Imported::new(
        "cube".to_owned(),
        Arc::new(cube),
        Transform::default(),
        summary,
    ));
    scene
}

/// The cube with one support under it in the first group.
fn one_supported_cube() -> Scene {
    let mut scene = one_cube();
    let object = &mut scene.objects_mut()[0];
    object.transform.translation.z = 10.0;
    let transform = object.transform;
    object.supports.add(Vec3::new(0.5, 0.5, 10.0), transform, 0);
    scene
}

/// Runs placement with the shipped automatic profile under `scene` to its end.
fn grown_by_auto(scene: &mut Scene) -> (SupportTool, Status) {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut tool = SupportTool {
        auto: catalogue.autos().next().map(|entry| entry.profile.clone()),
        ..SupportTool::default()
    };
    tool.start(scene, 0.05, &catalogue, Side::Outside)
        .expect("a model to place under");
    let mut status = Status::default();
    while tool.poll(scene, &mut status) {
        std::thread::yield_now();
    }
    (tool, status)
}

#[test]
fn a_small_island_is_held_by_the_lightest_shape_in_the_empty_group() {
    let mut scene = one_cube();
    scene.objects_mut()[0].transform.translation.z = 10.0;
    let (tool, status) = grown_by_auto(&mut scene);

    assert!(
        matches!(status, Status::Info(_)),
        "the run ends placed: {status:?}"
    );
    assert_eq!(
        tool.groups.len(),
        1,
        "no group for a shape nothing is sized into"
    );
    assert_eq!(
        tool.groups[0].profile.name, "Auto Light",
        "the empty first group is retuned to the shape a 1 mm² island takes"
    );
    let object = &scene.objects()[0];
    assert!(object.supports.point_count() > 0);
    assert_eq!(
        object.supports.group_count(0),
        object.supports.point_count()
    );
}

#[test]
fn sizing_turned_off_and_on_again_keeps_the_numbers_it_was_left_with() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut tool = SupportTool::default();
    tool.set_automatic(true, &catalogue);
    let auto = tool
        .auto
        .as_mut()
        .expect("the shipped automatic profile is taken up");
    auto.density = 2.0;
    tool.set_automatic(false, &catalogue);
    assert!(tool.auto.is_none());
    tool.set_automatic(true, &catalogue);
    assert_eq!(tool.auto.map(|auto| auto.density), Some(2.0));
}

#[test]
fn a_missing_shape_stops_the_run_before_it_starts() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut auto = catalogue
        .autos()
        .next()
        .map(|entry| entry.profile.clone())
        .expect("an automatic profile ships");
    auto.shapes[2] = "no-such-shape".to_owned();
    let mut tool = SupportTool {
        auto: Some(auto),
        ..SupportTool::default()
    };
    assert!(
        tool.start(&mut one_cube(), 0.05, &catalogue, Side::Outside)
            .is_err()
    );
    assert!(tool.job.is_none());
}

#[test]
fn a_model_is_oriented_by_the_overhang_of_its_own_profile() {
    let mut scene = one_cube();
    let tool = SupportTool::default();
    let mut own = tool.profile.clone();
    own.max_overhang_deg += 10.0;
    let wanted = own.max_overhang_deg;
    scene.objects_mut()[0].own.supports.insert(tool.active, own);

    let tasks = crate::job::orient_tasks(&scene, None, |object| {
        core_engine::SupportPlan::of(tool.profile_for(object).clone())
    });
    assert!(
        (tasks[0].support.placing().max_overhang_deg - wanted).abs() < f32::EPSILON,
        "not the angle of the plate's profile"
    );
}

#[test]
fn a_model_is_oriented_on_the_raft_it_stands_on() {
    let mut scene = one_cube();
    let mut tool = SupportTool::default();
    tool.raft.enabled = true;

    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let plans = tool
        .orient_plans(&catalogue)
        .expect("no automatic profile to read");
    let tasks = crate::job::orient_tasks(&scene, None, &plans);
    assert!(
        tasks[0].support.placing().raft.enabled,
        "the plate's raft is weighed"
    );

    let own = Raft {
        enabled: false,
        ..tool.raft
    };
    scene.objects_mut()[0].own.raft = Some(own);
    let tasks = crate::job::orient_tasks(&scene, None, &plans);
    assert!(
        !tasks[0].support.placing().raft.enabled,
        "its own raft wins"
    );
}

#[test]
fn with_sizing_on_a_model_is_oriented_on_the_shapes_a_run_sizes_its_supports_into() {
    let scene = one_cube();
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut tool = SupportTool::default();
    tool.set_automatic(true, &catalogue);
    tool.raft.enabled = true;

    let plans = tool
        .orient_plans(&catalogue)
        .expect("the shipped shapes are found");
    let tasks = crate::job::orient_tasks(&scene, None, &plans);
    let auto = tool.auto.as_ref().expect("sizing is on");
    let plan = &tasks[0].support;
    assert_eq!(plan.sizing(), Some(auto), "sized as a run sizes them");
    assert_eq!(
        plan.table()[0].name,
        super::run::shapes_of(auto, &catalogue).expect("the shipped shapes are found")[0].name,
        "built to the profile's own shapes, not the group in hand"
    );
    assert!(
        plan.table().iter().all(|profile| profile.raft.enabled),
        "every shape stands on the plate's raft"
    );
}

#[test]
fn another_profile_starts_a_kind_of_its_own_and_leaves_what_stands_alone() {
    let mut scene = one_supported_cube();
    let mut tool = SupportTool::default();
    let first = tool.profile.clone();
    let other = SupportProfile::shipped("heavy").expect("the heavy preset ships");
    assert_ne!(first.name, other.name);

    tool.chosen = Some(other.clone());
    tool.take_up_chosen(&mut scene);

    assert_eq!(tool.groups.len(), 2, "what stands keeps its group");
    assert_eq!(tool.active, 1, "and the next support joins the new one");
    assert_eq!(tool.groups[1].name, other.name);
    assert_eq!(tool.table(), vec![first, other]);
    assert_eq!(scene.objects()[0].supports.group_count(0), 1);
}

#[test]
fn a_group_nothing_stands_in_is_retuned_rather_than_left_behind() {
    let mut scene = one_cube();
    let mut tool = SupportTool::default();
    let other = SupportProfile::shipped("heavy").expect("the heavy preset ships");

    tool.chosen = Some(other.clone());
    tool.take_up_chosen(&mut scene);

    assert_eq!(tool.groups.len(), 1);
    assert_eq!(tool.groups[0].name, other.name);
    assert_eq!(tool.table(), vec![other]);
}

#[test]
fn a_profile_a_group_is_built_to_is_that_group_again() {
    let mut scene = one_supported_cube();
    let mut tool = SupportTool::default();
    let first = tool.profile.clone();
    tool.chosen = Some(SupportProfile::shipped("heavy").expect("the heavy preset ships"));
    tool.take_up_chosen(&mut scene);

    tool.chosen = Some(first);
    tool.take_up_chosen(&mut scene);

    assert_eq!(
        tool.groups.len(),
        2,
        "no third group for a profile already there"
    );
    assert_eq!(tool.active, 0);
}

#[test]
fn the_profile_in_hand_chosen_again_goes_back_to_what_is_saved() {
    let mut scene = one_supported_cube();
    let id = scene.objects()[0].id;
    scene.select(Some(id));
    let mut tool = SupportTool::default();
    let saved = tool.profile.clone();
    tool.profile.middle.diameter_mm += 0.3;
    let mut own = saved.clone();
    own.raft.enabled = !own.raft.enabled;
    scene.objects_mut()[0].own.supports.insert(0, own);

    tool.chosen = Some(saved.clone());
    tool.take_up_chosen(&mut scene);

    assert_eq!(tool.profile, saved);
    assert!(
        scene.objects()[0].own.supports.is_empty(),
        "the picked model goes back with it"
    );
    assert_eq!(tool.groups.len(), 1);
}

/// A raft unlike the default one: switched the other way and a millimetre thicker.
fn another_raft(tool: &SupportTool) -> Raft {
    Raft {
        enabled: !tool.raft.enabled,
        thickness_mm: tool.raft.thickness_mm + 1.0,
        ..tool.raft
    }
}

#[test]
fn a_raft_edited_with_a_model_picked_is_the_plates() {
    let mut scene = one_cube();
    scene.select(Some(scene.objects()[0].id));
    let mut tool = SupportTool::default();
    let edited = another_raft(&tool);

    tool.keep_raft(edited, &mut scene);

    assert_eq!(tool.raft, edited, "the plate's, so the next run keeps it");
    assert_eq!(scene.objects()[0].own.raft, None, "and not the model's own");
}

#[test]
fn a_models_own_raft_is_edited_in_place_and_dropped_at_the_plates() {
    let mut scene = one_cube();
    let first = scene.objects()[0].id;
    scene.select(Some(first));
    let mut tool = SupportTool::default();
    let (plate, own) = (tool.raft, another_raft(&tool));
    scene.get_mut(first).expect("there").own.raft = Some(own);
    let thicker = Raft {
        thickness_mm: own.thickness_mm + 1.0,
        ..own
    };

    tool.keep_raft(thicker, &mut scene);
    assert_eq!(scene.get(first).expect("there").own.raft, Some(thicker));
    assert_eq!(tool.raft, plate, "a project's own raft leaves the plate's");

    tool.keep_raft(plate, &mut scene);
    assert_eq!(
        scene.get(first).expect("there").own.raft,
        None,
        "a raft that is the plate's again is no longer the model's own"
    );
}

#[test]
fn a_raft_edited_with_nothing_picked_is_the_plates() {
    let mut scene = one_cube();
    scene.select(None);
    let mut tool = SupportTool::default();
    let edited = another_raft(&tool);

    tool.keep_raft(edited, &mut scene);

    assert_eq!(tool.raft, edited);
    assert_eq!(tool.table_for(&scene.objects()[0])[0].raft, edited);
}

#[test]
fn another_profile_leaves_the_raft_as_it_was() {
    let mut scene = one_supported_cube();
    let mut tool = SupportTool::default();
    let raft = another_raft(&tool);
    tool.raft = raft;
    assert_ne!(
        SupportProfile::shipped("heavy")
            .expect("the heavy preset ships")
            .raft,
        raft
    );

    tool.chosen = Some(SupportProfile::shipped("heavy").expect("the heavy preset ships"));
    tool.take_up_chosen(&mut scene);
    tool.retune_group(
        0,
        SupportProfile::shipped("light").expect("the light preset ships"),
        &mut scene,
    );

    assert_eq!(
        tool.table_for(&scene.objects()[0])[0].raft,
        raft,
        "the raft is the plate's, whatever the groups are built to"
    );
}

/// An axis-aligned cube spanning 0..1, twelve triangles.
fn cube() -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::ONE)
}

/// The index of the first face of `mesh` whose normal points the given way.
fn face_looking(mesh: &Mesh, up: Scalar) -> usize {
    (0..mesh.faces.len())
        .find(|face| {
            mesh.triangle(*face).is_some_and(|triangle| {
                (triangle.normal_unnormalized().normalize_or_zero().z - up).abs() < 0.1
            })
        })
        .expect("a cube has a face looking each way")
}

/// Paints the surface the face `seed` belongs to on the only object of `scene`.
fn paint_surface(scene: &mut Scene, seed: usize, blocking: bool) {
    let mesh = Arc::clone(&scene.objects()[0].mesh);
    let adjacency = Adjacency::of(&mesh);
    scene.objects_mut()[0]
        .supports
        .paint_face(&mesh, &adjacency, seed, 30.0, blocking, true);
}

#[test]
fn filling_a_painted_patch_stands_supports_under_it() {
    let mut scene = one_cube();
    let underside = face_looking(&scene.objects()[0].mesh, -1.0);
    paint_surface(&mut scene, underside, false);

    let mut tool = SupportTool {
        fill: ProjectSettings {
            infill_spacing_mm: Some(0.3),
            border_spacing_mm: None,
            infill_offset_mm: 0.0,
            ..ProjectSettings::default()
        },
        ..SupportTool::default()
    };
    let added = tool.fill(&mut scene, &Catalogue::default());

    assert!(
        added > 1,
        "a 1 mm square at 0.3 mm spacing takes several supports, got {added}"
    );
    for point in scene.objects()[0].supports.points() {
        assert!(
            point.contact.z.abs() < 1e-4,
            "{} is not on the underside that was painted",
            point.contact
        );
    }
}

#[test]
fn a_fill_builds_its_rim_and_its_inside_to_the_profiles_chosen_for_them() {
    let mut scene = one_cube();
    let underside = face_looking(&scene.objects()[0].mesh, -1.0);
    paint_surface(&mut scene, underside, false);
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let manual: Vec<&printer_profiles::Entry<SupportProfile>> = catalogue
        .supports()
        .filter(|entry| !catalogue.is_auto_shape(&entry.id))
        .collect();
    let [rim, inside, ..] = manual.as_slice() else {
        panic!("the shipped catalogue has at least two profiles to build by hand");
    };

    let mut tool = SupportTool {
        fill: ProjectSettings {
            infill_spacing_mm: Some(0.3),
            border_spacing_mm: Some(0.3),
            border_offset_mm: 0.0,
            infill_offset_mm: 0.0,
            ..ProjectSettings::default()
        },
        fill_profiles: FillProfiles {
            rim: Some(rim.id.clone()),
            inside: Some(inside.id.clone()),
        },
        ..SupportTool::default()
    };
    tool.fill(&mut scene, &catalogue);

    let group_of = |name: &str| {
        tool.groups
            .iter()
            .position(|group| group.profile.name == name)
            .unwrap_or_else(|| panic!("no group was made for {name}")) as u16
    };
    let (rim_group, inside_group) = (group_of(&rim.profile.name), group_of(&inside.profile.name));
    assert_ne!(rim_group, tool.active, "the group in hand was not retuned");
    assert_ne!(
        inside_group, tool.active,
        "the group in hand was not retuned"
    );
    let points = scene.objects()[0].supports.points();
    assert!(
        points.iter().any(|point| point.group == rim_group),
        "nothing in the rim's group"
    );
    assert!(
        points.iter().any(|point| point.group == inside_group),
        "nothing in the inside's group"
    );
}

#[test]
fn a_painted_patch_is_drawable_before_the_columns_are_rebuilt() {
    let mut scene = one_cube();
    let underside = face_looking(&scene.objects()[0].mesh, -1.0);
    paint_surface(&mut scene, underside, false);

    let mesh = Arc::clone(&scene.objects()[0].mesh);
    scene.objects_mut()[0]
        .supports
        .refresh_patches(&mesh, Transform::default());

    let (painted, blocked) = scene.objects()[0].supports.patches();
    assert!(painted.is_some(), "the stroke has geometry to draw at once");
    assert!(blocked.is_none(), "nothing was painted out of bounds");
}

#[test]
fn nothing_is_filled_where_the_patch_is_blocked() {
    let mut scene = one_cube();
    let underside = face_looking(&scene.objects()[0].mesh, -1.0);
    paint_surface(&mut scene, underside, false);
    paint_surface(&mut scene, underside, true);

    let mut tool = SupportTool::default();
    assert_eq!(
        tool.fill(&mut scene, &Catalogue::default()),
        0,
        "the painted patch is the blocked patch"
    );
}

#[test]
fn a_support_over_a_blocked_face_steps_off_it() {
    let mut scene = one_cube();
    let lid = face_looking(&scene.objects()[0].mesh, 1.0);
    let mesh = Arc::clone(&scene.objects()[0].mesh);
    let bvh = Bvh::build(&mesh);
    scene.objects_mut()[0]
        .supports
        .add(Vec3::new(0.5, 0.5, 8.0), Transform::default(), 0);

    let rebuild = |scene: &mut Scene| {
        scene.objects_mut()[0].supports.refresh(
            &mesh,
            &bvh,
            Transform::default(),
            std::slice::from_ref(
                &SupportProfile::shipped("medium").expect("the medium preset ships"),
            ),
        );
        scene.objects()[0].supports.trees()[0].landing().on_model
    };

    assert!(
        rebuild(&mut scene),
        "it stands on the lid of the cube while nothing is blocked"
    );

    paint_surface(&mut scene, lid, true);
    assert!(
        !rebuild(&mut scene),
        "the lid is painted out of bounds, so the support has to reach past it"
    );
}

#[test]
fn only_the_two_brushes_paint() {
    assert!(Placing::Paint.paints());
    assert!(Placing::Block.paints());
    assert!(!Placing::Point.paints());
    assert!(Placing::Erase.paints() && !Placing::Erase.blocks());
}

#[test]
fn a_new_group_starts_from_the_numbers_in_hand_and_takes_the_pill() {
    let mut tool = SupportTool::default();
    tool.profile.middle.diameter_mm = 1.5;
    tool.add_group();

    assert_eq!(tool.active, 1);
    assert_eq!(tool.groups.len(), 2);
    assert!(
        (tool.table()[1].middle.diameter_mm - 1.5).abs() < 1e-6,
        "the new group was struck from what was being edited"
    );
}

#[test]
fn choosing_another_group_keeps_what_was_edited_of_the_one_being_left() {
    let mut tool = SupportTool::default();
    tool.add_group();
    tool.profile.middle.diameter_mm = 2.5;

    tool.choose_group(0);
    assert!(
        (tool.profile.middle.diameter_mm - SupportProfile::default().middle.diameter_mm).abs()
            < 1e-6,
        "the default group's own numbers came back"
    );

    tool.choose_group(1);
    assert!(
        (tool.profile.middle.diameter_mm - 2.5).abs() < 1e-6,
        "the edit made to the second group was kept"
    );
}

#[test]
fn dropping_a_group_takes_its_supports_away_with_it() {
    let mut scene = one_cube();
    let mut tool = SupportTool::default();
    tool.add_group();
    scene.objects_mut()[0]
        .supports
        .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 1);
    assert_eq!(
        tool.group_holds(1, &scene),
        1,
        "the group holds one support"
    );

    assert_eq!(tool.remove_group(1, &mut scene), 1);

    assert_eq!(tool.groups.len(), 1);
    assert_eq!(tool.active, 0);
    assert!(
        scene.objects()[0].supports.points().is_empty(),
        "a support of a group that is gone is gone with it, not handed to another \
             group to be rebuilt in its shape"
    );
}

#[test]
fn a_support_of_another_group_survives_the_one_that_is_dropped() {
    let mut scene = one_cube();
    let mut tool = SupportTool::default();
    tool.add_group();
    let object = &mut scene.objects_mut()[0];
    object
        .supports
        .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 0);
    object
        .supports
        .add(Vec3::new(0.6, 0.6, 5.0), Transform::default(), 1);

    assert_eq!(tool.remove_group(1, &mut scene), 1);
    assert_eq!(
        scene.objects()[0].supports.point_count(),
        1,
        "the default group's own support stayed"
    );
}

/// Stands one support under the cube of `scene` and builds what it grows into.
fn one_support(scene: &mut Scene, tool: &SupportTool) {
    scene.objects_mut()[0]
        .supports
        .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 0);
    rebuild(scene, tool);
}

fn rebuild(scene: &mut Scene, tool: &SupportTool) {
    let mesh = Arc::clone(&scene.objects()[0].mesh);
    let bvh = Bvh::build(&mesh);
    scene.objects_mut()[0]
        .supports
        .refresh(&mesh, &bvh, Transform::default(), &tool.table());
}

#[test]
fn freezing_a_support_takes_its_point_out_of_the_automatic_run() {
    let mut scene = one_cube();
    let tool = SupportTool::default();
    one_support(&mut scene, &tool);

    let frozen = scene.objects_mut()[0]
        .supports
        .freeze(0, Transform::default())
        .expect("the support was standing");
    rebuild(&mut scene, &tool);

    let supports = &scene.objects()[0].supports;
    assert_eq!(frozen, 0);
    assert!(supports.points().is_empty(), "the point went with the tree");
    assert_eq!(supports.frozen().len(), 1);
    assert_eq!(supports.trees().len(), 1, "it is still standing");
    assert_eq!(supports.point_count(), 1, "and still counted");
}

#[test]
fn a_node_carried_aside_stays_there_through_a_rebuild() {
    let mut scene = one_cube();
    let tool = SupportTool::default();
    one_support(&mut scene, &tool);
    scene.objects_mut()[0]
        .supports
        .freeze(0, Transform::default())
        .expect("the support was standing");

    let moved = Vec3::new(2.0, 0.5, 5.0);
    scene.objects_mut()[0]
        .supports
        .frozen_mut(0)
        .expect("just frozen")
        .move_node(0, moved);
    rebuild(&mut scene, &tool);

    let standing = scene.objects()[0].supports.trees()[0].nodes()[0].position;
    assert!(
        standing.abs_diff_eq(moved, 1e-4),
        "the node stayed where it was carried, got {standing}"
    );
}

#[test]
fn a_thawed_support_is_grown_again_from_its_tips() {
    let mut scene = one_cube();
    let tool = SupportTool::default();
    one_support(&mut scene, &tool);
    scene.objects_mut()[0]
        .supports
        .freeze(0, Transform::default())
        .expect("the support was standing");

    scene.objects_mut()[0]
        .supports
        .thaw(0, Transform::default());
    rebuild(&mut scene, &tool);

    let supports = &scene.objects()[0].supports;
    assert!(supports.frozen().is_empty());
    assert_eq!(supports.points().len(), 1, "its tip became a point again");
    assert_eq!(supports.trees().len(), 1);
}

#[test]
fn the_last_group_cannot_be_dropped() {
    let mut scene = one_cube();
    let mut tool = SupportTool::default();
    tool.remove_group(0, &mut scene);
    assert_eq!(tool.groups.len(), 1, "something has to hold the raft");
}

#[test]
fn the_first_group_goes_and_the_next_takes_its_place() {
    let mut scene = one_supported_cube();
    let transform = scene.objects()[0].transform;
    let mut tool = SupportTool {
        chosen: Some(SupportProfile::shipped("heavy").expect("the heavy preset ships")),
        ..SupportTool::default()
    };
    tool.take_up_chosen(&mut scene);
    scene.objects_mut()[0]
        .supports
        .add(Vec3::new(0.2, 0.2, 10.0), transform, 1);

    let gone = tool.remove_group(0, &mut scene);

    assert_eq!(gone, 1, "the first group's one support went with it");
    assert_eq!(tool.groups.len(), 1);
    assert_eq!(
        tool.groups[0].name,
        SupportProfile::shipped("heavy")
            .expect("the heavy preset ships")
            .name
    );
    assert_eq!(
        tool.profile,
        SupportProfile::shipped("heavy").expect("the heavy preset ships")
    );
    assert_eq!(
        scene.objects()[0].supports.group_count(0),
        1,
        "what stood in the second group now stands in the first"
    );
}

#[test]
fn a_group_is_rebuilt_to_another_profile_and_named_for_it() {
    let mut scene = one_supported_cube();
    let mut tool = SupportTool::default();
    let mut own = tool.profile.clone();
    own.raft.enabled = !own.raft.enabled;
    scene.objects_mut()[0].own.supports.insert(0, own);

    tool.retune_group(
        0,
        SupportProfile::shipped("heavy").expect("the heavy preset ships"),
        &mut scene,
    );

    assert_eq!(
        tool.table(),
        vec![SupportProfile::shipped("heavy").expect("the heavy preset ships")]
    );
    assert_eq!(
        tool.groups[0].name,
        SupportProfile::shipped("heavy")
            .expect("the heavy preset ships")
            .name
    );
    assert!(scene.objects()[0].own.supports.is_empty());
    assert_eq!(
        scene.objects()[0].supports.group_count(0),
        1,
        "the support stays, in the group it was in"
    );
}

#[test]
fn two_groups_stand_two_thicknesses_on_one_model() {
    let mut scene = one_cube();
    let mut tool = SupportTool::default();
    tool.add_group();
    tool.profile.middle.diameter_mm *= 2.0;
    tool.groups[1].profile = tool.profile.clone();

    let object = &mut scene.objects_mut()[0];
    object
        .supports
        .add(Vec3::new(0.2, 0.5, 5.0), Transform::default(), 0);
    object
        .supports
        .add(Vec3::new(0.8, 0.5, 5.0), Transform::default(), 1);

    let mesh = Arc::clone(&scene.objects()[0].mesh);
    let bvh = Bvh::build(&mesh);
    scene.objects_mut()[0]
        .supports
        .refresh(&mesh, &bvh, Transform::default(), &tool.table());

    let groups: Vec<u16> = scene.objects()[0]
        .supports
        .trees()
        .iter()
        .map(SupportTree::group)
        .collect();
    assert_eq!(
        groups,
        vec![0, 1],
        "one tree per group, each to its own shape"
    );
}

#[test]
fn a_raft_stands_a_supported_model_up_by_its_thickness_and_back_down_without_it() {
    let mut scene = one_cube();
    let mut tool = SupportTool::default();
    scene.objects_mut()[0].lift_to(tool.profile.z_lift_mm);
    let bottom = |scene: &Scene| {
        scene.objects()[0]
            .world_bounds()
            .expect("a cube has bounds")
            .mins
            .z
    };
    let lifted = bottom(&scene);

    tool.raft.enabled = true;
    tool.settle_on_raft(&mut scene);
    assert!(
        (bottom(&scene) - lifted).abs() < 1e-5,
        "a model with no supports has no raft under it"
    );

    let transform = scene.objects()[0].transform;
    scene.objects_mut()[0]
        .supports
        .add(Vec3::new(0.5, 0.5, lifted), transform, 0);
    tool.settle_on_raft(&mut scene);
    tool.settle_on_raft(&mut scene);
    assert!(
        (bottom(&scene) - lifted - tool.raft.thickness_mm).abs() < 1e-5,
        "the slab must not be printed into the part, and asking twice lifts once"
    );
    let over_raft = scene.objects()[0].lift().expect("a cube has bounds");
    assert!(
        (over_raft - tool.profile.z_lift_mm).abs() < 1e-5,
        "the lift is measured over the raft, got {over_raft}"
    );

    tool.raft.enabled = false;
    tool.settle_on_raft(&mut scene);
    assert!((bottom(&scene) - lifted).abs() < 1e-5, "and back down");
}

#[test]
fn a_rafted_model_put_back_on_the_plate_is_not_sunk_into_it_when_its_supports_go() {
    let mut scene = one_supported_cube();
    let mut tool = SupportTool::default();
    tool.raft.enabled = true;
    tool.settle_on_raft(&mut scene);

    scene.objects_mut()[0].stand_on_plate();
    tool.settle_on_raft(&mut scene);
    scene.objects_mut()[0].supports.clear();
    tool.settle_on_raft(&mut scene);

    let bottom = scene.objects()[0]
        .world_bounds()
        .expect("a cube has bounds")
        .mins
        .z;
    assert!(
        bottom > -1e-5,
        "a model stood on the plate owes the raft nothing, got its bottom at {bottom} mm"
    );
}

/// A unit cube held 5 mm over the plate: its underside, the face at z = 5, is an
/// island on the layer that starts there.
fn floating_cube() -> Scene {
    let mut scene = one_cube();
    scene.objects_mut()[0].transform.translation.z = 5.0;
    scene
}

#[test]
fn an_island_gets_a_support_on_its_underside_in_a_group_of_its_own() {
    let mut scene = floating_cube();
    let mut tool = SupportTool::default();
    let floor = Vec3::new(0.5, 0.5, 5.0);

    assert_eq!(tool.support_islands(&mut scene, &[floor], 0.05), 1);
    assert_eq!(tool.groups.len(), 2);
    assert_eq!(tool.groups[1].name, ISLANDS_GROUP);
    assert_eq!(tool.active, 0, "the group in hand stays in hand");
    let point = scene.objects()[0].supports.points()[0];
    assert_eq!(point.group, 1);
    let contact = scene.objects()[0]
        .transform
        .to_matrix()
        .transform_point3(point.contact);
    assert!(
        (contact - floor).length() < 1e-4,
        "the cube's bottom face is straight over the floor, got {contact}"
    );

    tool.support_islands(&mut scene, &[floor], 0.05);
    assert_eq!(tool.groups.len(), 2, "a second press joins the same group");
}

#[test]
fn an_island_with_nothing_over_its_centre_stands_nothing_and_adds_no_group() {
    let mut scene = floating_cube();
    let mut tool = SupportTool::default();
    let beside = Vec3::new(3.0, 0.5, 5.0);
    let far_under = Vec3::new(0.5, 0.5, 2.0);

    assert_eq!(
        tool.support_islands(&mut scene, &[beside, far_under], 0.05),
        0
    );
    assert_eq!(tool.groups.len(), 1);
}
/// The cube 20 mm a side, 5 mm over the plate, hollowed to a cavity 12 mm a side in its
/// middle: the shell is the cube with that box appended wound inward.
fn hollow_cube() -> Scene {
    let mut scene = one_cube();
    let object = &mut scene.objects_mut()[0];
    object.transform = Transform {
        translation: Vec3::new(0.0, 0.0, 5.0),
        scale: Vec3::splat(20.0),
        ..Transform::default()
    };
    let mut shell = cube();
    let inner = cube();
    let offset = shell.vertices.len() as u32;
    shell
        .vertices
        .extend(inner.vertices.iter().map(|at| *at * 0.6 + Vec3::splat(0.2)));
    shell.faces.extend(
        inner
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );
    let settings = object
        .hollow
        .asking(&core_volume::HollowSettings::for_wall(4.0));
    object.hollow.take(core_volume::Shell {
        mesh: Arc::new(shell),
        cavity: 12..24,
        cavity_mm3: 1728.0,
        voxel_mm: 0.1,
        coarsened: false,
        shape: object.transform.unturned(),
        settings,
    });
    assert!(
        object.keep_support_surface(),
        "the shell begins with the cube"
    );
    scene
}

/// A run's placement under the hollow cube: one contact under the cavity's ceiling,
/// 21 mm up, and one under the cube's own underside at 5 mm.
fn inside_and_out(scene: &Scene) -> Vec<Placement> {
    let object = &scene.objects()[0];
    let need = |z: Scalar| Need {
        contact: Vec3::new(10.0, 10.0, z),
        reason: Reason::Steady,
    };
    vec![Placement {
        id: object.id,
        transform: object.transform,
        needs: vec![need(20.95), need(4.95)],
    }]
}

fn needs_at(placements: &[Placement]) -> Vec<Scalar> {
    placements[0]
        .needs
        .iter()
        .map(|need| need.contact.z)
        .collect()
}

#[test]
fn a_run_keeps_only_the_contacts_on_its_side_of_the_wall() {
    let scene = hollow_cube();
    let tool = SupportTool {
        side: Side::Inside,
        ..SupportTool::default()
    };
    let inside = tool.on_its_side(inside_and_out(&scene), &scene);
    assert_eq!(
        needs_at(&inside),
        [20.95],
        "only the cavity's ceiling is held from inside"
    );

    let tool = SupportTool::default();
    let outside = tool.on_its_side(inside_and_out(&scene), &scene);
    assert_eq!(
        needs_at(&outside),
        [4.95],
        "only the underside is held from outside"
    );
}

/// A box `min`..`max` from the unit cube, wound inward when `inward`.
fn box_of(min: Vec3, max: Vec3, inward: bool) -> Mesh {
    let unit = cube();
    let vertices = unit.vertices.iter().map(|at| min + *at * (max - min));
    let faces = unit
        .faces
        .iter()
        .map(|&[a, b, c]| if inward { [a, c, b] } else { [a, b, c] });
    Mesh::new(vertices.collect(), faces.collect())
}

fn joined(meshes: &[Mesh]) -> Mesh {
    let mut all = Mesh::new(Vec::new(), Vec::new());
    for mesh in meshes {
        let offset = all.vertices.len() as u32;
        all.vertices.extend(&mesh.vertices);
        all.faces
            .extend(mesh.faces.iter().map(|face| face.map(|at| at + offset)));
    }
    all
}

/// A 4 mm slab tilted 50° about Y and lifted 25 mm, and, when `hollow`, a cavity in it
/// 0.6 mm in from every face: its ceiling runs along the slab's leaning underside.
fn tilted_slab(hollow: bool) -> Scene {
    let solid = box_of(Vec3::ZERO, Vec3::new(20.0, 10.0, 4.0), false);
    let mut scene = one_cube();
    let object = &mut scene.objects_mut()[0];
    object.mesh = Arc::new(solid.clone());
    object.bvh = Arc::new(core_geometry::Bvh::build(&solid));
    object.transform.rotation = core_geometry::Quat::from_rotation_y(50f32.to_radians());
    object.transform.translation.z = 25.0;
    if hollow {
        let cavity = box_of(Vec3::splat(0.6), Vec3::new(19.4, 9.4, 3.4), true);
        let shell = joined(&[solid.clone(), cavity]);
        let settings = object
            .hollow
            .asking(&core_volume::HollowSettings::for_wall(1.0));
        object.hollow.take(core_volume::Shell {
            cavity: solid.faces.len()..shell.faces.len(),
            mesh: Arc::new(shell),
            cavity_mm3: 18.8 * 8.8 * 2.8,
            voxel_mm: 0.1,
            coarsened: false,
            shape: object.transform.unturned(),
            settings,
        });
        assert!(object.keep_support_surface());
    }
    scene
}

/// Runs `sides` one after the other over `scene`.
fn grown(scene: &mut Scene, sides: &[Side]) {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut tool = SupportTool::default();
    for side in sides {
        tool.start(scene, 0.05, &catalogue, *side)
            .expect("a model to hold up");
        let mut status = Status::default();
        while tool.poll(scene, &mut status) {
            std::thread::yield_now();
        }
    }
}

/// The contacts `sides` stand under the slab, in the model's own space.
fn under_the_slab(mut scene: Scene, sides: &[Side]) -> Vec<[u32; 3]> {
    grown(&mut scene, sides);
    let mut contacts: Vec<[u32; 3]> = scene.objects()[0]
        .supports
        .points()
        .iter()
        .filter(|point| point.contact.z < 1.0)
        .map(|point| point.contact.to_array().map(f32::to_bits))
        .collect();
    contacts.sort_unstable();
    contacts
}

#[test]
fn a_cavity_through_a_thin_wall_holds_none_of_the_outside_up() {
    let solid = under_the_slab(tilted_slab(false), &[Side::Outside]);
    assert!(!solid.is_empty(), "the slab's underside needs holding up");
    assert_eq!(
        under_the_slab(tilted_slab(true), &[Side::Outside]),
        solid,
        "the cavity's ceiling 0.6 mm through the wall holds none of the underside up"
    );
    assert_eq!(
        under_the_slab(tilted_slab(true), &[Side::Inside, Side::Outside]),
        solid,
        "nor do the supports standing under it already"
    );
}

#[test]
fn a_support_under_a_cavitys_ceiling_lands_on_its_floor() {
    let mut scene = hollow_cube();
    let object = &mut scene.objects_mut()[0];
    let transform = object.transform;
    object
        .supports
        .add(Vec3::new(10.0, 10.0, 20.95), transform, 0);

    object.refresh_supports(&SupportTool::default().table());
    assert_eq!(
        object.supports.standing(),
        1,
        "the cavity's floor is 12 mm under its ceiling, room enough to stand"
    );
}

#[test]
fn holding_the_cavity_up_again_takes_only_what_stood_inside_it() {
    let mut scene = hollow_cube();
    let object = &mut scene.objects_mut()[0];
    let transform = object.transform;
    object
        .supports
        .add(Vec3::new(10.0, 10.0, 20.95), transform, 0);
    object
        .supports
        .add(Vec3::new(10.0, 10.0, 5.0), transform, 0);

    assert_eq!(SupportTool::clear_inside(&mut scene), 1);
    let left = scene.objects()[0].supports.points();
    assert_eq!(left.len(), 1);
    assert!(
        left[0].contact.z.abs() < 1e-4,
        "the underside's support stays"
    );
}

#[test]
fn a_solid_model_has_nothing_inside() {
    let scene = one_cube();
    let tool = SupportTool::default();
    let object = &scene.objects()[0];
    let placements = vec![Placement {
        id: object.id,
        transform: object.transform,
        needs: vec![Need {
            contact: Vec3::new(0.5, 0.5, 0.5),
            reason: Reason::Steady,
        }],
    }];
    assert!(!object.supported_inside());
    let outside = tool.on_its_side(placements, &scene);
    assert_eq!(
        outside[0].needs.len(),
        1,
        "every contact of a solid model is outside"
    );
}

#[test]
fn holding_the_cavity_up_stands_supports_under_its_ceiling_and_nowhere_else() {
    let mut scene = hollow_cube();
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut tool = SupportTool::default();
    tool.start(&mut scene, 0.05, &catalogue, Side::Inside)
        .expect("a hollow model to hold up");
    let mut status = Status::default();
    while tool.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }

    let object = &scene.objects()[0];
    let points = object.supports.points();
    assert!(
        !points.is_empty(),
        "the cavity's 12 mm ceiling needs holding up: {status:?}"
    );
    for point in points {
        // The cavity spans 0.2..0.8 of the unit cube the model is built from.
        assert!(
            (point.contact.z - 0.8).abs() < 0.01,
            "{} is not under the cavity's ceiling",
            point.contact
        );
    }
}

#[test]
fn a_drilled_hole_is_held_up_inside_like_any_underside() {
    use crate::scene::{ImportSummary, Imported};
    use core_geometry::diagnose;

    let mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(20.0));
    let mut scene = Scene::default();
    let transform = Transform::from_translation(Vec3::new(0.0, 0.0, 5.0));
    let id = scene.insert(Imported::new(
        "block".to_owned(),
        Arc::new(mesh.clone()),
        transform,
        ImportSummary::made(diagnose(&mesh)),
    ));
    let object = scene.get_mut(id).expect("the block is on the plate");
    // A hole 10 mm across bored 10 mm in from the side at x = 20: a millimetre under
    // its crown it is 6 mm wide, 3 mm past the reach of a 45 degree face either side.
    let size = core_volume::HoleSize::round(10.0, 10.0, 1.0);
    let bvh = Bvh::build(&mesh);
    object.hollow.add_drain(
        &mesh,
        &bvh,
        Vec3::new(20.0, 10.0, 10.0),
        Vec3::X,
        size,
        transform,
    );
    let source = Arc::clone(object.uncut());
    let cuts = Arc::clone(object.hollow.cut_bodies().expect("the hole is cut"));
    let drilled = core_volume::Drilled::of(&source, None, &cuts).expect("a block drills");
    object.hollow.keep_drilled(&source, &cuts, Some(drilled));
    assert!(
        object.keep_support_surface(),
        "supports stand against the block drilled"
    );

    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut tool = SupportTool::default();
    tool.start(&mut scene, 0.05, &catalogue, Side::Outside)
        .expect("a model to place under");
    let mut status = Status::default();
    while tool.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }

    let in_the_hole = scene.objects()[0]
        .supports
        .points()
        .iter()
        .filter(|point| {
            let at = point.contact;
            // Over the block's underside, in from the side the hole was bored through.
            at.x > 10.0 && (at.y - 10.0).abs() < 5.0 && at.z > 1.0
        })
        .count();
    assert!(
        in_the_hole > 0,
        "the ceiling of the bore hangs over nothing: {status:?}"
    );
}

#[test]
fn a_support_edited_inside_the_cavity_is_counted_and_deleted_with_it() {
    let mut scene = hollow_cube();
    let object = &mut scene.objects_mut()[0];
    let transform = object.transform;
    object
        .supports
        .add(Vec3::new(10.0, 10.0, 20.95), transform, 0);
    object
        .supports
        .add(Vec3::new(10.0, 10.0, 5.0), transform, 0);
    object.refresh_supports(&SupportTool::default().table());
    let inside_tree = (0..object.supports.trees().len())
        .find(|tree| object.supports.trees()[*tree].root().position.z > 9.0)
        .expect("the cavity's support stands on the cavity's floor");
    object.supports.freeze(inside_tree, transform);

    assert_eq!(
        SupportTool::inside_count(&scene),
        1,
        "the frozen tree is the cavity's"
    );
    assert_eq!(SupportTool::clear_inside(&mut scene), 1);
    let object = &scene.objects()[0];
    assert!(
        object.supports.frozen().is_empty(),
        "the edited support went with the cavity"
    );
    assert_eq!(
        object.supports.points().len(),
        1,
        "the underside's support stays"
    );
    assert_eq!(SupportTool::inside_count(&scene), 0);
}

#[test]
fn a_line_starts_only_from_a_support_that_still_stands() {
    let mut scene = one_cube();
    let object = &mut scene.objects_mut()[0];
    object.transform.translation.z = 10.0;
    let (id, transform) = (object.id, object.transform);
    let at = Vec3::new(0.5, 0.5, 10.0);
    object.supports.add(at, transform, 0);
    let tool = SupportTool {
        last_placed: Some((id, transform.to_matrix().inverse().transform_point3(at))),
        ..SupportTool::default()
    };
    let start = tool
        .line_start(&scene, id)
        .expect("the last support stands");
    assert!(start.distance(at) < 1e-4, "the line starts at {start}");

    // As an undo, or a new scene whose first model reuses the id, leaves it.
    scene.objects_mut()[0].supports.clear();
    assert_eq!(tool.line_start(&scene, id), None);
}
