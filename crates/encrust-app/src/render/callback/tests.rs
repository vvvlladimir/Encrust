use super::*;
use core_geometry::{Bvh, Quat, diagnose};
use core_supports::Trapped;

use crate::scene::{ImportSummary, Imported};

/// A ball hollowed the way the window hollows one, with resin reported trapped in it.
fn a_hollowed_ball_with_resin_in_it() -> Scene {
    let mesh = Arc::new(core_volume::markers([(Vec3::splat(10.0), 10.0)]).expect("a ball"));
    let mut scene = Scene::default();
    scene.insert(Imported::new(
        "ball".to_owned(),
        Arc::clone(&mesh),
        Transform::default(),
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: core_geometry::Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ));

    let object = &mut scene.objects_mut()[0];
    let bvh = Bvh::build(&mesh);
    let asking = object
        .hollow
        .asking(&crate::hollow::HollowTool::default().settings());
    let hollowed = core_volume::hollow(&mesh, &bvh, &asking, core_volume::Cancel::never())
        .expect("a ball hollows");
    object.hollow.take(core_volume::Shell {
        mesh: Arc::new(hollowed.mesh),
        cavity: hollowed.cavity,
        cavity_mm3: hollowed.cavity_mm3,
        voxel_mm: hollowed.voxel_mm,
        coarsened: hollowed.coarsened,
        shape: core_geometry::Mat3::IDENTITY,
        settings: asking,
    });
    object.traps.set(vec![Trapped {
        at: Vec3::splat(10.0),
        volume_mm3: 100.0,
        bounds: core_geometry::Aabb::new(Vec3::splat(-50.0), Vec3::splat(50.0)),
    }]);
    scene
}

/// The whole window path from a drainage check to the picture: a model the check found
/// resin in has the faces of its own cavity drawn once more, in the colour of resin
/// that cannot get out and in a way the x-ray will not wash down.
#[test]
fn a_model_with_resin_trapped_in_it_has_its_cavity_drawn_in_its_own_colour() {
    let mut scene = a_hollowed_ball_with_resin_in_it();
    let cavity = scene.objects()[0]
        .hollow
        .cavity_faces()
        .expect("the ball is hollow");
    assert!(!cavity.is_empty(), "and the cavity has faces of its own");

    let draws = Draws::of(&scene, NOT_MARKED, None, &[], None);
    let painted = draws
        .models
        .iter()
        .filter(|draw| draw.faces() == cavity)
        .count();
    assert_eq!(painted, 1, "the cavity is drawn once over the shell");

    scene.objects_mut()[0].traps.set(Vec::new());
    let drained = Draws::of(&scene, NOT_MARKED, None, &[], None);
    assert!(
        drained.models.iter().all(|draw| draw.faces() != cavity),
        "and not at all once the resin can get out"
    );
}

fn hole() -> DrainHole {
    DrainHole {
        at: Vec3::new(0.0, 0.0, 10.0),
        axis: Vec3::NEG_Z,
        diameter_mm: 4.0,
        depth_mm: 6.0,
        taper: 1.0,
        lift_mm: 0.0,
        shape: core_volume::HoleShape::Round,
        turn_deg: 0.0,
        kind: core_volume::HoleKind::Drain,
    }
}

#[test]
fn a_hole_about_to_be_drilled_is_cut_into_its_model_and_never_capped() {
    let scene = a_hollowed_ball_with_resin_in_it();
    let mut draws = Draws::of(&scene, NOT_MARKED, None, &[], None);
    let (cuts, surfaces, solids) = (
        draws.cuts.len(),
        draws.cut_surfaces.len(),
        draws.solids.len(),
    );

    let ghost = Ghost {
        shape: GhostShape::Hole {
            id: scene.objects()[0].id,
            hole: hole(),
            into: None,
        },
        colour: Color32::WHITE,
    };
    draws.ghost(&scene, None, &ghost);
    assert_eq!(
        draws.cuts.len(),
        cuts + 1,
        "the hole is cut out of the model"
    );
    assert_eq!(
        draws.cut_surfaces.len(),
        surfaces + 1,
        "and walled the way a drilled one is"
    );
    assert_eq!(
        draws.solids.len(),
        solids,
        "it is not part of the print yet"
    );
}

