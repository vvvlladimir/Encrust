use super::*;
use crate::drain::{HoleKind, HoleShape};
use core_geometry::{cuboid, signed_volume};

/// A slab 20 mm square and 10 mm thick, its top face at z = 10.
fn slab() -> (Mesh, Bvh) {
    let mesh = cuboid(Vec3::ZERO, Vec3::new(20.0, 20.0, 10.0));
    let bvh = Bvh::build(&mesh);
    (mesh, bvh)
}

fn hole_in_the_top() -> DrainHole {
    DrainHole {
        at: Vec3::new(10.0, 10.0, 10.0),
        axis: Vec3::NEG_Z,
        diameter_mm: 4.0,
        depth_mm: 6.0,
        taper: 1.0,
        lift_mm: 0.0,
        shape: HoleShape::Round,
        turn_deg: 0.0,
        kind: HoleKind::Drain,
    }
}

/// `plugs` from `model` read afresh, as a front end asks for them the first time.
fn cut_plugs(
    model: &Mesh,
    bvh: &Bvh,
    holes: &[DrainHole],
    wall_mm: Option<Scalar>,
    gap_mm: Scalar,
) -> Result<Plugs, VolumeError> {
    plugs(&mut None, model, bvh, holes, wall_mm, gap_mm)
}

/// A bar 100 mm long, with a hole at each end of its top.
fn bar_with_holes_far_apart() -> (Mesh, Bvh, DrainHole, DrainHole) {
    let mesh = cuboid(Vec3::ZERO, Vec3::new(100.0, 20.0, 10.0));
    let bvh = Bvh::build(&mesh);
    let near = DrainHole {
        at: Vec3::new(10.0, 10.0, 10.0),
        ..hole_in_the_top()
    };
    let far = DrainHole {
        at: Vec3::new(90.0, 10.0, 10.0),
        ..hole_in_the_top()
    };
    (mesh, bvh, near, far)
}

#[test]
fn a_model_read_round_one_hole_covers_its_gap_and_its_neighbour_but_not_far_off() {
    let (mesh, _, near, far) = bar_with_holes_far_apart();
    let source = PlugSource::read(&mesh, &[near], Some(2.0));
    let beside = DrainHole {
        at: near.at + Vec3::new(3.0, 0.0, 0.0),
        ..near
    };
    assert!(
        source.covers(&[near], Some(2.0)),
        "the hole it was read for"
    );
    assert!(
        source.covers(&[beside], Some(2.0)),
        "a hole moved 3 mm, within the margin"
    );
    assert!(!source.covers(&[far], Some(2.0)), "a hole 80 mm away");
}

#[test]
fn a_hole_past_what_was_read_is_cut_from_the_model_read_again() {
    let (mesh, bvh, near, far) = bar_with_holes_far_apart();
    let mut source = Some(PlugSource::read(&mesh, &[near], Some(2.0)));
    let made = plugs(&mut source, &mesh, &bvh, &[far], Some(2.0), 0.1).expect("sound");
    // Cut from the piece round the other hole, it would meet nothing and be sampled.
    assert_eq!(
        made.sampled, 0,
        "the far plug is cut from the model, not sampled"
    );
    assert!(
        source.is_some_and(|read| read.covers(&[far], Some(2.0))),
        "and the piece read round it is left for the next cut"
    );
}

#[test]
fn a_gap_that_closes_a_widening_hole_at_its_lifted_mouth_leaves_no_plug() {
    let (mesh, bvh) = slab();
    let widening = DrainHole {
        taper: 2.0,
        ..hole_in_the_top()
    };
    // 2 mm of radius at the lifted mouth, still 2.0165 at the surface: 2.01 closes only
    // the first.
    let made = cut_plugs(&mesh, &bvh, &[widening], Some(2.0), 2.01).expect("sound");
    assert_eq!(made.closed, 1);
    assert!(made.meshes[0].is_empty(), "the closed hole has no plug");
}

