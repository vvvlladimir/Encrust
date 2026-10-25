use super::*;
use core_geometry::signed_volume;

fn hole() -> DrainHole {
    DrainHole {
        at: Vec3::ZERO,
        axis: Vec3::NEG_Z,
        diameter_mm: 2.0,
        depth_mm: 10.0,
        taper: 1.0,
        lift_mm: 1.0,
        shape: HoleShape::Round,
        turn_deg: 0.0,
        walled: false,
    }
}

#[test]
fn a_cylindrical_hole_encloses_its_own_volume() {
    let cut = drill(&[hole()], &[]).expect("a 2 mm hole drills");
    // The mouth of this one stands a millimetre clear of the surface, so the tube is
    // depth + lift long, and a 24-sided prism is a hair under the circle it stands in.
    let expected = std::f32::consts::PI * 1.0 * 11.0;
    let volume = -signed_volume(&cut) / CUT_WEIGHT as Scalar;
    assert!(
        (volume - expected).abs() < 0.03 * expected,
        "a 2 mm by 11 mm tube holds {expected} mm3, got {volume}"
    );
}

#[test]
fn a_square_hole_holds_the_square_its_circle_bounds() {
    let square = DrainHole {
        shape: HoleShape::Polygon { sides: 4 },
        ..hole()
    };
    let cut = drill(&[square], &[]).expect("a square hole drills");
    // Corners on a 1 mm radius make a square of side root two, 2 mm2, over 11 mm.
    let expected = 2.0 * 11.0;
    let volume = -signed_volume(&cut) / CUT_WEIGHT as Scalar;
    assert!(
        (volume - expected).abs() < 1e-3 * expected,
        "a square in a 2 mm circle by 11 mm holds {expected} mm3, got {volume}"
    );
}

