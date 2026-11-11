//! Bodies and profiles the unit tests of this crate share.

use crate::{Column, Landing, Placed, Profiles, SupportPoint};
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};
use printer_profiles::SupportProfile;

/// Drops a column against a mesh, building the hierarchy that every caller outside
/// the tests keeps beside its mesh.
pub fn landing_on(
    model: &Mesh,
    transform: Transform,
    contact: Vec3,
    profile: &SupportProfile,
) -> Option<Landing> {
    let bvh = Bvh::build(model);
    let placed = Placed::new(model, &bvh, transform);
    let neck = crate::neck_of(&placed, contact, profile)?;
    crate::landing(&placed, neck, profile.pillar_radius_mm(), 1, profile)
}

/// The same for a whole set of points.
pub fn columns_on(
    points: &[SupportPoint],
    model: &Mesh,
    transform: Transform,
    profiles: Profiles,
) -> Vec<Column> {
    crate::columns(
        points,
        &Placed::new(model, &Bvh::build(model), transform),
        profiles,
    )
}

/// The forest a set of points grows into, building the hierarchy every caller
/// outside the tests keeps beside its mesh.
pub fn trees_on(
    points: &[SupportPoint],
    model: &Mesh,
    transform: Transform,
    profiles: Profiles,
) -> Vec<crate::SupportTree> {
    let bvh = Bvh::build(model);
    let placed = Placed::new(model, &bvh, transform);
    let columns = crate::columns(points, &placed, profiles);
    crate::grow(&columns, &placed, profiles)
}

/// Meshes a forest standing in clear air, which is what every test that is not
/// about keeping out of the model asks for.
pub fn meshed(trees: &[crate::SupportTree], profiles: Profiles) -> Mesh {
    let model = Mesh::default();
    crate::mesh_trees(
        trees,
        &Placed::new(&model, &Bvh::build(&model), Transform::default()),
        profiles,
    )
}

/// The profile every test measures against: its own, not a shipped preset, so retuning presets
/// moves no test. Round numbers, 45 degree branches, nothing braced.
pub fn profile() -> SupportProfile {
    use printer_profiles::{
        Bracing, Branching, ContactShape, MiddleSegment, PlatformShape, Raft, SmallPillar,
        TipSegment, TopSegment,
    };
    SupportProfile {
        grid: printer_profiles::Grid::default(),
        islands: printer_profiles::IslandSpacing::default(),
        name: "Fixture".to_owned(),
        facets: 12,
        density: 1.0,
        max_overhang_deg: 45.0,
        contact_spacing_mm: 4.0,
        steep_contact_spacing_mm: 4.0,
        clearance_mm: 0.5,
        tip: TipSegment {
            shape: ContactShape::Plane,
            contact_diameter_mm: 0.4,
            contact_depth_mm: 0.2,
        },
        top: TopSegment { length_mm: 2.0 },
        middle: MiddleSegment { diameter_mm: 1.2 },
        bottom: printer_profiles::BottomSegment {
            shape: PlatformShape::Cylinder,
            platform_diameter_mm: 8.0,
            platform_thickness_mm: 1.0,
            lower_diameter_mm: 2.2,
        },
        land_on_model: true,
        small_pillar: SmallPillar::default(),
        // Struts as long as the merge distance lets them run, so a test of how tips
        // merge is not one of the limit on a strut.
        branching: Branching {
            max_strut_length_mm: 10.0,
            ..Branching::default()
        },
        raft: Raft::default(),
        bracing: Bracing::default(),
        z_lift_mm: 5.0,
    }
}

/// An axis-aligned box spanning `min`..`max`, twelve triangles wound outwards, used as a slab with
/// its underside at z = 10.5 half a millimetre over a bar 9..10 thick hanging in the air.
pub fn under_a_gap_over_a_bar() -> Mesh {
    let mut model = box_mesh(Vec3::new(-5.0, -5.0, 9.0), Vec3::new(5.0, 5.0, 10.0));
    let over = box_mesh(Vec3::new(-5.0, -5.0, 10.5), Vec3::new(5.0, 5.0, 12.0));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&over.vertices);
    model
        .faces
        .extend(over.faces.iter().map(|face| face.map(|at| at + offset)));
    model
}

pub fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
    core_geometry::cuboid(min, max)
}

/// A triangular prism `run` wide and `rise` tall, its sloping face looking up and back along -x,
/// wound outwards; that face's steepness is what a landing is measured against.
pub fn ramp(run: Scalar, rise: Scalar) -> Mesh {
    let vertices = vec![
        Vec3::new(0.0, -5.0, 0.0),
        Vec3::new(run, -5.0, 0.0),
        Vec3::new(run, -5.0, rise),
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::new(run, 5.0, 0.0),
        Vec3::new(run, 5.0, rise),
    ];
    let faces = vec![
        [0, 1, 2],
        [3, 5, 4],
        [0, 3, 4],
        [0, 4, 1],
        [1, 4, 5],
        [1, 5, 2],
        [0, 2, 5],
        [0, 5, 3],
    ];
    Mesh::new(vertices, faces)
}

#[test]
fn the_test_ramp_is_closed_and_wound_outwards() {
    let mesh = ramp(4.0, 10.0);
    assert_eq!(core_geometry::diagnose(&mesh).boundary_edges, 0);
    assert!(
        (core_geometry::signed_volume(&mesh) - 200.0).abs() < 1e-4,
        "a 4 by 10 right triangle 10 mm deep holds 200 mm3"
    );
}

#[test]
fn the_test_box_is_closed_and_wound_outwards() {
    let mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
    assert_eq!(core_geometry::diagnose(&mesh).boundary_edges, 0);
    assert!(
        (core_geometry::signed_volume(&mesh) - 8.0).abs() < 1e-5,
        "a 2 mm cube holds 8 mm3"
    );
}