#[test]
fn a_hole_the_gap_closes_leaves_the_plugs_of_the_others() {
    let (mesh, bvh, near, far) = bar_with_holes_far_apart();
    let pin = DrainHole {
        diameter_mm: 0.2,
        ..near
    };
    let made = cut_plugs(&mesh, &bvh, &[pin, far], Some(2.0), 0.15).expect("sound");
    assert_eq!(
        made.closed, 1,
        "a 0.2 mm hole has nothing left at 0.15 mm a side"
    );
    assert!(made.meshes[0].is_empty());
    assert!(
        signed_volume(&made.meshes[1]) > 0.0,
        "the 4 mm hole beside it still gets its plug"
    );
}

#[test]
fn a_cone_the_gap_closes_short_of_its_end_is_cut_off_where_it_does() {
    let (mesh, bvh) = slab();
    // 2 mm of radius at the mouth narrowing to 0.2 mm 6 mm in: a 0.3 mm gap closes it
    // where the hole's own radius is 0.35 mm, 5.5 mm in, less the lift and prism's inset.
    let cone = DrainHole {
        taper: 0.1,
        ..hole_in_the_top()
    };
    let made = cut_plugs(&mesh, &bvh, &[cone], None, 0.3).expect("sound");
    assert_eq!(made.closed, 0);
    let bounds = made.meshes[0].aabb().expect("the cone has a plug");
    assert!(
        (bounds.mins.z - 4.5).abs() < 0.02,
        "the plug ends 5.5 mm under the 10 mm top, got {bounds:?}"
    );
}

#[test]
fn holes_far_apart_keep_only_the_model_round_each() {
    let (mesh, bvh, near, far) = bar_with_holes_far_apart();
    let mut source = None;
    plugs(&mut source, &mesh, &bvh, &[near, far], Some(2.0), 0.1).expect("sound");
    let read = source.expect("a piece of the bar is kept");
    let middle = DrainHole {
        at: Vec3::new(50.0, 10.0, 10.0),
        ..near
    };
    assert!(
        read.covers(&[near, far], Some(2.0)),
        "both holes it was read for"
    );
    assert!(
        !read.covers(&[middle], Some(2.0)),
        "but not the 70 mm of bar between them"
    );
}

#[test]
fn a_model_held_whole_is_not_kept_for_the_next_cut() {
    let (mesh, bvh) = slab();
    let everywhere = DrainHole {
        diameter_mm: 40.0,
        depth_mm: 12.0,
        ..hole_in_the_top()
    };
    let mut source = None;
    plugs(&mut source, &mesh, &bvh, &[everywhere], None, 0.1).expect("sound");
    assert!(
        source.is_none(),
        "a box round the whole slab crops nothing worth keeping"
    );
}

#[test]
fn a_plug_drilled_over_an_earlier_hole_has_that_hole_through_it() {
    let (mesh, bvh) = slab();
    let narrow = DrainHole {
        diameter_mm: 3.0,
        ..hole_in_the_top()
    };
    let wide = DrainHole {
        diameter_mm: 10.0,
        ..hole_in_the_top()
    };
    let volume = |plug: &Mesh| signed_volume(plug);
    let made = cut_plugs(&mesh, &bvh, &[narrow, wide], Some(2.0), 0.1).expect("sound");
    let (alone, taken) = (
        the_plug(wide, Some(2.0), 0.1),
        the_plug(narrow, Some(2.0), 0.0),
    );
    // The wide plug less what the narrow hole took out of the same 2 mm of wall.
    let expected = volume(&alone) - volume(&taken);
    assert!(
        (volume(&made.meshes[1]) - expected).abs() < 1e-2,
        "the 10 mm plug has the 3 mm hole through it: {} against {expected}",
        volume(&made.meshes[1])
    );
    assert!(
        (volume(&made.meshes[0]) - volume(&the_plug(narrow, Some(2.0), 0.1))).abs() < 1e-3,
        "the 3 mm plug, drilled first, is whole and closes that hole"
    );

    let reversed = cut_plugs(&mesh, &bvh, &[wide, narrow], Some(2.0), 0.1).expect("sound");
    assert!(
        (volume(&reversed.meshes[0]) - volume(&alone)).abs() < 1e-3,
        "a hole drilled later leaves an earlier plug whole"
    );
}

