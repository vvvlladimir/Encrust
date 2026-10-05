//! A solid becomes a shell. What has to hold is that the shell encloses the wall the
//! thickness asked for, that the cavity never reaches outside the model, and that what
//! fills it takes its own volume back.

use core_geometry::{Bvh, Mesh, Scalar, Vec3, signed_volume};
use core_volume::{
    Blocker, CUT_WEIGHT, Channel, HollowMode, HollowSettings, InfillPattern, InfillSettings,
    VolumeError, drill, hole_at, hollow, hollow_at_scale, pierce, sleeves,
};

const PI: Scalar = std::f32::consts::PI;

/// A cube of `size`, standing at `base` on Z and nudged off the lattice: a face landing
/// exactly on a plane of lattice points is a separate question from this one.
fn cube(size: Scalar, base: Scalar) -> Mesh {
    let s = size;
    let nudge = Vec3::new(0.017, 0.023, base);
    let corners = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(s, 0.0, 0.0),
        Vec3::new(s, s, 0.0),
        Vec3::new(0.0, s, 0.0),
        Vec3::new(0.0, 0.0, s),
        Vec3::new(s, 0.0, s),
        Vec3::new(s, s, s),
        Vec3::new(0.0, s, s),
    ];
    Mesh::new(
        corners.into_iter().map(|corner| corner + nudge).collect(),
        vec![
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
        ],
    )
}

/// A closed sphere approximation about the origin, wound outward.
fn ball(radius: Scalar, rings: usize, segments: usize) -> Mesh {
    let mut vertices = vec![Vec3::new(0.0, 0.0, radius)];
    for ring in 1..rings {
        let theta = PI * ring as Scalar / rings as Scalar;
        let (sin_theta, cos_theta) = theta.sin_cos();
        for segment in 0..segments {
            let phi = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            let (sin_phi, cos_phi) = phi.sin_cos();
            vertices.push(Vec3::new(
                radius * sin_theta * cos_phi,
                radius * sin_theta * sin_phi,
                radius * cos_theta,
            ));
        }
    }
    let south = vertices.len() as u32;
    vertices.push(Vec3::new(0.0, 0.0, -radius));

    let at = |ring: usize, segment: usize| (1 + (ring - 1) * segments + segment % segments) as u32;
    let mut faces = Vec::new();
    for segment in 0..segments {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            faces.push([
                at(ring, segment),
                at(ring + 1, segment),
                at(ring + 1, segment + 1),
            ]);
            faces.push([
                at(ring, segment),
                at(ring + 1, segment + 1),
                at(ring, segment + 1),
            ]);
        }
    }
    for segment in 0..segments {
        faces.push([at(rings - 1, segment), south, at(rings - 1, segment + 1)]);
    }
    Mesh::new(vertices, faces)
}

fn settings(thickness_mm: Scalar, mode: HollowMode) -> HollowSettings {
    HollowSettings {
        thickness_mm,
        mode,
        precision: 0.5,
        ..HollowSettings::default()
    }
}

/// What the whole hollowed mesh encloses: the outer surface less the cavity and the
/// drains wound the other way, which is exactly what the positive winding rule will fill.
fn enclosed(mesh: &Mesh) -> Scalar {
    signed_volume(mesh)
}

#[test]
fn a_hollowed_ball_is_a_wall_of_the_thickness_it_asked_for() {
    let mesh = ball(10.0, 96, 96);
    let bvh = Bvh::build(&mesh);
    let hollowed =
        hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal)).expect("a closed ball hollows");

    let wall = 4.0 / 3.0 * PI * (10.0 as Scalar).powi(3) - 4.0 / 3.0 * PI * (8.0 as Scalar).powi(3);
    let measured = enclosed(&hollowed.mesh);
    assert!(
        (measured - wall).abs() / wall < 0.03,
        "4/3 pi (10^3 - 8^3) is {wall} mm3, got {measured}"
    );
}

#[test]
fn the_cavity_is_the_ball_the_wall_left_room_for() {
    let mesh = ball(10.0, 96, 96);
    let bvh = Bvh::build(&mesh);
    let hollowed =
        hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal)).expect("a closed ball hollows");

    let cavity = 4.0 / 3.0 * PI * (8.0 as Scalar).powi(3);
    assert!(
        (hollowed.cavity_mm3 - cavity).abs() / cavity < 0.03,
        "4/3 pi 8^3 is {cavity} mm3, got {}",
        hollowed.cavity_mm3
    );
}