#[test]
fn a_textured_model_drilled_exactly_still_has_its_holes_cut_on_screen() {
    let mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let mut scene = Scene::default();
    let id = scene.insert(Imported::new(
        "block".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary::made(diagnose(&mesh)),
    ));
    let object = scene.get_mut(id).expect("the block is on the plate");
    let size = core_volume::HoleSize::round(3.0, 5.0, 1.0);
    let at = Vec3::new(5.0, 5.0, 10.0);
    let bvh = Bvh::build(&mesh);
    object
        .hollow
        .add_drain(&mesh, &bvh, at, Vec3::Z, size, Transform::default());
    let source = Arc::clone(object.uncut());
    let cuts = Arc::clone(object.hollow.cut_bodies().expect("the hole is cut"));
    let drilled = Drilled::of(&source, None, &cuts).expect("a block drills");
    object.hollow.keep_drilled(&source, &cuts, Some(drilled));

    let plain = Draws::of(&scene, NOT_MARKED, None, &[], None);
    assert!(
        plain.cuts.is_empty(),
        "drilled, the hole is faces of the mesh"
    );

    let mapped = Arc::new(Mapped {
        names: Vec::new(),
        uvs: core_geometry::UvMap::default(),
        heights: Vec::new(),
    });
    let textures = crate::relief::Textures::from([(id, mapped)]);
    let textured = Draws::of(&scene, NOT_MARKED, Some(&textures), &[], None);
    assert_eq!(
        textured.cuts.len(),
        1,
        "drawn from its own faces, the hole is still cut by the shader"
    );
    let scene_object = scene.get(id).expect("the block");
    assert!(
        Arc::ptr_eq(&textured.cut_surfaces[0].solid.mesh, scene_object.uncut()),
        "and its inside drawn against the faces still standing in it"
    );
}

#[test]
fn a_cut_stands_where_the_model_it_was_drilled_in_stands() {
    let moved = Transform::from_translation(Vec3::new(50.0, 20.0, 0.0));
    let cuts = cuts_of(&[hole()], &[], moved);

    assert_eq!(cuts.len(), 1);
    assert!((cuts[0].mouth[0] - 50.0).abs() < 1e-4);
    assert!((cuts[0].mouth[1] - 20.0).abs() < 1e-4);
    assert!(
        cuts[0].mouth[2] > 10.0 && (cuts[0].tip[2] - 4.0).abs() < 1e-4,
        "the mouth stands clear of the surface and the tip is a depth below it"
    );
    assert!((cuts[0].mouth[3] - 2.0).abs() < 1e-4, "half the diameter");
}

#[test]
fn a_cut_turns_and_scales_with_its_model() {
    let placed = Transform {
        rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };
    let cuts = cuts_of(&[hole()], &[], placed);

    // A quarter turn about x maps the model's own +z onto the plate's -y, doubled.
    assert!(
        (cuts[0].mouth[1] + 20.0).abs() < 0.2,
        "got {:?}",
        cuts[0].mouth
    );
    assert!(
        (cuts[0].mouth[3] - 4.0).abs() < 1e-4,
        "the radius doubles too"
    );
}

#[test]
fn a_channel_is_handed_over_a_segment_at_a_time() {
    let channel = Channel {
        points: vec![
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(4.0, 0.0, 5.0),
        ],
        diameter_mm: 2.0,
        ..Channel::default()
    };
    let cuts = cuts_of(&[], &[channel], Transform::default());

    assert_eq!(cuts.len(), 2, "two segments between three points");
    assert!((cuts[1].tip[0] - 4.0).abs() < 1e-4);
}

