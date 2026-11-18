//! The blocker a click puts down, fitted, sized or run on as a rod, and the one a click
//! through the model meets (ADR 0396).

use core_geometry::{Bvh, Mesh, Ray, Scalar, Transform, Vec3, raycast_placed};

use serde::{Deserialize, Serialize};

use crate::{Blocker, ModelHollow};

/// How far past the clicked face the ray across the part starts, in plate millimetres: off
/// the face it was clicked on, far under any wall a part can be printed with.
const START_INSIDE_MM: Scalar = 1e-3;

/// How far a blocker of the size a user has not set reaches past the wall, millimetres.
pub const BLOCKER_REACH_MM: Scalar = 4.0;

/// How a click sizes the blocker it puts down.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BlockerSize {
    /// Whether the ball fills the part under the click; `reach_mm` stands where it has no far side.
    pub fit: bool,
    /// How far the blocker reaches past the wall into the cavity, plate millimetres.
    pub reach_mm: Scalar,
}

impl Default for BlockerSize {
    fn default() -> Self {
        Self {
            fit: true,
            reach_mm: BLOCKER_REACH_MM,
        }
    }
}

/// The blocker a click at `point` on a face of outward `normal`, in plate coordinates, puts
/// down by `size`, kept in model space; `None` where the placement has flattened the model.
pub fn blocker_under(
    mesh: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    point: Vec3,
    normal: Vec3,
    size: BlockerSize,
) -> Option<Blocker> {
    size.fit
        .then(|| fitted_blocker(mesh, bvh, transform, point, normal))
        .flatten()
        .or_else(|| ModelHollow::blocker_at(point, size.reach_mm, transform))
}

/// A rod from where `last` ends to where `next` stands, as wide as `next`: a limb kept solid a
/// click at a time.
pub fn rod_on(last: &Blocker, next: &Blocker) -> Blocker {
    Blocker::along(last.to, next.from, next.radius_mm)
}

/// The blockers a user placed, each walled by the shell's `wall_mm` so that its radius is how
/// far it reaches past the wall.
pub fn walled_by(blockers: &[Blocker], wall_mm: Scalar) -> impl Iterator<Item = Blocker> + '_ {
    blockers.iter().map(move |blocker| blocker.walled(wall_mm))
}

/// The ball filling `mesh` as placed under a click at `point` on a face of outward `normal`,
/// in plate coordinates, kept in model space; `None` where the part has no far side there.
pub fn fitted_blocker(
    mesh: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    point: Vec3,
    normal: Vec3,
) -> Option<Blocker> {
    let inward = -normal.try_normalize()?;
    let width_mm = across_mm(mesh, bvh, transform, point, inward)?;
    let center = point + inward * (width_mm / 2.0);
    ModelHollow::blocker_at(center, width_mm / 2.0, transform)
}

/// How far `mesh` as placed runs from `point` on its surface along `direction` to its far
/// side, in plate millimetres; `None` where the ray finds no far side.
pub fn across_mm(
    mesh: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    point: Vec3,
    direction: Vec3,
) -> Option<Scalar> {
    let direction = direction.try_normalize()?;
    let start = point + direction * START_INSIDE_MM;
    let far = raycast_placed(mesh, bvh, transform, &Ray::new(start, direction))?;
    Some(far.distance + START_INSIDE_MM)
}

/// The index of the blocker `ray` crosses nearest its origin among those it enters within
/// `reach` of it, or `None` where it crosses none.
pub(crate) fn first_crossed(blockers: &[Blocker], ray: &Ray, reach: Scalar) -> Option<usize> {
    blockers
        .iter()
        .enumerate()
        .filter_map(|(index, blocker)| {
            let along = nearest_along(ray, blocker.from, blocker.to);
            let depth = blocker.depth_at(ray.at(along));
            (depth >= 0.0 && entered_at(blocker, along, depth) <= reach).then_some((index, along))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

/// Where a ray coming nearest `blocker`'s axis `along` itself, `depth` inside it, enters it:
/// half a chord of its round section back.
fn entered_at(blocker: &Blocker, along: Scalar, depth: Scalar) -> Scalar {
    let radius = blocker.core_mm + blocker.radius_mm;
    let half_chord = (depth * (2.0 * radius - depth)).max(0.0).sqrt();
    (along - half_chord).max(0.0)
}

/// How far along `ray` it comes nearest the segment `from`–`to`, never behind its origin:
/// the clamped closest points of two segments (Ericson, Real-Time Collision Detection, 5.1.9).
fn nearest_along(ray: &Ray, from: Vec3, to: Vec3) -> Scalar {
    let span = to - from;
    let offset = ray.origin - from;
    let skew = ray.direction.dot(span);
    let span_sq = span.length_squared();
    let ray_off = ray.direction.dot(offset);
    let span_off = span.dot(offset);
    let denominator = span_sq - skew * skew;
    let along = if denominator > Scalar::EPSILON {
        ((skew * span_off - span_sq * ray_off) / denominator).max(0.0)
    } else {
        (-ray_off).max(0.0)
    };
    if span_sq <= Scalar::EPSILON {
        return along;
    }
    let on_span = ((skew * along + span_off) / span_sq).clamp(0.0, 1.0);
    (skew * on_span - ray_off).max(0.0)
}
