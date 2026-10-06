//! Geometric primitives shared by the whole slicing pipeline, plus the mesh repair and
//! inspection that every consumer needs before it can trust a mesh.
//!
//! Types here are plain data; the operations on them are free functions. Vector types
//! come from `glam` so that meshes reach the GPU without a conversion pass; see
//! `docs/decisions/0003-math-library.md`.

mod bvh;
mod clip;
mod closest;
mod fill;
mod hash;
mod mesh;
mod orient;
mod ray;
mod split;
mod topology;
mod transform;
mod triangle;
mod triangulate;
mod uv;
mod validate;
mod weld;
mod winding;

pub use bvh::Bvh;
pub use clip::{Cut, Plane, cut};
pub use closest::{ClosestPoint, closest_point, point_aabb_squared, point_triangle};
pub use fill::{Filled, fill_holes};
pub use hash::{FastHasher, FastMap, FastSet};
pub use mesh::Mesh;
pub use orient::{Orientation, orient_outward};
pub use ray::{PlacedHit, Ray, RayHit, ray_aabb, ray_triangle, raycast, raycast_placed};
pub use split::split;
pub use topology::Adjacency;
pub use transform::{Transform, center_over_plate, drop_to_plate, lift_over_plate, transform_mesh};
pub use triangle::Triangle;
pub use uv::{Heightmap, Mapping, UvMap};
pub use validate::{MeshDiagnostics, center_of_mass, diagnose, signed_volume, signed_volume_x6};
pub use weld::{DEFAULT_WELD_TOLERANCE, Welded, weld};
pub use winding::{Winding, solid_angle, winding_number};

pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3};

/// The maths library itself, for the corners it covers that this crate does not wrap,
/// such as `glam::camera` for view and projection matrices.
pub use glam;
pub use parry3d::bounding_volume::Aabb;

/// Scalar type used across the whole pipeline.
pub type Scalar = f32;