#[test]
fn a_polygonal_hole_is_cut_to_its_corners_and_a_round_one_as_a_circle() {
    let hexagon = DrainHole {
        shape: core_volume::HoleShape::Polygon { sides: 6 },
        turn_deg: 90.0,
        ..hole()
    };
    let turned = Transform::from_scale_rotation_translation(
        Vec3::ONE,
        core_geometry::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        Vec3::ZERO,
    );
    let [cut] = cuts_of(&[hexagon], &[], turned)[..] else {
        panic!("one hole is one cut");
    };
    let corner = Vec3::new(cut.corner[0], cut.corner[1], cut.corner[2]);
    assert_eq!(cut.corner[3], 6.0, "the shader is told it has six sides");
    assert!(
        (corner.length() - 1.0).abs() < 1e-5 && corner.z.abs() < 1e-5,
        "the corner is a unit offset square to the upright axis, got {corner}"
    );
    assert_eq!(cuts_of(&[hole()], &[], turned)[0].corner, [0.0; 4]);
    let channel = Channel {
        points: vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
        diameter_mm: 2.0,
        shape: core_volume::HoleShape::Polygon { sides: 5 },
        turn_deg: 0.0,
    };
    let [leg] = cuts_of(&[], std::slice::from_ref(&channel), turned)[..] else {
        panic!("two points are one leg");
    };
    assert_eq!(
        leg.corner[3], 5.0,
        "a channel's leg is cut to its outline too"
    );

    let bent = Channel {
        points: vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 8.0, 0.0),
        ],
        ..channel
    };
    let [first, second] = cuts_of(&[], &[bent], turned)[..] else {
        panic!("three points are two legs");
    };
    assert_eq!(first.start[3], 0.0, "the channel opens at its mouth");
    assert_eq!(
        first.end, second.start,
        "two legs are cut off at the mitre they share"
    );
    assert_eq!(first.end[3], 1.0);
}

#[test]
fn a_plate_past_the_limit_is_drawn_with_its_later_holes_uncut() {
    let many: Vec<DrainHole> = (0..MAX_CUTS + 10).map(|_| hole()).collect();
    assert_eq!(cuts_of(&many, &[], Transform::default()).len(), MAX_CUTS);
}

#[test]
fn a_wall_is_never_marked_and_a_ceiling_always_is() {
    assert!(marking(0.0).abs() < 1e-6, "a wall leans nowhere");
    assert!(
        (marking(90.0) - 1.0).abs() < 1e-6,
        "a ceiling leans all the way"
    );
    assert!(
        (marking(45.0) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6,
        "a 45 degree surface marks at the sine of 45 degrees"
    );
    assert!(marking(45.0) < marking(46.0), "a slacker angle marks less");
}

/// A box of ten triangles — a cube with the two of its top missing — on a plate of
/// its own, which is a model that reaches the viewport open.
fn an_open_box() -> Scene {
    let mesh = Arc::new(Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
    ));
    let mut scene = Scene::default();
    scene.insert(Imported::new(
        "open.stl".to_owned(),
        Arc::clone(&mesh),
        Transform::default(),
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: core_geometry::Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ));
    scene
}

#[test]
fn a_model_that_is_not_closed_is_drawn_but_never_counted_into_the_cap() {
    let scene = an_open_box();
    let draws = Draws::of(&scene, NOT_MARKED, None, &[], None);

    assert_eq!(draws.models.len(), 1, "an open model is still drawn");
    assert!(
        draws.solids.is_empty(),
        "a surface with a hole has no inside for the cap to count"
    );
}

#[test]
fn a_closed_model_is_counted_into_the_cap() {
    let mut scene = an_open_box();
    let id = scene.objects()[0].id;
    let mut mesh = (*scene.get(id).expect("the box is there").mesh).clone();
    core_geometry::fill_holes(&mut mesh);
    scene
        .get_mut(id)
        .expect("the box is there")
        .reshape(crate::scene::Shaped::of(Arc::new(mesh)));

    assert_eq!(
        Draws::of(&scene, NOT_MARKED, None, &[], None).solids.len(),
        1
    );
}

#[test]
fn selection_outranks_the_repair_warning() {
    assert_eq!(color_for(true, false), theme::scene().selected);
    assert_eq!(color_for(false, false), theme::scene().unsound);
    assert_eq!(color_for(false, true), theme::scene().object);
}
