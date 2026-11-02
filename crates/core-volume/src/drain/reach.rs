use std::f32::consts::TAU;

use core_geometry::{Bvh, Mesh, Ray, Scalar, Vec3};

use super::{DrainHole, HoleSize, frame, lift_for};

/// Rays cast round the vertical, once level and once tilted 45° toward it.
const RAYS_AROUND: usize = 8;

/// The most faces one ray is followed through: past it the ray is crossing a lattice, not a wall.
const MOST_HITS: usize = 32;

/// How far past a face the next cast starts, millimetres, so it does not meet that face again.
const PAST_FACE_MM: Scalar = 1e-3;

/// How far off the mouth's plane the surface under its rim may stand, as a share of its radius:
/// a sphere of 2.5 radii curves that far away; off an edge or a fin there is no surface at all.
const RIM_SAG: Scalar = 0.2;

/// Heights closer than this rank as one, and the mouth nearer the cavity wins, millimetres.
const LEVEL_STEP_MM: Scalar = 0.1;

/// Points of the rim asked whether they stand on the surface.
const RIM_POINTS: usize = 8;

/// How far over the plate a mouth's lowest point stays, millimetres: the bottom layers
/// spread on the plate and close a hole lower than this.
const PLATE_CLEARANCE_MM: Scalar = 1.0;

/// Which way out of a cavity a hole is looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward {
    /// As low as it goes, preferring faces that look down: a drain.
    Down,
    /// As high as it goes: a vent letting air in over the draining resin.
    Up,
}

/// Holes of `size` from the outer surface of `mesh` into the cavity around `inside`, best
/// first, in the frame the mesh prints in, +Z up; see `docs/design/hollowing.md`.
///
/// Each is drilled square into the face a ray out of the cavity leaves by; one whose ray crossed
/// more than the cavity's wall comes after every one that did not. With `plate_z`, the model
/// stands on the plate there and no mouth comes near it.
pub fn holes_out(
    mesh: &Mesh,
    bvh: &Bvh,
    inside: Vec3,
    toward: Toward,
    size: HoleSize,
    plate_z: Option<Scalar>,
) -> Vec<DrainHole> {
    let radius = size.diameter_mm / 2.0;
    let Some(origin) = origin_of(bvh, inside, toward, radius) else {
        return Vec::new();
    };
    let mut found: Vec<((bool, i64, Scalar), DrainHole)> = directions(toward)
        .filter_map(|direction| {
            let way = way_out(mesh, bvh, Ray::new(origin, direction))?;
            let hole = way.hole(mesh, bvh, size, plate_z)?;
            let height = match toward {
                Toward::Down => hole.at.z + radius * way.normal.z,
                Toward::Up => -hole.at.z,
            };
            let level = (height / LEVEL_STEP_MM).round() as i64;
            Some(((way.crossed > 2, level, hole.at.distance(origin)), hole))
        })
        .collect();
    found.sort_by(|(a, _), (b, _)| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)));
    found.into_iter().map(|(_, hole)| hole).collect()
}

/// Where the rays start: a radius over the cavity's floor for a drain, so the hole's lowest
/// point is level with it, and a radius under its ceiling for a vent. `None` when `inside`
/// has nothing over it, so is in no cavity.
fn origin_of(bvh: &Bvh, inside: Vec3, toward: Toward, radius: Scalar) -> Option<Vec3> {
    let room = bvh.raycast(&Ray::new(inside, Vec3::Z))?.t;
    let rise = match toward {
        Toward::Down => radius.min(room / 2.0),
        Toward::Up => room - radius.min(room / 2.0),
    };
    Some(inside + Vec3::Z * rise)
}

/// Straight down or up, then round it at 45°, then level.
fn directions(toward: Toward) -> impl Iterator<Item = Vec3> {
    let vertical = match toward {
        Toward::Down => Vec3::NEG_Z,
        Toward::Up => Vec3::Z,
    };
    let around = |tilt: Scalar| {
        (0..RAYS_AROUND).map(move |step| {
            let (sin, cos) = (TAU * step as Scalar / RAYS_AROUND as Scalar).sin_cos();
            (Vec3::new(cos, sin, 0.0) + vertical * tilt).normalize()
        })
    };
    std::iter::once(vertical)
        .chain(around(1.0))
        .chain(around(0.0))
}

