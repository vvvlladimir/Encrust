//! Supports: where a column can stand under a model, and the mesh it is printed as.
//!
//! A support is geometry, not a mark on a layer. It is meshed here and merged into the
//! model before slicing, so nothing downstream needs to know it exists; see
//! `docs/architecture.md` and `docs/design/supports.md`.
//!
//! Where the supports go is worked out on the stack of layers rather than on the mesh's
//! faces: `generate_supports` reads a `Sliced` model and answers with the contacts that
//! need a column under them. `grow` then merges the tips of nearby columns into shared
//! trunks, so what gets meshed is a forest of `SupportTree`s rather than a set of
//! separate pillars; see `docs/decisions/0040`.

mod base;
mod branch;
mod clear;
mod column;
mod config;
mod detect;
mod edit;
mod field;
mod generate;
mod group;
mod mesher;
mod model;
mod pick;
mod placed;
mod project;
mod region;
mod sample;
mod trapped;
mod tree;

pub use branch::grow;
pub use column::{Column, Landing, SupportPoint, columns, landing, neck_of};
pub use edit::{carried, fits, on_model};
pub use generate::generate_supports;
pub use group::Profiles;
pub use mesher::{FOOT_BEVEL, mesh_groups, mesh_trees};
pub use model::ModelSupports;
pub use pick::{Grab, Part, grab, support_under};
pub use placed::Placed;
pub use project::{ProjectSettings, project};
pub use region::{Blocked, Region};
pub use trapped::{TrapScan, Trapped};
pub use tree::{SupportTree, TreeNode};

#[cfg(test)]
mod tests {
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
        crate::landing(
            &placed,
            crate::neck_of(&placed, contact, profile),
            profile.pillar_radius_mm(),
            profile,
        )
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

    /// The profile every test measures against: a fixture of its own, not a shipped
    /// preset, so that retuning what Encrust ships does not move what the tests pin down.
    /// Its numbers are round, its branches lean 45 degrees and nothing is braced.
    pub fn profile() -> SupportProfile {
        use printer_profiles::{
            Bracing, Branching, ContactShape, MiddleSegment, PlatformShape, Raft, SmallPillar,
            TipSegment, TopSegment,
        };
        SupportProfile {
            name: "Fixture".to_owned(),
            facets: 12,
            density: 1.0,
            max_overhang_deg: 45.0,
            contact_spacing_mm: 4.0,
            clearance_mm: 0.5,
            tip: TipSegment {
                shape: ContactShape::Cone,
                contact_diameter_mm: 0.4,
                contact_depth_mm: 0.2,
            },
            top: TopSegment {
                upper_diameter_mm: 0.4,
                lower_diameter_mm: 1.2,
                length_mm: 2.0,
            },
            middle: MiddleSegment { diameter_mm: 1.2 },
            bottom: printer_profiles::BottomSegment {
                shape: PlatformShape::Cylinder,
                platform_diameter_mm: 8.0,
                platform_thickness_mm: 1.0,
                upper_diameter_mm: 1.2,
                lower_diameter_mm: 2.2,
            },
            land_on_model: true,
            small_pillar: SmallPillar::default(),
            branching: Branching::default(),
            raft: Raft::default(),
            bracing: Bracing::default(),
            z_lift_mm: 5.0,
        }
    }

    /// An axis-aligned box spanning `min`..`max`, twelve triangles wound outwards.
    pub fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
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

    /// A triangular prism `run` wide and `rise` tall, its sloping face looking up and
    /// back along -x, wound outwards. The steepness of that face is what a landing is
    /// measured against.
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
}
