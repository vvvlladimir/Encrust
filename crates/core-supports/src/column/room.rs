use super::{SURFACE_EPSILON_MM, has_room};
use crate::islands::MOVE_REACH_MM;
use crate::placed::Placed;
use core_geometry::{Ray, Scalar, Vec3};
use printer_profiles::SupportProfile;

/// How far apart the rings a contact with no room for a head looks round on are, mm, and how
/// many ways round each: a sixth of the reach, and every 30 degrees.
const RING_MM: Scalar = MOVE_REACH_MM / 6.0;

const AROUND: u32 = 12;

/// How far above or below the contact a spot may lie and still hold what it held, mm: a layer
/// of the finest printer, so the first layer of an island stays touched.
const LEVEL_MM: Scalar = 0.05;

/// The nearest spot on a face looking down within [`MOVE_REACH_MM`] of `contact`, at its height,
/// where a head has room, if `contact` has none; `None` when it has room or no spot does (ADR 0348).
pub(super) fn roomier(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> Option<Vec3> {
    if has_room(placed, contact, profile) {
        return None;
    }
    let rings = (MOVE_REACH_MM / RING_MM).round() as u32;
    (1..=rings)
        .flat_map(|ring| {
            (0..AROUND).map(move |step| {
                let turn = std::f32::consts::TAU * step as Scalar / AROUND as Scalar;
                Vec3::new(turn.cos(), turn.sin(), 0.0) * (ring as Scalar * RING_MM)
            })
        })
        .filter_map(|offset| face_under(placed, contact + offset))
        .find(|spot| !placed.blocks_at(*spot) && has_room(placed, *spot, profile))
}

/// The face looking down at `at`'s height, within [`LEVEL_MM`] of it, over `at`'s place.
fn face_under(placed: &Placed, at: Vec3) -> Option<Vec3> {
    let from = at - Vec3::Z * LEVEL_MM;
    let ray = Ray::new(from, Vec3::Z);
    placed
        .raycast_within(&ray, 2.0 * LEVEL_MM + SURFACE_EPSILON_MM)
        .filter(|hit| hit.normal.z < 0.0)
        .map(|hit| hit.point)
}
