//! Types are plain data, operations free functions; `glam` vectors, see ADR 0003.

mod buckets;
mod bvh;
mod clip;
mod closest;
mod hash;
mod hull;
mod mesh;
mod orient;
mod primitives;
mod ray;
mod region;
mod repair;
mod ring;
mod smooth;
mod split;
mod topology;
mod transform;
mod triangle;
mod triangulate;
mod uv;
mod validate;
mod weld;
mod winding;

pub use buckets::Buckets;
pub use bvh::Bvh;
pub use clip::{Cut, Plane, axes_across, cut};
pub use closest::{ClosestPoint, closest_point, point_aabb_squared, point_triangle};
pub use hash::{FastHasher, FastMap, FastSet};
pub use hull::{convex_hull, fitting_turn};
pub use mesh::Mesh;
pub use orient::{Orientation, orient_outward};
pub use primitives::{
    ball, cuboid, cylinder, extrude, oriented_box, prism, revolve, segments_for, turns,
};
pub use ray::{
    PlacedHit, Ray, RayHit, ray_aabb, ray_triangle, raycast, raycast_placed, raycast_placed_within,
};
pub use region::Region;
pub use repair::{Filled, collapse_flat_faces, fill_holes, remove_duplicate_faces};
pub use ring::{ring_area, ring_contains};
pub use smooth::{SmoothNormals, smooth_normals};
pub use split::{split, split_at_edges};
pub use topology::{Adjacency, Components};
pub use transform::{
    Transform, center_over_plate, drop_to_plate, lift_over_plate, smallest_scale, transform_mesh,
};
pub use triangle::Triangle;
pub use triangulate::triangulate;
pub use uv::{Heightmap, Mapping, UvMap};
pub use validate::{MeshDiagnostics, center_of_mass, diagnose, signed_volume, signed_volume_x6};
pub use weld::{DEFAULT_WELD_TOLERANCE, Welded, point_key, weld};
pub use winding::{Winding, solid_angle, winding_number};

pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3};

/// The maths library itself, for the corners it covers that this crate does not wrap,
/// such as `glam::camera` for view and projection matrices.
pub use glam;
pub use parry3d::bounding_volume::Aabb;

/// Scalar type used across the whole pipeline.
pub type Scalar = f32;