#[test]
fn hollowing_leaves_the_outside_of_the_model_alone() {
    let mesh = ball(10.0, 48, 48);
    let bvh = Bvh::build(&mesh);
    let hollowed =
        hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal)).expect("a closed ball hollows");

    assert_eq!(
        &hollowed.mesh.vertices[..mesh.vertices.len()],
        &mesh.vertices[..],
        "the model's own vertices come through untouched"
    );
    assert_eq!(&hollowed.mesh.faces[..mesh.faces.len()], &mesh.faces[..]);
}

#[test]
fn a_wall_thicker_than_the_model_hollows_nothing() {
    let mesh = ball(5.0, 48, 48);
    let bvh = Bvh::build(&mesh);
    let hollowed =
        hollow(&mesh, &bvh, &settings(6.0, HollowMode::Internal)).expect("a closed ball hollows");

    assert!(hollowed.cavity_mm3 < Scalar::EPSILON);
    assert_eq!(hollowed.mesh.faces.len(), mesh.faces.len());
}

#[test]
fn a_blocker_keeps_the_wall_solid() {
    let mesh = ball(10.0, 48, 48);
    let bvh = Bvh::build(&mesh);
    let mut asked = settings(2.0, HollowMode::Internal);
    asked.blockers = vec![Blocker::ball(Vec3::ZERO, 9.0)];

    let hollowed = hollow(&mesh, &bvh, &asked).expect("a closed ball hollows");
    assert!(
        hollowed.cavity_mm3 < Scalar::EPSILON,
        "a blocker wider than the cavity leaves nothing to hollow, got {}",
        hollowed.cavity_mm3
    );
}

#[test]
fn a_blocker_only_fills_in_what_it_covers() {
    let mesh = ball(10.0, 48, 48);
    let bvh = Bvh::build(&mesh);
    let open = hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal))
        .expect("a closed ball hollows")
        .cavity_mm3;

    let mut asked = settings(2.0, HollowMode::Internal);
    asked.blockers = vec![Blocker::ball(Vec3::ZERO, 4.0)];
    let blocked = hollow(&mesh, &bvh, &asked)
        .expect("a closed ball hollows")
        .cavity_mm3;

    let ball_of_four = 4.0 / 3.0 * PI * (4.0 as Scalar).powi(3);
    let kept = open - blocked;
    assert!(
        (kept - ball_of_four).abs() / ball_of_four < 0.05,
        "a 4 mm blocker keeps 4/3 pi 4^3 = {ball_of_four} mm3 of resin, got {kept}"
    );
}

#[test]
fn a_channel_is_kept_out_of_the_cavity_as_a_pipe() {
    let mesh = ball(10.0, 48, 48);
    let bvh = Bvh::build(&mesh);
    let open = hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal))
        .expect("a closed ball hollows")
        .cavity_mm3;

    // Straight through the middle, so the sleeve is a rod the ball's own width.
    let channel = Channel {
        points: vec![Vec3::new(0.0, 0.0, -12.0), Vec3::new(0.0, 0.0, 12.0)],
        diameter_mm: 2.0,
    };
    let mut asked = settings(2.0, HollowMode::Internal);
    asked.blockers = sleeves(&[channel], asked.thickness_mm);
    let piped = hollow(&mesh, &bvh, &asked)
        .expect("a closed ball hollows")
        .cavity_mm3;

    // A rod of radius 1 + 2 mm through a cavity of radius 8, so pi r^2 times its chord.
    let rod = PI * 9.0 * 2.0 * ((8.0 * 8.0 - 9.0) as Scalar).sqrt();
    let kept = open - piped;
    assert!(
        (kept - rod).abs() / rod < 0.1,
        "the wall around the tube keeps {rod} mm3 of resin, got {kept}"
    );
}