#[test]
fn a_turn_moves_a_polygons_corners_about_its_axis() {
    let square = |turn_deg| {
        let cut = drill(
            &[DrainHole {
                shape: HoleShape::Polygon { sides: 4 },
                turn_deg,
                ..hole()
            }],
            &[],
        )
        .expect("a square hole drills");
        cut.aabb().expect("the hole has vertices")
    };
    let (upright, turned) = (square(0.0), square(45.0));
    assert!(
        (upright.maxs.x - 1.0).abs() < 1e-4,
        "unturned, a corner stands on the level line a radius out, got {}",
        upright.maxs.x
    );
    assert!(
        (turned.maxs.x - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4,
        "an eighth of a turn puts a side there instead, got {}",
        turned.maxs.x
    );
}

#[test]
fn a_turn_is_measured_from_the_plates_level_on_a_turned_model() {
    // A model stood up a quarter turn about X: its own -Y is the plate's -Z, and a hole
    // drilled along its own +X runs along the plate's +X, level in both.
    let linear = Mat3::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let axis = Vec3::X;
    let turn = turn_in_model(axis, 0.0, linear);
    let corner = Section::of(HoleShape::Polygon { sides: 6 }, turn)
        .corners(axis)
        .next()
        .expect("a hexagon has corners");
    assert!(
        (linear * corner).z.abs() < 1e-5,
        "unturned, the first corner lies level on the plate, got {}",
        linear * corner
    );
    assert!(
        (turn_in_model(axis, 30.0, Mat3::IDENTITY) - 30.0).abs() < 1e-4,
        "on a model standing as it was made, the turn is the one asked for"
    );
}

#[test]
fn a_hole_is_wound_inward() {
    let cut = drill(&[hole()], &[]).expect("a 2 mm hole drills");
    assert!(
        signed_volume(&cut) < 0.0,
        "a body that subtracts encloses a negative volume; see ADR 0071"
    );
}

#[test]
fn a_tapered_hole_is_a_cone() {
    let cone = drill(
        &[DrainHole {
            taper: 0.0625,
            ..hole()
        }],
        &[],
    )
    .expect("a tapered hole drills");
    let cylinder = drill(&[hole()], &[]).expect("a straight hole drills");
    assert!(
        -signed_volume(&cone) < -signed_volume(&cylinder) / 2.0,
        "a cone of the same mouth holds less than half the cylinder"
    );
}

#[test]
fn a_mouth_on_a_flat_face_still_stands_clear_of_it() {
    let flush = DrainHole {
        lift_mm: 0.0,
        ..hole()
    };
    let cut = drill(&[flush], &[]).expect("a 2 mm hole drills");
    let bounds = cut.aabb().expect("the tube has vertices");

    assert!(
        bounds.maxs.z > 0.0,
        "a cap sitting on the surface would leave a film over the hole, got {}",
        bounds.maxs.z
    );
}

#[test]
fn a_hole_reaches_out_past_the_surface_it_was_placed_on() {
    let cut = drill(&[hole()], &[]).expect("a 2 mm hole drills");
    let bounds = cut.aabb().expect("the tube has vertices");
    assert!(
        bounds.maxs.z >= 1.0,
        "the mouth stands its lift clear of the surface, got {}",
        bounds.maxs.z
    );
}

#[test]
fn a_hole_with_no_width_is_not_a_hole() {
    let flat = DrainHole {
        diameter_mm: 0.0,
        ..hole()
    };
    assert_eq!(
        drill(&[flat], &[]),
        Err(VolumeError::BadDrain { index: 0 }),
        "the caller is told which hole it was"
    );
}

#[test]
fn a_hole_with_no_axis_is_not_a_hole() {
    let nowhere = DrainHole {
        axis: Vec3::ZERO,
        ..hole()
    };
    assert_eq!(
        drill(&[nowhere], &[]),
        Err(VolumeError::BadDrain { index: 0 })
    );
}

#[test]
fn a_channel_of_one_point_digs_nothing() {
    let stub = Channel {
        points: vec![Vec3::ZERO],
        diameter_mm: 2.0,
        ..Channel::default()
    };
    assert_eq!(
        drill(&[], &[stub]),
        Err(VolumeError::BadChannel { index: 0 })
    );
}

#[test]
fn a_bent_channel_is_longer_than_a_straight_one() {
    let straight = Channel {
        points: vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
        diameter_mm: 2.0,
        ..Channel::default()
    };
    let bent = Channel {
        points: vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 8.0, 0.0),
        ],
        diameter_mm: 2.0,
        ..Channel::default()
    };
    let straight = drill(&[], &[straight]).expect("a straight channel digs");
    let bent = drill(&[], &[bent]).expect("a bent channel digs");
    let bounds = bent.aabb().expect("the tunnel has vertices");
    assert!(bent.faces.len() > straight.faces.len());
    assert!(
        bounds.maxs.y >= 8.0,
        "the second leg reaches its own end, got {}",
        bounds.maxs.y
    );
}

#[test]
fn a_channel_drawn_over_a_flat_face_runs_under_it() {
    // Two clicks on the lid of the box, whose outward normal is +Z.
    let clicks = [
        (Vec3::new(2.0, 5.0, 10.0), Vec3::Z),
        (Vec3::new(8.0, 5.0, 10.0), Vec3::Z),
    ];
    let dug = channel_under(&clicks, 3.0).expect("two clicks are a channel");

    assert_eq!(dug.points.len(), 4, "a mouth at each end over two dives");
    let (first, last) = (dug.points[0], dug.points[3]);
    assert!(
        first.z > 10.0 && last.z > 10.0,
        "only the ends break out of the surface, got {first} and {last}"
    );
    for under in &dug.points[1..3] {
        assert!(
            under.z <= 10.0 - 1.5,
            "the run is sunk a radius clear of the face it was drawn on, got {under}"
        );
    }
}

#[test]
fn a_channel_needs_two_points_with_a_normal_to_dive_along() {
    let one = [(Vec3::new(2.0, 5.0, 10.0), Vec3::Z)];
    assert!(channel_under(&one, 3.0).is_none());

    let flat = [
        (Vec3::new(2.0, 5.0, 10.0), Vec3::Z),
        (Vec3::new(8.0, 5.0, 10.0), Vec3::ZERO),
    ];
    assert!(
        channel_under(&flat, 3.0).is_none(),
        "a point with no normal cannot be sunk under the surface"
    );
}