#[test]
fn a_hole_inside_an_earlier_one_leaves_an_empty_plug_and_no_sampled_one() {
    let (mesh, bvh) = slab();
    let wide = DrainHole {
        diameter_mm: 10.0,
        ..hole_in_the_top()
    };
    // The narrow hole lies wholly in the wide one before it, so nothing is left to cut.
    let made = cut_plugs(&mesh, &bvh, &[wide, hole_in_the_top()], Some(2.0), 0.1).expect("sound");
    assert_eq!(made.sampled, 0, "the model was read, so nothing is sampled");
    assert!(!made.meshes[0].is_empty(), "the wide hole has its plug");
    assert!(
        made.meshes[1].is_empty(),
        "which already fills the narrow one"
    );
}

#[test]
fn an_unsound_hole_fails_before_the_model_is_read() {
    let (mesh, bvh) = slab();
    let nowhere = DrainHole {
        axis: Vec3::ZERO,
        ..hole_in_the_top()
    };
    let mut source = None;
    let made = plugs(&mut source, &mesh, &bvh, &[nowhere], None, 0.1);
    assert_eq!(made, Err(VolumeError::BadDrain { index: 0 }));
    assert!(
        source.is_none(),
        "nothing was read for a cut that cannot be made"
    );
}

fn the_plug(hole: DrainHole, wall_mm: Option<Scalar>, gap_mm: Scalar) -> Mesh {
    let (mesh, bvh) = slab();
    let mut made = cut_plugs(&mesh, &bvh, &[hole], wall_mm, gap_mm).expect("the hole is sound");
    assert_eq!(made.sampled, 0, "a closed slab is cut, not sampled");
    made.meshes.pop().expect("one plug a hole")
}

#[test]
fn a_plug_through_a_wall_fills_the_wall_less_its_gap() {
    let plug = the_plug(hole_in_the_top(), Some(2.0), 0.1);
    let volume = signed_volume(&plug);
    // The hole is a 24-gon with its corners on 2 mm, so its sides stand 2 cos(pi / 24)
    // off the axis; the plug's stand 0.1 mm in from those, and a regular n-gon of
    // apothem a holds n a^2 tan(pi / n). Over the 2 mm wall.
    let sides: Scalar = 24.0;
    let half_angle = std::f32::consts::PI / sides;
    let apothem = 2.0 * half_angle.cos() - 0.1;
    let expected = sides * apothem * apothem * half_angle.tan() * 2.0;
    assert!(
        (volume - expected).abs() < 1e-3 * expected,
        "a plug 0.1 mm in from the hole's sides holds {expected} mm3, got {volume}"
    );
}

#[test]
fn a_plug_drilled_over_a_point_is_the_point_it_took() {
    let pyramid = Mesh::new(
        vec![
            Vec3::new(-5.0, -5.0, 0.0),
            Vec3::new(5.0, -5.0, 0.0),
            Vec3::new(5.0, 5.0, 0.0),
            Vec3::new(-5.0, 5.0, 0.0),
            Vec3::new(0.0, 0.0, 10.0),
        ],
        vec![
            [0, 3, 2],
            [0, 2, 1],
            [0, 1, 4],
            [1, 2, 4],
            [2, 3, 4],
            [3, 0, 4],
        ],
    );
    let bvh = Bvh::build(&pyramid);
    let over_the_apex = DrainHole {
        at: Vec3::new(0.0, 0.0, 10.0),
        diameter_mm: 6.0,
        depth_mm: 3.0,
        ..hole_in_the_top()
    };
    let made = cut_plugs(&pyramid, &bvh, &[over_the_apex], None, 0.1).expect("the hole is sound");
    let volume = signed_volume(&made.meshes[0]);
    // 3 mm down a pyramid of base 10 and height 10 it is 3 mm square, all inside the
    // hole, so the plug is the tip itself: a third of 9 mm2 over 3 mm.
    assert!(
        (volume - 9.0).abs() < 1e-3,
        "the plug is the 9 mm3 tip the hole took, not a cylinder, got {volume}"
    );
}

