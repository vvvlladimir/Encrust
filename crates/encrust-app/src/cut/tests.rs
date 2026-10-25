use super::*;
use crate::scene::{ImportSummary, Imported};
use core_geometry::{diagnose, signed_volume};
use std::sync::Arc;

/// The tool set to one plane: where it crosses `axis`, and which halves to keep.
fn at(axis: Axis, height_mm: Scalar, keep: Keep) -> CutTool {
    CutTool {
        state: CutState {
            axis,
            height_mm,
            keep,
            joints: Joints::default(),
            parts: core_volume::PartSettings::default(),
        },
        ..CutTool::default()
    }
}

/// Runs the cut `tool` is set to through to the end, as the window's frames would.
fn cut_now(mut tool: CutTool, scene: &mut Scene, id: ObjectId) -> Status {
    if let Err(error) = tool.start(scene, id) {
        return Status::Error(error.to_string());
    }
    let mut status = Status::default();
    while tool.poll(scene, &mut status) {
        std::thread::yield_now();
    }
    status
}

/// Runs a split of `id` through to the end, as the window's frames would.
fn split_now(scene: &mut Scene, id: ObjectId) -> Status {
    let mut tool = CutTool::default();
    if let Err(error) = tool.start_split(scene, id) {
        return Status::Error(error.to_string());
    }
    let mut status = Status::default();
    while tool.poll(scene, &mut status) {
        std::thread::yield_now();
    }
    status
}

/// Four 4 mm pegs 5 mm long of `kind`, two to a piece, all of a width.
fn joined_by(kind: JointKind) -> Joints {
    Joints {
        kind: Some(kind),
        pegs: core_joint::Pegs {
            auto_size: false,
            diameter_mm: 4.0,
            length_mm: 5.0,
            keyed: false,
            ..core_joint::Pegs::default()
        },
        ..Joints::default()
    }
}

/// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
fn cuboid(size: Vec3) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, size)
}

fn scene_with(mesh: Mesh, transform: Transform) -> (Scene, ObjectId) {
    let mesh = Arc::new(mesh);
    let mut scene = Scene::default();
    let id = scene.insert(Imported::new(
        "cube".to_owned(),
        Arc::clone(&mesh),
        transform,
        ImportSummary::made(diagnose(&mesh)),
    ));
    (scene, id)
}

#[test]
fn a_cut_leaves_two_models_where_there_was_one() {
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
    let status = cut_now(at(Axis::Z, 4.0, Keep::Both), &mut scene, id);
    assert!(!status.is_error());
    assert_eq!(scene.objects().len(), 2);

    let volumes: Vec<f32> = scene
        .objects()
        .iter()
        .map(|object| signed_volume(&object.mesh))
        .collect();
    assert!(volumes.iter().any(|volume| (volume - 400.0).abs() < 1e-1));
    assert!(volumes.iter().any(|volume| (volume - 600.0).abs() < 1e-1));
}

#[test]
fn the_cut_is_made_where_the_model_stands() {
    // The cube is lifted 20 mm, so a plane at 4 mm passes under it entirely.
    let lifted = Transform::from_translation(Vec3::new(0.0, 0.0, 20.0));
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), lifted);
    let status = cut_now(at(Axis::Z, 4.0, Keep::Both), &mut scene, id);
    assert!(
        status.is_error(),
        "a plane that misses the model cuts nothing"
    );
    assert_eq!(scene.objects().len(), 1, "and the model is left alone");
}

#[test]
fn a_half_laid_flat_stands_on_its_cut_face() {
    let (mut scene, id) = scene_with(cuboid(Vec3::new(10.0, 10.0, 20.0)), Transform::default());
    let tool = CutTool {
        lay_flat: true,
        ..at(Axis::X, 0.0, Keep::Below)
    };
    cut_now(tool, &mut scene, id);

    // The half is 5 mm across X; lying on the face the cut left, that is its height.
    let bounds = scene.objects()[0]
        .world_bounds()
        .expect("a half has bounds");
    let height = bounds.maxs.z - bounds.mins.z;
    assert!((height - 5.0).abs() < 1e-3, "got {height}");
    assert!(bounds.mins.z.abs() < 1e-3, "and stands on the plate");
}