#[test]
fn infill_takes_its_own_volume_back_out_of_the_cavity() {
    let mesh = ball(12.0, 64, 64);
    let bvh = Bvh::build(&mesh);
    let empty =
        hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal)).expect("a closed ball hollows");

    for pattern in InfillPattern::ALL {
        let mut asked = settings(2.0, HollowMode::Internal);
        asked.infill = Some(InfillSettings {
            pattern,
            size_mm: 6.0,
            density: 0.2,
        });
        let filled = hollow(&mesh, &bvh, &asked).expect("a closed ball hollows");

        assert!(
            filled.cavity_mm3 < empty.cavity_mm3,
            "{} must stand in the cavity: {} mm3 against {} mm3 empty",
            pattern.label(),
            filled.cavity_mm3,
            empty.cavity_mm3
        );
        assert!(
            enclosed(&filled.mesh) > enclosed(&empty.mesh),
            "{} must add material to the shell",
            pattern.label()
        );
    }
}

#[test]
fn a_stretched_model_keeps_the_wall_it_asked_for_on_the_plate() {
    let mesh = cube(20.0, 0.0);
    let bvh = Bvh::build(&mesh);
    let stretch = Vec3::new(2.0, 1.0, 1.0);
    let hollowed = hollow_at_scale(&mesh, &bvh, &settings(2.0, HollowMode::Internal), stretch)
        .expect("a closed cube hollows");

    let cavity = (40.0 - 4.0) * (20.0 - 4.0) * (20.0 - 4.0);
    assert!(
        (hollowed.cavity_mm3 - cavity).abs() / cavity < 0.05,
        "a 40 x 20 x 20 mm box with a 2 mm wall holds {cavity} mm3, got {}",
        hollowed.cavity_mm3
    );
}

#[test]
fn a_stretched_model_is_filled_as_if_it_had_been_built_that_size() {
    let mesh = cube(20.0, 0.0);
    let bvh = Bvh::build(&mesh);
    let stretch = Vec3::new(1.6, 1.0, -1.0);
    let mut asked = settings(2.0, HollowMode::Internal);
    asked.infill = Some(InfillSettings {
        pattern: InfillPattern::Hive,
        size_mm: 5.0,
        density: 0.15,
    });
    asked.blockers = vec![Blocker::ball(Vec3::new(10.0, 10.0, 10.0), 3.0)];
    let scaled = hollow_at_scale(&mesh, &bvh, &asked, stretch).expect("a closed cube hollows");

    let size = stretch.abs();
    let placed = Mesh::new(
        mesh.vertices.iter().map(|vertex| *vertex * size).collect(),
        mesh.faces.clone(),
    );
    asked.blockers = vec![Blocker::ball(Vec3::new(16.0, 10.0, 10.0), 3.0)];
    let built = hollow(&placed, &Bvh::build(&placed), &asked).expect("a closed box hollows");

    assert_eq!(
        scaled.mesh.faces, built.mesh.faces,
        "the same cells, the same struts and the same blocker as on the box itself"
    );
    assert!(
        scaled
            .mesh
            .vertices
            .iter()
            .zip(&built.mesh.vertices)
            .all(|(scaled, built)| (*scaled * size).abs_diff_eq(*built, 1e-3)),
        "the shell comes back in the model's own space, to be scaled onto the plate"
    );
}

#[test]
fn a_model_flattened_to_nothing_cannot_be_hollowed() {
    let mesh = cube(20.0, 0.0);
    let bvh = Bvh::build(&mesh);
    let flat = Vec3::new(1.0, 0.0, 1.0);
    assert_eq!(
        hollow_at_scale(&mesh, &bvh, &settings(2.0, HollowMode::Internal), flat).unwrap_err(),
        VolumeError::BadScale(flat)
    );
}

/// A box standing on the floor of the model and up through its wall, the way a stray part
/// left inside a sculpt does. A field over it read its outside as air and cut a wall
/// around it, open where it met the model's own.
#[test]
fn a_shell_inside_the_model_gets_no_wall_of_its_own() {
    let outer = cube(20.0, 0.0);
    let inner = cube(4.0, 0.0);
    let mut mesh = outer.clone();
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend(
        inner
            .vertices
            .iter()
            .map(|vertex| *vertex + Vec3::new(8.0, 8.0, 0.0)),
    );
    mesh.faces.extend(
        inner
            .faces
            .iter()
            .map(|face| face.map(|corner| corner + offset)),
    );
    let bvh = Bvh::build(&mesh);

    let hollowed =
        hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal)).expect("a closed cube hollows");

    let cavity = 16.0 * 16.0 * 16.0;
    assert!(
        (hollowed.cavity_mm3 - cavity).abs() / cavity < 0.03,
        "the cavity of a 20 mm cube with a 2 mm wall is {cavity} mm3, with no wall cut around \
         the box inside it; got {}",
        hollowed.cavity_mm3
    );
    assert_eq!(
        &hollowed.mesh.faces[..mesh.faces.len()],
        &mesh.faces[..],
        "the box inside is still in the model, and still prints solid"
    );
}