/// Where a ray out of the cavity leaves the model for good.
struct WayOut {
    /// The mouth, and the outward unit normal of the face it is on.
    mouth: Vec3,
    normal: Vec3,
    /// How far the ray ran in material before the mouth, millimetres: the wall it crossed.
    wall_mm: Scalar,
    /// How many faces the ray crossed from the cavity out.
    crossed: usize,
}

/// The last face `ray` leaves the material through, having entered it by the one before, or
/// `None` when the ray is not out within [`MOST_HITS`] faces or does not end leaving material.
fn way_out(mesh: &Mesh, bvh: &Bvh, ray: Ray) -> Option<WayOut> {
    let mut hits: Vec<(Scalar, Vec3)> = Vec::new();
    let mut t = 0.0;
    loop {
        let from = Ray::new(ray.at(t + PAST_FACE_MM), ray.direction);
        let Some(hit) = bvh.raycast(&from) else {
            break;
        };
        if hits.len() == MOST_HITS {
            return None;
        }
        t += PAST_FACE_MM + hit.t;
        hits.push((t, normal_of(mesh, hit.face)?));
    }
    let [.., (entered, into), (left, out)] = hits[..] else {
        return None;
    };
    let leaves = out.dot(ray.direction) > 0.0 && into.dot(ray.direction) < 0.0;
    leaves.then(|| WayOut {
        mouth: ray.at(left),
        normal: out,
        wall_mm: left - entered,
        crossed: hits.len(),
    })
}

fn normal_of(mesh: &Mesh, face: usize) -> Option<Vec3> {
    mesh.triangle(face)?.normal_unnormalized().try_normalize()
}

impl WayOut {
    /// The hole drilled square into the face the ray left by, a diameter past the wall there,
    /// or `None` where it would open off an edge or a fin, on the plate, or reach no cavity.
    fn hole(
        &self,
        mesh: &Mesh,
        bvh: &Bvh,
        size: HoleSize,
        plate_z: Option<Scalar>,
    ) -> Option<DrainHole> {
        let (out, radius) = (self.normal, size.diameter_mm / 2.0);
        let lowest = self.mouth.z - radius * (1.0 - out.z.powi(2)).max(0.0).sqrt();
        let on_plate = plate_z.is_some_and(|plate| lowest < plate + PLATE_CLEARANCE_MM);
        if on_plate || !self.rim_on_surface(mesh, bvh, radius) {
            return None;
        }
        let inward = Ray::new(self.mouth - out * PAST_FACE_MM, -out);
        // Square to the face the wall is no thicker than the ray crossed it; further is a far side.
        let wall = bvh.raycast_within(&inward, self.wall_mm)?;
        if normal_of(mesh, wall.face)?.dot(out) > 0.0 {
            return None;
        }
        let depth_mm = size.depth_mm.max(PAST_FACE_MM + wall.t + size.diameter_mm);
        let lift_mm = lift_for(mesh, bvh, self.mouth, -out, size.diameter_mm, depth_mm);
        Some(DrainHole::sized(
            self.mouth,
            -out,
            HoleSize { depth_mm, ..size },
            lift_mm,
        ))
    }

    /// Whether the surface lies under every point of the mouth's rim, within [`RIM_SAG`].
    fn rim_on_surface(&self, mesh: &Mesh, bvh: &Bvh, radius: Scalar) -> bool {
        let out = self.normal;
        let (right, up) = frame(out);
        let sag = RIM_SAG * radius;
        (0..RIM_POINTS).all(|point| {
            let (sin, cos) = (TAU * point as Scalar / RIM_POINTS as Scalar).sin_cos();
            let over = self.mouth + (right * cos + up * sin) * radius + out * sag;
            bvh.raycast_within(&Ray::new(over, -out), 2.0 * sag)
                .and_then(|hit| normal_of(mesh, hit.face))
                .is_some_and(|normal| normal.dot(out) > 0.0)
        })
    }
}

#[cfg(test)]
mod tests;