#[test]
fn pins_stand_on_a_lower_half_that_stays_upright_and_sockets_go_into_the_upper() {
    let (mut scene, id) = scene_with(cuboid(Vec3::new(40.0, 10.0, 20.0)), Transform::default());
    let mut tool = at(Axis::Z, 10.0, Keep::Both);
    tool.state.joints = joined_by(JointKind::Pins);
    tool.lay_flat = true;
    let status = cut_now(tool, &mut scene, id);
    let Status::Noted(_, notes) = &status else {
        panic!("a block through the middle is a cut, got {status:?}");
    };
    assert_eq!(
        notes,
        &[tr!("cut-with-pins", count = 2)],
        "two pins are named"
    );

    let [lower, upper] = scene.objects() else {
        panic!("two halves, got {}", scene.objects().len());
    };
    let top = lower.world_bounds().expect("a half has bounds").maxs.z;
    assert!(
        (top - 15.0).abs() < 1e-3,
        "5 mm pins stand on the 10 mm cut, so it is not laid on it, got {top}"
    );
    assert!(
        lower.summary.diagnostics.is_sound(),
        "a pin is welded into its half"
    );
    assert!(
        signed_volume(&upper.mesh) < 4000.0 - 2.0 * 50.0,
        "two sockets of some 4.2 mm by 5.3 mm are taken out of the upper half"
    );
}

#[test]
fn dowels_stand_on_the_plate_beside_the_lower_half() {
    let (mut scene, id) = scene_with(cuboid(Vec3::new(40.0, 10.0, 20.0)), Transform::default());
    let mut tool = at(Axis::Z, 10.0, Keep::Both);
    tool.state.joints = joined_by(JointKind::Dowels);
    let status = cut_now(tool, &mut scene, id);
    assert!(!status.is_error(), "got {status:?}");

    let [lower, _, dowels @ ..] = scene.objects() else {
        panic!("two halves and their dowels");
    };
    assert_eq!(dowels.len(), 2, "one dowel a pair of sockets");
    let edge = lower.world_bounds().expect("a half has bounds").maxs.x;
    for dowel in dowels {
        let bounds = dowel.world_bounds().expect("a dowel has bounds");
        assert!(
            bounds.mins.z.abs() < 1e-3,
            "{} stands on the plate",
            dowel.name
        );
        assert!(bounds.mins.x > edge, "{} is clear of the half", dowel.name);
    }
}

#[test]
fn keeping_one_half_throws_the_other_away() {
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
    cut_now(at(Axis::Z, 6.0, Keep::Below), &mut scene, id);
    assert_eq!(scene.objects().len(), 1);
    let volume = signed_volume(&scene.objects()[0].mesh);
    assert!((volume - 600.0).abs() < 1e-1, "got {volume}");
}

#[test]
fn a_hole_drilled_before_the_cut_stays_in_the_half_it_was_drilled_into() {
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(20.0)), Transform::default());
    let object = scene.get_mut(id).expect("the cube");
    let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
    let size = core_volume::HoleSize::round(4.0, 6.0, 1.0);
    let top = Vec3::new(10.0, 10.0, 20.0);
    object
        .hollow
        .add_drain(&mesh, &bvh, top, Vec3::Z, size, Transform::default());
    cut_now(at(Axis::Z, 10.0, Keep::Above), &mut scene, id);

    // A 24-gon with its corners on 2 mm holds 12 r^2 sin(pi / 12), 6 mm deep.
    let bore = 12.0 * 4.0 * (std::f32::consts::PI / 12.0).sin() * 6.0;
    let volume = signed_volume(&scene.objects()[0].mesh);
    let expected = 4000.0 - bore;
    assert!(
        (volume - expected).abs() < 1e-3 * expected,
        "the upper half is 4000 mm3 less the {bore} mm3 bore, {expected}, got {volume}"
    );
}

#[test]
fn a_model_its_holes_cannot_be_cut_into_is_left_whole() {
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(2.0)), Transform::default());
    let object = scene.get_mut(id).expect("the cube");
    let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
    // Wider and deeper than the cube, so taking it out leaves nothing.
    let size = core_volume::HoleSize::round(8.0, 4.0, 1.0);
    let top = Vec3::new(1.0, 1.0, 2.0);
    object
        .hollow
        .add_drain(&mesh, &bvh, top, Vec3::Z, size, Transform::default());
    let status = cut_now(at(Axis::Z, 1.0, Keep::Both), &mut scene, id);

    assert!(
        matches!(&status, Status::Error(said) if *said == tr!("cut-undrilled", name = "cube")),
        "got {status:?}"
    );
    assert_eq!(scene.objects().len(), 1, "the model stays as it was");
    assert!(scene.get(id).is_some());
}