#[test]
fn a_mould_grows_a_wall_outside_the_model() {
    let mesh = ball(8.0, 64, 64);
    let bvh = Bvh::build(&mesh);
    let hollowed =
        hollow(&mesh, &bvh, &settings(2.0, HollowMode::External)).expect("a closed ball hollows");

    let wall = 4.0 / 3.0 * PI * ((10.0 as Scalar).powi(3) - (8.0 as Scalar).powi(3));
    let measured = enclosed(&hollowed.mesh);
    assert!(
        (measured - wall).abs() / wall < 0.03,
        "4/3 pi (10^3 - 8^3) is {wall} mm3, got {measured}"
    );

    let bounds = hollowed.mesh.aabb().expect("the mould has vertices");
    assert!(
        bounds.maxs.x > 9.5,
        "the mould reaches past the model it was grown from, got {}",
        bounds.maxs.x
    );
}

#[test]
fn a_wall_of_no_thickness_is_refused() {
    let mesh = ball(5.0, 16, 16);
    let bvh = Bvh::build(&mesh);
    assert_eq!(
        hollow(&mesh, &bvh, &settings(0.0, HollowMode::Internal)).unwrap_err(),
        VolumeError::BadThickness(0.0)
    );
}

#[test]
fn a_mesh_with_no_faces_has_nothing_to_hollow() {
    let mesh = Mesh::default();
    let bvh = Bvh::build(&mesh);
    assert_eq!(
        hollow(&mesh, &bvh, &settings(1.0, HollowMode::Internal)).unwrap_err(),
        VolumeError::EmptyMesh
    );
}

#[test]
fn an_infill_cell_of_no_size_is_refused() {
    let mesh = ball(10.0, 32, 32);
    let bvh = Bvh::build(&mesh);
    let mut asked = settings(2.0, HollowMode::Internal);
    asked.infill = Some(InfillSettings {
        pattern: InfillPattern::Hive,
        size_mm: 0.0,
        density: 0.2,
    });
    assert_eq!(
        hollow(&mesh, &bvh, &asked).unwrap_err(),
        VolumeError::BadCell(0.0)
    );
}

#[test]
fn a_budget_too_small_for_the_lattice_coarsens_the_cavity_rather_than_failing() {
    let mesh = ball(20.0, 96, 96);
    let bvh = Bvh::build(&mesh);
    let mut asked = settings(2.0, HollowMode::Internal);
    asked.precision = 1.0;
    // Four megabytes is a few thousand tiles: far less than a 20 mm ball at the finest
    // lattice asks for, and enough for a coarse one.
    asked.budget_bytes = 4 << 20;

    let hollowed =
        hollow(&mesh, &bvh, &asked).expect("a budget is met by coarsening, not by failing");
    assert!(
        hollowed.coarsened,
        "the run had to give up the lattice precision asked for"
    );
    assert!(
        hollowed.voxel_mm > asked.voxel_mm(40.0),
        "the lattice it settled on, {} mm, is no coarser than the {} mm asked for",
        hollowed.voxel_mm,
        asked.voxel_mm(40.0)
    );
    assert!(
        hollowed.cavity_mm3 > 0.0,
        "a coarsened run still hollows the model"
    );
}

#[test]
fn a_lattice_is_bounded_by_the_surface_it_pays_for() {
    // A field's cost is the surface over the square of the spacing, so sixteen times the
    // surface asks for four times the spacing. Both are past the point where the finest
    // spacing would have won.
    let small = core_volume::lattice_mm(2.0, 1.0, 1.0e5);
    let large = core_volume::lattice_mm(2.0, 1.0, 16.0e5);
    assert!(
        (large / small - 4.0).abs() < 0.01,
        "sixteen times the surface should ask for four times the spacing, \
         got {small} mm and {large} mm"
    );
    assert!(
        core_volume::lattice_mm(2.0, 1.0, 1.0e3) < small,
        "a small surface is still cut on the fine lattice precision asks for"
    );
}

