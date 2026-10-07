//! A mesh becomes a field and comes back a mesh. What has to hold is that the body it
//! comes back as is the body that went in, to within the lattice it was sampled on.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Bvh, Mesh, Scalar, Vec3, diagnose, glam::IVec3, signed_volume, weld};
use core_volume::{
    Cancel, FieldSettings, SignMode, VolumeError, build, difference, extract, shell,
};

const PI: Scalar = std::f32::consts::PI;

/// A cube of `size`, standing on the origin and nudged off the lattice: a face landing
/// exactly on a plane of lattice points is a separate question from this one.
fn cube(size: Scalar) -> Mesh {
    let s = size;
    let nudge = Vec3::new(0.017, 0.023, 0.031);
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

fn field_of(mesh: &Mesh, settings: &FieldSettings) -> core_volume::Sdf {
    let bvh = Bvh::build(mesh);
    build(mesh, &bvh, settings, Cancel::never()).expect("the mesh has faces")
}

#[test]
fn a_ball_field_reads_the_distance_to_its_own_surface() {
    let radius = 10.0;
    let mesh = ball(radius, 64, 64);
    let settings = FieldSettings {
        voxel_mm: 0.2,
        ..FieldSettings::default()
    };
    let field = field_of(&mesh, &settings);

    // Trilinear interpolation of a curved surface is a chord of it, so the field can sit
    // half a voxel out, and the mesh itself is inscribed by another 12 micrometres.
    let slack = settings.voxel_mm / 2.0 + radius * (1.0 - (PI / 128.0).cos() * (PI / 64.0).cos());

    for point in [
        Vec3::new(10.25, 0.0, 0.0),
        Vec3::new(0.0, -9.8, 0.0),
        Vec3::new(0.0, 0.0, 10.0),
        Vec3::new(5.7, 5.7, 5.7),
    ] {
        let exact = point.length() - radius;
        let found = field.sample(point);
        assert!(
            (found - exact).abs() <= slack,
            "at {point}: |p| - r is {exact}, the field says {found}"
        );
    }
}

#[test]
fn a_ball_survives_the_round_trip_within_a_voxel() {
    let radius = 10.0;
    let mesh = ball(radius, 96, 96);
    let settings = FieldSettings {
        voxel_mm: 0.25,
        ..FieldSettings::default()
    };
    let extracted = extract(&field_of(&mesh, &settings), Cancel::never());

    let diagnostics = diagnose(&extracted);
    assert!(
        diagnostics.is_closed(),
        "the extracted surface is open: {diagnostics:?}"
    );
    assert_eq!(diagnostics.shells, 1);

    // Marching cubes places the surface within half a voxel of where the field says it is,
    // so the volume can be off by the surface area times that.
    let exact = 4.0 / 3.0 * PI * radius.powi(3);
    let slack = 4.0 * PI * radius.powi(2) * settings.voxel_mm / 2.0;
    let found = signed_volume(&extracted);
    assert!(
        (found - exact).abs() <= slack,
        "a ball of {exact} mm3 came back as {found} mm3, past the {slack} mm3 a voxel allows"
    );
}

#[test]
fn a_cube_survives_the_round_trip_within_a_voxel() {
    let side = 8.0;
    let mesh = cube(side);
    let settings = FieldSettings {
        voxel_mm: 0.2,
        ..FieldSettings::default()
    };
    let extracted = extract(&field_of(&mesh, &settings), Cancel::never());

    assert!(diagnose(&extracted).is_closed());
    let exact = side.powi(3);
    let slack = 6.0 * side.powi(2) * settings.voxel_mm / 2.0;
    let found = signed_volume(&extracted);
    assert!(
        (found - exact).abs() <= slack,
        "a cube of {exact} mm3 came back as {found} mm3"
    );
}

#[test]
fn the_winding_number_signs_the_same_field_as_the_pseudonormal() {
    let mesh = ball(6.0, 48, 48);
    let coarse = FieldSettings {
        voxel_mm: 0.5,
        ..FieldSettings::default()
    };
    let by_winding = FieldSettings {
        sign: SignMode::Winding,
        ..coarse
    };

    let one = signed_volume(&extract(&field_of(&mesh, &coarse), Cancel::never()));
    let other = signed_volume(&extract(&field_of(&mesh, &by_winding), Cancel::never()));
    assert!(
        (one - other).abs() < 1.0,
        "the two signs disagree: {one} mm3 against {other} mm3"
    );
}

#[test]
fn shelling_a_cube_hollows_it_and_leaves_the_wall_it_was_asked_for() {
    let side = 8.0;
    let wall = 0.6;
    let settings = FieldSettings {
        voxel_mm: 0.2,
        band_voxels: 6.0,
        ..FieldSettings::default()
    };
    let hollow = shell(&field_of(&cube(side), &settings), wall).expect("the wall fits the band");

    let middle = Vec3::splat(side / 2.0);
    assert!(
        hollow.sample(middle) > 0.0,
        "the middle of a shelled cube is no longer solid"
    );
    // Half a wall in from the outside is the middle of the wall itself.
    let in_the_wall = Vec3::new(wall / 2.0, side / 2.0, side / 2.0);
    assert!(
        hollow.sample(in_the_wall) < 0.0,
        "the wall itself has gone hollow at {in_the_wall}"
    );

    let extracted = extract(&hollow, Cancel::never());
    assert_eq!(
        diagnose(&extracted).shells,
        2,
        "a shell is the outer surface and the inner one"
    );

    let exact = side.powi(3) - (side - 2.0 * wall).powi(3);
    let slack = 12.0 * side.powi(2) * settings.voxel_mm / 2.0;
    let found = signed_volume(&extracted);
    assert!(
        (found - exact).abs() <= slack,
        "a wall of {exact} mm3 came back as {found} mm3"
    );
}

#[test]
fn a_ball_taken_out_of_a_bigger_one_leaves_a_shell() {
    let settings = FieldSettings {
        voxel_mm: 0.25,
        ..FieldSettings::default()
    };
    let outer = field_of(&ball(6.0, 64, 64), &settings);
    let inner = field_of(&ball(3.0, 64, 64), &settings);

    let hollow = difference(&outer, &inner).expect("both fields share the lattice");
    let extracted = extract(&hollow, Cancel::never());

    assert_eq!(diagnose(&extracted).shells, 2);
    let exact = 4.0 / 3.0 * PI * (6.0f32.powi(3) - 3.0f32.powi(3));
    let slack = 4.0 * PI * (6.0f32.powi(2) + 3.0f32.powi(2)) * settings.voxel_mm / 2.0;
    let found = signed_volume(&extracted);
    assert!(
        (found - exact).abs() <= slack,
        "a shell of {exact} mm3 came back as {found} mm3"
    );
}

#[test]
fn a_mesh_with_no_faces_has_no_field() {
    let bvh = Bvh::build(&Mesh::default());
    assert_eq!(
        build(
            &Mesh::default(),
            &bvh,
            &FieldSettings::default(),
            Cancel::never()
        )
        .unwrap_err(),
        VolumeError::EmptyMesh
    );
}

#[test]
fn a_voxel_with_no_size_is_refused() {
    let mesh = cube(4.0);
    let bvh = Bvh::build(&mesh);
    let settings = FieldSettings {
        voxel_mm: 0.0,
        ..FieldSettings::default()
    };
    assert_eq!(
        build(&mesh, &bvh, &settings, Cancel::never()).unwrap_err(),
        VolumeError::BadVoxelSize(0.0)
    );
}

#[test]
fn a_band_narrower_than_a_voxel_is_refused() {
    let mesh = cube(4.0);
    let bvh = Bvh::build(&mesh);
    let settings = FieldSettings {
        band_voxels: 0.5,
        ..FieldSettings::default()
    };
    assert_eq!(
        build(&mesh, &bvh, &settings, Cancel::never()).unwrap_err(),
        VolumeError::BadBand(0.5)
    );
}

#[test]
fn moving_the_surface_further_than_the_band_is_refused() {
    let settings = FieldSettings {
        voxel_mm: 0.4,
        ..FieldSettings::default()
    };
    let field = field_of(&cube(4.0), &settings);
    assert!(matches!(
        shell(&field, 2.0).unwrap_err(),
        VolumeError::OutsideBand { asked_mm, .. } if (asked_mm - 2.0).abs() < 1e-6
    ));
}

#[test]
fn two_fields_on_different_lattices_cannot_be_combined() {
    let mesh = cube(4.0);
    let coarse = field_of(
        &mesh,
        &FieldSettings {
            voxel_mm: 0.4,
            ..FieldSettings::default()
        },
    );
    let fine = field_of(
        &mesh,
        &FieldSettings {
            voxel_mm: 0.2,
            ..FieldSettings::default()
        },
    );
    assert!(matches!(
        difference(&coarse, &fine),
        Err(VolumeError::GridMismatch { .. })
    ));
}

#[test]
fn a_surface_crossing_tiles_shares_its_seam_vertices() {
    // A ball wide enough to cross many tiles: every vertex on a seam is written once, so
    // a mesh that came out of several tiles has no coincident pair left in it.
    let mesh = ball(6.0, 64, 64);
    let settings = FieldSettings {
        voxel_mm: 0.2,
        ..FieldSettings::default()
    };
    let extracted = extract(&field_of(&mesh, &settings), Cancel::never());

    let welded = weld(&extracted, settings.voxel_mm * 1e-3);
    assert_eq!(
        welded.vertices_merged, 0,
        "the extracted surface still has {} vertices a weld would merge",
        welded.vertices_merged
    );
    assert!(diagnose(&extracted).is_closed());
}

/// The swept field answers what a full search of the hierarchy would, everywhere the band
/// reaches, including a band a whole wall thickness off the mesh.
///
/// The tolerance is half a voxel: that is what `docs/decisions/0063` already allows the
/// wall, and it is the bound the carrier is chosen against. The comparison is against the
/// hierarchy and not against the sphere, so that the mesh's own faceting is not counted
/// as the sweep's error.
#[test]
fn the_sweep_answers_what_the_hierarchy_would() {
    let radius = 6.0;
    let mesh = ball(radius, 64, 64);
    let bvh = Bvh::build(&mesh);

    for iso_mm in [0.0, -2.0] {
        let settings = FieldSettings {
            voxel_mm: 0.2,
            iso_mm,
            ..FieldSettings::default()
        };
        let field = build(&mesh, &bvh, &settings, Cancel::never()).expect("the ball has faces");
        let grid = field.grid();
        let band_mm = field.band_mm();

        let mut worst = 0.0;
        let mut at = IVec3::ZERO;
        // Every other voxel: 46k samples say the same as 357k and cost an eighth.
        for z in (-35..=35).step_by(2) {
            for y in (-35..=35).step_by(2) {
                for x in (-35..=35).step_by(2) {
                    let voxel = IVec3::new(x, y, z);
                    let point = grid.position(voxel);
                    let found = bvh.closest(&mesh, point).expect("the ball has faces");
                    // A ball is the one body where the side needs no pseudonormal.
                    let side = if point.length() < radius { -1.0 } else { 1.0 };
                    let truth = side * found.distance - iso_mm;
                    if truth.abs() > band_mm * 0.9 {
                        continue;
                    }
                    let error = (field.value(voxel) - truth).abs();
                    if error > worst {
                        worst = error;
                        at = voxel;
                    }
                }
            }
        }

        assert!(
            worst <= settings.voxel_mm / 2.0,
            "at {at} the swept field is {worst} mm off what the hierarchy answers, \
             on a {} mm lattice around the isosurface at {iso_mm} mm",
            settings.voxel_mm
        );
    }
}

/// A cavity is not one triangle per voxel face.
///
/// Marching cubes gave about three triangles for every cell its surface crossed, whatever
/// the surface did; clustering onto the lattice gives a flat stretch one facet, so what
/// comes out follows the shape and not the spacing. The count has to stay well under the
/// cells crossed, and the surface has to still be closed — a cavity is sliced, and an open
/// contour is a print.
#[test]
fn a_smooth_cavity_costs_far_fewer_triangles_than_the_cells_it_crosses() {
    let radius = 10.0;
    let mesh = ball(radius, 96, 96);
    let bvh = Bvh::build(&mesh);
    let settings = FieldSettings {
        voxel_mm: 0.2,
        iso_mm: -2.0,
        ..FieldSettings::default()
    };
    let extracted = extract(
        &build(&mesh, &bvh, &settings, Cancel::never()).expect("the ball has faces"),
        Cancel::never(),
    );

    let diagnostics = diagnose(&extracted);
    assert!(
        diagnostics.is_closed(),
        "the clustered cavity is open: {diagnostics:?}"
    );

    // The cavity is the sphere of radius 8; its surface crosses about this many cells.
    let crossed = 4.0 * PI * 64.0 / (settings.voxel_mm * settings.voxel_mm);
    assert!(
        (extracted.faces.len() as Scalar) < crossed / 4.0,
        "a cavity crossing {crossed:.0} cells came out as {} triangles, which is no better \
         than a triangle a cell",
        extracted.faces.len()
    );
}