#[test]
fn cutting_across_the_other_axes_is_the_same_cut() {
    // The cube's centre of mass is at 5 mm, so 2 mm short of it is 3 mm into the cube.
    for (axis, expected) in [(Axis::X, 300.0), (Axis::Y, 300.0)] {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
        cut_now(at(axis, -2.0, Keep::Below), &mut scene, id);

        let volume = signed_volume(&scene.objects()[0].mesh);
        assert!(
            (volume - expected).abs() < 1e-1,
            "cutting on {} gave {volume}",
            axis.label()
        );
    }
}

#[test]
fn a_cut_across_x_follows_the_model_across_the_plate() {
    let moved = Transform::from_translation(Vec3::new(40.0, 25.0, 0.0));
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), moved);
    let status = cut_now(at(Axis::X, 0.0, Keep::Below), &mut scene, id);
    assert!(
        !status.is_error(),
        "a plane through the centre of mass meets the model"
    );
    let volume = signed_volume(&scene.objects()[0].mesh);
    assert!(
        (volume - 500.0).abs() < 1e-1,
        "through the middle of a 10 mm cube leaves half of 1000 mm³, got {volume}"
    );
}

#[test]
fn turning_the_plane_puts_it_through_the_middle_of_the_model() {
    let lifted = Transform::from_translation(Vec3::new(0.0, 0.0, 20.0));
    let (scene, id) = scene_with(cuboid(Vec3::splat(10.0)), lifted);
    let object = scene.get(id).expect("the cube was just inserted");
    let mut tool = CutTool::default();

    tool.set_axis(Axis::Z, object);
    assert!(
        (tool.position_mm(object) - 25.0).abs() < 1e-4,
        "20 mm up, 5 mm in"
    );
    tool.set_axis(Axis::Y, object);
    assert!(
        (tool.position_mm(object) - 5.0).abs() < 1e-4,
        "the cube spans 0..10 in Y"
    );
}

#[test]
fn picking_a_model_puts_the_plane_halfway_up_it_once() {
    let (scene, id) = scene_with(cuboid(Vec3::new(10.0, 10.0, 30.0)), Transform::default());
    let object = scene.get(id).expect("the box was just inserted");
    let mut tool = CutTool::default();

    tool.centre_on(id, object);
    assert!(
        (tool.position_mm(object) - 15.0).abs() < 1e-4,
        "half of a 30 mm tall box"
    );
    tool.state.height_mm = 4.0;
    tool.centre_on(id, object);
    assert!(
        (tool.state.height_mm - 4.0).abs() < 1e-6,
        "a height typed for the same model stays"
    );
}

/// The cuboid with the two faces of its top missing: every half of it comes off the
/// cut open, as a model does whose surface was already torn.
fn open_cuboid(size: Vec3) -> Mesh {
    let mut mesh = cuboid(size);
    let vertices = mesh.vertices.clone();
    mesh.faces
        .retain(|face| !face.iter().all(|&at| vertices[at as usize].z == size.z));
    mesh
}

#[test]
fn a_half_that_comes_off_the_cut_broken_is_mended_before_it_lands() {
    let open = open_cuboid(Vec3::splat(10.0));
    assert!(
        !diagnose(&open).is_sound(),
        "the model is open before it is cut"
    );
    let (mut scene, id) = scene_with(open, Transform::default());

    let status = cut_now(at(Axis::Z, 4.0, Keep::Both), &mut scene, id);

    assert!(!status.is_error());
    assert_eq!(scene.objects().len(), 2);
    for object in scene.objects() {
        assert!(
            object.summary.is_sound(),
            "{} came down mended, got {:?}",
            object.name,
            object.summary.defects()
        );
    }
}

#[test]
fn a_half_that_comes_off_the_cut_sound_is_left_alone() {
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());

    let status = cut_now(at(Axis::Z, 4.0, Keep::Both), &mut scene, id);
    assert!(
        matches!(status, Status::Info(_)),
        "a cut with nothing mended notes nothing, got {status:?}"
    );
}

#[test]
fn splitting_a_cut_model_gives_a_part_each() {
    let mut both = cuboid(Vec3::splat(4.0));
    let away = cuboid(Vec3::splat(4.0));
    let offset = both.vertices.len() as u32;
    both.vertices
        .extend(away.vertices.iter().map(|v| *v + Vec3::new(20.0, 0.0, 0.0)));
    both.faces.extend(
        away.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );

    let (mut scene, id) = scene_with(both, Transform::default());
    let status = split_now(&mut scene, id);
    assert!(!status.is_error(), "got {status:?}");
    assert_eq!(scene.objects().len(), 2);
}