/// A spire and a block of the same height are not the same surface, and the ceiling is
/// what says so: under the old one, both asked for the same spacing.
#[test]
fn a_slender_model_is_not_capped_like_a_bulky_one_of_its_height() {
    // 150 mm tall: a 5 mm rod against a cube of the same height.
    let rod = std::f32::consts::PI * 5.0 * 150.0;
    let block = 6.0 * 150.0 * 150.0;
    assert!(
        core_volume::lattice_mm(2.0, 1.0, rod) < core_volume::lattice_mm(2.0, 1.0, block),
        "the rod has a fortieth of the block's surface and should be cut finer"
    );
}

#[test]
fn the_same_model_hollows_to_the_same_mesh_twice() {
    // Tiles come out of a hash map, so extraction has to put them in an order of its own
    // before merging them; without that the vertices of a cavity come out numbered
    // differently on every run, and a mesh with a hole in it then slices differently too.
    let mesh = ball(10.0, 48, 48);
    let bvh = Bvh::build(&mesh);
    let asked = settings(2.0, HollowMode::Internal);

    let one = hollow(&mesh, &bvh, &asked).expect("a closed ball hollows");
    let other = hollow(&mesh, &bvh, &asked).expect("a closed ball hollows");

    assert_eq!(one.mesh.vertices, other.mesh.vertices);
    assert_eq!(one.mesh.faces, other.mesh.faces);
}

#[test]
fn a_drain_hole_takes_its_own_tube_out_of_the_shell() {
    let mesh = ball(10.0, 96, 96);
    let bvh = Bvh::build(&mesh);
    let solid =
        hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal)).expect("a closed ball hollows");

    let hole =
        hole_at(&mesh, &bvh, Vec3::new(0.0, 0.0, 12.0), 3.0, 4.0, 1.0).expect("the ball has faces");
    let mut drained = solid.mesh.clone();
    append(
        &mut drained,
        &drill(&[hole], &[]).expect("a 3 mm hole drills"),
    );

    // A ball curves away under the mouth, so the tube starts on the surface and runs four
    // millimetres in. What the rule fills is only the part of it inside the wall, but what
    // is appended, and so what the signed volume counts, is the whole tube.
    let tube = PI * 1.5 * 1.5 * 4.0;
    let taken = (enclosed(&solid.mesh) - enclosed(&drained)) / CUT_WEIGHT as Scalar;
    assert!(
        (taken - tube).abs() / tube < 0.05,
        "a 3 mm by 4 mm tube is {tube} mm3, got {taken}"
    );
}

#[test]
fn a_hole_shallower_than_the_wall_is_deepened_until_it_is_through_it() {
    let mesh = ball(10.0, 96, 96);
    let bvh = Bvh::build(&mesh);
    let shell =
        hollow(&mesh, &bvh, &settings(2.0, HollowMode::Internal)).expect("a closed ball hollows");

    // Half a millimetre into a two millimetre wall: a dimple, not a drain.
    let shallow =
        hole_at(&mesh, &bvh, Vec3::new(0.0, 0.0, 12.0), 3.0, 0.5, 1.0).expect("the ball has faces");
    let through = pierce(&[shallow], 2.0, shell.voxel_mm);
    let mut drained = shell.mesh.clone();
    append(
        &mut drained,
        &drill(&through, &[]).expect("a 3 mm hole drills"),
    );

    let wall = PI * 1.5 * 1.5 * 2.0;
    let taken = (enclosed(&shell.mesh) - enclosed(&drained)) / CUT_WEIGHT as Scalar;
    assert!(
        taken > wall,
        "the hole has to reach past the {wall} mm3 of wall in front of it, got {taken}"
    );
}

/// The bodies a cut is made of are appended to whatever they cut; see ADR 0075.
fn append(whole: &mut Mesh, part: &Mesh) {
    let offset = whole.vertices.len() as u32;
    whole.vertices.extend_from_slice(&part.vertices);
    whole.faces.extend(
        part.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}