#[test]
fn a_torn_model_is_patched_for_the_engine_and_still_gives_the_point_it_took() {
    let pyramid = Mesh::new(
        vec![
            Vec3::new(-5.0, -5.0, 0.0),
            Vec3::new(5.0, -5.0, 0.0),
            Vec3::new(5.0, 5.0, 0.0),
            Vec3::new(-5.0, 5.0, 0.0),
            Vec3::new(0.0, 0.0, 10.0),
        ],
        // Half the base is missing, so the engine refuses the model as it stands.
        vec![[0, 3, 2], [0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
    );
    let bvh = Bvh::build(&pyramid);
    let over_the_apex = DrainHole {
        at: Vec3::new(0.0, 0.0, 10.0),
        diameter_mm: 6.0,
        depth_mm: 3.0,
        ..hole_in_the_top()
    };
    let made = cut_plugs(&pyramid, &bvh, &[over_the_apex], None, 0.1).expect("the hole is sound");
    let volume = signed_volume(&made.meshes[0]);
    // As in the closed pyramid: the 3 mm tip, a third of 9 mm2 over 3 mm.
    assert!(
        (volume - 9.0).abs() < 1e-3,
        "a tear far from the hole still leaves the 9 mm3 tip, not a cylinder, got {volume}"
    );
}

#[test]
fn a_plug_no_patch_can_cut_is_sampled_and_counted() {
    // A lone square has no inside, so even closed over it leaves the hole nothing.
    let sheet = Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(20.0, 0.0, 10.0),
            Vec3::new(20.0, 20.0, 10.0),
            Vec3::new(0.0, 20.0, 10.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    );
    let bvh = Bvh::build(&sheet);
    let made = cut_plugs(&sheet, &bvh, &[hole_in_the_top()], Some(2.0), 0.1).expect("sound");
    assert_eq!(made.sampled, 1, "the one plug is sampled, and said to be");
    assert_eq!(made.meshes.len(), 1, "a hole still gets its plug");
}

#[test]
fn a_torn_model_still_gets_a_plug_through_its_wall() {
    let (mut mesh, _) = slab();
    let underneath = mesh
        .faces
        .iter()
        .position(|face| face.iter().all(|at| mesh.vertices[*at as usize].z == 0.0))
        .expect("the slab has a floor");
    mesh.faces.remove(underneath);
    let bvh = Bvh::build(&mesh);
    let made = cut_plugs(&mesh, &bvh, &[hole_in_the_top()], Some(2.0), 0.1).expect("sound");
    let bounds = made.meshes[0].aabb().expect("a plug has vertices");
    assert!(
        (bounds.maxs.z - 10.0).abs() < 1e-4 && (bounds.mins.z - 8.0).abs() < 1e-4,
        "a torn floor still leaves a plug through the 2 mm wall: {bounds:?}"
    );
}

#[test]
fn a_plug_is_flush_with_the_surface_it_closes() {
    let plug = the_plug(hole_in_the_top(), Some(2.0), 0.1);
    let bounds = plug.aabb().expect("a plug has vertices");
    assert!(
        (bounds.maxs.z - 10.0).abs() < 1e-4 && (bounds.mins.z - 8.0).abs() < 1e-4,
        "the plug runs from the slab's top down through the 2 mm wall, got {bounds:?}"
    );
}

#[test]
fn a_walled_hole_takes_a_plug_as_deep_as_itself_through_any_wall() {
    let walled = DrainHole {
        kind: HoleKind::Walled,
        ..hole_in_the_top()
    };
    let plug = the_plug(walled, Some(2.0), 0.1);
    let bounds = plug.aabb().expect("a plug has vertices");
    assert!(
        (bounds.maxs.z - 10.0).abs() < 1e-4 && (bounds.mins.z - 4.0).abs() < 1e-4,
        "the plug fills the hole's 6 mm, not the 2 mm wall, got {bounds:?}"
    );
}

#[test]
fn a_hole_in_the_supports_takes_no_plug_and_is_not_counted_closed() {
    let (mesh, bvh) = slab();
    let support = DrainHole {
        kind: HoleKind::Support,
        ..hole_in_the_top()
    };
    let made = cut_plugs(&mesh, &bvh, &[support], Some(2.0), 0.1).expect("the hole is sound");
    assert!(made.meshes[0].is_empty());
    assert_eq!(made.closed, 0);
}

#[test]
fn a_square_hole_takes_a_square_plug() {
    let square = DrainHole {
        shape: HoleShape::Polygon { sides: 4 },
        ..hole_in_the_top()
    };
    let plug = the_plug(square, Some(2.0), 0.0);
    let volume = signed_volume(&plug);
    // Corners on a 2 mm radius make a square of side 2 root 2, 8 mm2, over 2 mm.
    assert!(
        (volume - 16.0).abs() < 1e-3,
        "a square plug with no gap fills the square hole, got {volume}"
    );
}

#[test]
fn a_plug_stands_on_the_end_that_goes_in() {
    let plug = the_plug(hole_in_the_top(), Some(2.0), 0.1);
    let stood = stand_on_end(&plug, Vec3::NEG_Z);
    let bounds = stood.aabb().expect("a plug has vertices");
    assert!(bounds.mins.z.abs() < 1e-5, "it stands on the plate");
    assert!(
        (bounds.maxs.z - 2.0).abs() < 1e-4,
        "drilled downward, the plug already stands on its inner end, 2 mm tall: {bounds:?}"
    );
    assert!(
        (bounds.mins.x + bounds.maxs.x).abs() < 1e-4,
        "its footprint is centred on the origin"
    );
    assert!(signed_volume(&stood) > 0.0, "a turn keeps it wound outward");
}

#[test]
fn a_tapered_plug_narrows_with_the_hole_as_it_is_cut_through_the_wall() {
    let shallow = DrainHole {
        depth_mm: 2.0,
        taper: 0.5,
        ..hole_in_the_top()
    };
    let cut = crate::pierce(&[shallow], 4.0, 0.2);
    let plug = the_plug(cut[0], Some(4.0), 0.1);
    let inner_x = plug
        .vertices
        .iter()
        .filter(|vertex| (vertex.z - 6.0).abs() < 1e-4)
        .map(|vertex| vertex.x - 10.0)
        .fold(Scalar::MIN, Scalar::max);
    // Pierced to 4 + 2 voxels from a mouth lifted MOUTH_LIFT_MM, the cone halves over
    // 4.45 mm, so 4 mm in a corner is 2 (1 - 0.5 * 4.05 / 4.45) out, less the gap's reach.
    let inset = 0.1 / (std::f32::consts::PI / 24.0).cos();
    let expected = 2.0 * (1.0 - 0.5 * 4.05 / 4.45) - inset;
    assert!(
        (inner_x - expected).abs() < 1e-3,
        "the plug's inner end is the hole there less its gap, {expected} mm, got {inner_x}"
    );
}

#[test]
fn plugs_are_laid_in_a_row_on_the_plate_past_their_model() {
    let plug = the_plug(hole_in_the_top(), Some(2.0), 0.1);
    let beside = Aabb::new(Vec3::ZERO, Vec3::new(20.0, 20.0, 10.0));
    let laid = laid_beside(
        &[plug.clone(), plug],
        &[hole_in_the_top(); 2],
        Transform::default(),
        beside,
    );
    let bounds: Vec<Aabb> = laid
        .iter()
        .map(|plug| plug.aabb().expect("a plug has vertices"))
        .collect();
    assert!(
        (bounds[0].mins.x - (20.0 + PLUG_SPACING_MM)).abs() < 1e-4
            && (bounds[1].mins.x - (bounds[0].maxs.x + PLUG_SPACING_MM)).abs() < 1e-4,
        "a spacing past the model, then a spacing past the plug before: {bounds:?}"
    );
    assert!(
        bounds
            .iter()
            .all(|b| b.mins.z.abs() < 1e-5 && b.mins.y.abs() < 1e-4),
        "each stands on the plate, level with the model's front: {bounds:?}"
    );
}

#[test]
fn a_gap_wider_than_the_hole_leaves_no_plug() {
    let (mesh, bvh) = slab();
    let made = cut_plugs(&mesh, &bvh, &[hole_in_the_top()], Some(2.0), 2.5).expect("sound");
    assert_eq!(made.closed, 1);
    assert!(made.meshes[0].is_empty());
}