/// Splits `id` with a part marked at `at` on the plate, through to the end.
fn part_off_now(tool: &mut CutTool, scene: &mut Scene, id: ObjectId, at: Vec3) -> Status {
    let object = scene.get(id).expect("the model is on the plate");
    tool.mark_part(object, at);
    if let Err(error) = tool.start_split(scene, id) {
        return Status::Error(error.to_string());
    }
    let mut status = Status::default();
    while tool.poll(scene, &mut status) {
        std::thread::yield_now();
    }
    status
}

#[test]
fn a_part_marked_on_a_moved_dumbbell_is_cut_off_and_the_mark_is_spent() {
    let solid = |min: Vec3, max: Vec3| {
        core_csg::Solid::new(&core_geometry::cuboid(min, max)).expect("a box is a solid")
    };
    let dumbbell = solid(Vec3::ZERO, Vec3::splat(10.0))
        .union(&solid(Vec3::new(10.0, 4.0, 4.0), Vec3::new(20.0, 6.0, 6.0)))
        .and_then(|joined| joined.union(&solid(Vec3::new(20.0, 0.0, 0.0), Vec3::splat(30.0))))
        .expect("three boxes in a row join")
        .mesh();
    let moved = Transform::from_translation(Vec3::new(5.0, 0.0, 0.0));
    let (mut scene, id) = scene_with(dumbbell, moved);
    let mut tool = CutTool::default();
    let status = part_off_now(&mut tool, &mut scene, id, Vec3::new(5.0, 5.0, 5.0));
    assert!(!status.is_error(), "got {status:?}");
    assert!(
        tool.part_mark.is_none(),
        "the mark went with the model it was on"
    );
    let [part, rest] = scene.objects() else {
        panic!("the marked part and the rest replace the model");
    };
    let ends = |object: &SceneObject| object.world_bounds().expect("a side has faces");
    assert!(
        ends(part).maxs.x < 25.0 && ends(rest).mins.x > 15.0,
        "the part is the box the mark is on, cut across the bar 15 to 25 mm on the plate"
    );
}

#[test]
fn a_part_marked_on_a_box_has_no_neck_and_the_box_stays() {
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(4.0)), Transform::default());
    let mut tool = CutTool::default();
    let status = part_off_now(&mut tool, &mut scene, id, Vec3::new(0.0, 2.0, 2.0));
    assert_eq!(status, Status::Error(tr!("cut-no-neck")));
    assert_eq!(scene.objects().len(), 1);
}

#[test]
fn splitting_one_solid_leaves_it_alone() {
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(4.0)), Transform::default());
    let status = split_now(&mut scene, id);
    assert!(!status.is_error());
    assert_eq!(scene.objects().len(), 1);
    assert_eq!(scene.objects()[0].id, id, "the model is the same one");
}

#[test]
fn a_scrap_hanging_beside_a_model_is_dropped_and_the_model_kept_as_one() {
    let mut model = cuboid(Vec3::splat(4.0));
    let scrap = cuboid(Vec3::splat(0.5));
    let offset = model.vertices.len() as u32;
    model.vertices.extend(
        scrap
            .vertices
            .iter()
            .map(|v| *v + Vec3::new(10.0, 0.0, 0.0)),
    );
    model.faces.extend(
        scrap
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
    let (mut scene, id) = scene_with(model, Transform::default());
    let status = split_now(&mut scene, id);
    let Status::Noted(_, notes) = &status else {
        panic!("a split that dropped a scrap notes it, got {status:?}");
    };
    let [object] = scene.objects() else {
        panic!("the scrap is no part of its own");
    };
    assert_eq!(object.name, "cube", "the model keeps its name");
    assert!((signed_volume(&object.mesh) - 64.0).abs() < 1e-3);
    assert_eq!(notes, &[tr!("cut-split-dropped", count = 1)]);
}

#[test]
fn a_part_marked_on_a_mesh_since_replaced_marks_nothing() {
    let (mut scene, id) = scene_with(cuboid(Vec3::splat(4.0)), Transform::default());
    let mut tool = CutTool::default();
    let object = scene.get(id).expect("the model is on the plate");
    tool.mark_part(object, Vec3::new(0.0, 2.0, 2.0));
    assert!(tool.marked_on(object).is_some());
    let replaced = crate::scene::Shaped::of(Arc::new(cuboid(Vec3::splat(6.0))));
    let object = scene.get_mut(id).expect("the model is on the plate");
    object.reshape(replaced);
    assert_eq!(
        tool.marked_on(object),
        None,
        "the mark was on the mesh that went"
    );
}
