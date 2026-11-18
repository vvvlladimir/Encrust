use rayon::prelude::*;

use super::cells::Grid;
use crate::{Bvh, FastMap, Ray, Scalar, Vec3, sphere_directions};

/// Rays over the whole sphere, of which the half a face looks into is cast: 32 a face.
const RAYS: usize = 64;
/// How far off its face a ray starts, as a share of the model's diagonal, so that it does
/// not meet the face it leaves.
const OFFSET_SHARE: Scalar = 1e-4;

/// How open the surface is in each cube of `grid` and each way it faces, 0 to 1: the share
/// of the sky over it, weighted by the cosine as light falls, that reaches past the model.
pub(super) fn exposure(bvh: &Bvh, grid: &Grid, diagonal_mm: Scalar) -> FastMap<[i32; 4], Scalar> {
    let directions = sphere_directions(RAYS);
    let offset_mm = diagonal_mm * OFFSET_SHARE;
    grid.outlooks
        .par_iter()
        .map(|(key, outlook)| {
            let from = outlook.point + outlook.normal * offset_mm;
            (*key, open_share(bvh, from, outlook.normal, &directions))
        })
        .collect()
}

fn open_share(bvh: &Bvh, from: Vec3, normal: Vec3, directions: &[Vec3]) -> Scalar {
    let (open, sky) = directions
        .iter()
        .map(|direction| (*direction, direction.dot(normal)))
        .filter(|(_, facing)| *facing > 0.0)
        .fold((0.0, 0.0), |(open, sky), (direction, facing)| {
            let escapes = bvh.raycast(&Ray::new(from, direction)).is_none();
            (open + if escapes { facing } else { 0.0 }, sky + facing)
        });
    if sky > 0.0 { open / sky } else { 0.0 }
}