#[test]
fn a_hole_is_placed_on_the_surface_nearest_the_point_it_is_asked_for() {
    // A box from the origin to (10, 10, 10), wound outward.
    let mesh = box_mesh();
    let bvh = Bvh::build(&mesh);
    let placed = hole_at(
        &mesh,
        &bvh,
        Vec3::new(5.0, 5.0, 12.0),
        HoleSize::round(2.0, 5.0, 1.0),
    )
    .expect("the box has faces");

    assert!(
        (placed.at.z - 10.0).abs() < 1e-4,
        "the mouth lands on the lid, got {}",
        placed.at
    );
    assert!(
        placed.axis.normalize().z < -0.99,
        "the hole is drilled into the model, got {}",
        placed.axis
    );
    assert!(
        placed.lift_mm < 1e-6,
        "a flat lid rises nowhere, so the mouth would only push the bounds out, got {}",
        placed.lift_mm
    );
}

#[test]
fn a_mouth_in_a_valley_is_lifted_over_the_walls_around_it() {
    // Two triangles meeting in a V along y, the floor of the valley at the origin and
    // its walls climbing at 45 degrees on either side.
    let mesh = Mesh::new(
        vec![
            Vec3::new(0.0, -5.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(-3.0, -5.0, 3.0),
            Vec3::new(-3.0, 5.0, 3.0),
            Vec3::new(3.0, -5.0, 3.0),
            Vec3::new(3.0, 5.0, 3.0),
        ],
        vec![[0, 1, 3], [0, 3, 2], [0, 4, 5], [0, 5, 1]],
    );
    let bvh = Bvh::build(&mesh);
    let lift = lift_for(&mesh, &bvh, Vec3::ZERO, Vec3::NEG_Z, 4.0, 3.0);

    assert!(
        (lift - 2.0).abs() < 1e-4,
        "a 45 degree wall is 2 mm up at the rim of a 4 mm mouth, got {lift}"
    );
}

#[test]
fn a_mouth_is_not_lifted_through_another_part_standing_over_it() {
    // A slab 2 mm thick with a separate block hanging over it from z 5 to 8.
    let mut mesh = boxed(Vec3::ZERO, Vec3::new(10.0, 10.0, 2.0));
    append(
        &mut mesh,
        &boxed(Vec3::new(0.0, 0.0, 5.0), Vec3::new(10.0, 10.0, 8.0)),
    );
    let bvh = Bvh::build(&mesh);
    let lift = lift_for(&mesh, &bvh, Vec3::new(5.0, 5.0, 2.0), Vec3::NEG_Z, 3.0, 3.0);

    assert!(
        lift < 1e-4,
        "the slab's lid is flat under the mouth, so the block over it is not cut, got {lift}"
    );
}

#[test]
fn a_mouth_in_a_fold_is_not_lifted_through_the_lobe_beside_it() {
    // A slab 2 mm thick against a tower rising 18 mm over it, whose wall the rim of a
    // 3 mm mouth 1 mm from its foot reaches into.
    let mut mesh = boxed(Vec3::ZERO, Vec3::new(6.0, 10.0, 2.0));
    append(
        &mut mesh,
        &boxed(Vec3::new(6.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 20.0)),
    );
    let bvh = Bvh::build(&mesh);
    let lift = lift_for(&mesh, &bvh, Vec3::new(5.0, 5.0, 2.0), Vec3::NEG_Z, 3.0, 3.0);

    assert!(
        lift < 1e-4,
        "a hole 3 mm deep does not cut the 18 mm tower over its rim, got {lift}"
    );
}

fn box_mesh() -> Mesh {
    boxed(Vec3::ZERO, Vec3::splat(10.0))
}

fn boxed(mins: Vec3, maxs: Vec3) -> Mesh {
    let vertices = vec![
        Vec3::new(mins.x, mins.y, mins.z),
        Vec3::new(maxs.x, mins.y, mins.z),
        Vec3::new(maxs.x, maxs.y, mins.z),
        Vec3::new(mins.x, maxs.y, mins.z),
        Vec3::new(mins.x, mins.y, maxs.z),
        Vec3::new(maxs.x, mins.y, maxs.z),
        Vec3::new(maxs.x, maxs.y, maxs.z),
        Vec3::new(mins.x, maxs.y, maxs.z),
    ];
    let faces = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    Mesh::new(vertices, faces)
}
