use super::{SURFACE_EPSILON_MM, face_normal, has_room, straight_neck};
use crate::islands::MOVE_REACH_MM;
use crate::placed::Placed;
use core_geometry::{Ray, Scalar, Vec3, turns};
use printer_profiles::SupportProfile;

/// How far apart the rings a contact with no room for a head looks round on are, mm, and how
/// many ways round each: a sixth of the reach, and every 30 degrees.
const RING_MM: Scalar = MOVE_REACH_MM / 6.0;

const AROUND: usize = 12;

/// How far above or below the contact a spot may lie and still hold what it held, mm: a layer
/// of the finest printer, so the first layer of an island stays touched.
const LEVEL_MM: Scalar = 0.05;

/// How many points round its rim a contact's circle is checked on for lying on its face.
const RIM_POINTS: usize = 8;

/// How far off the contact's plane its rim may find the face, as a share of the radius: a face
/// curving away by up to 26 degrees over one radius still holds the whole tip.
const RIM_DEPTH: Scalar = 0.5;

/// The least cosine between the contact's face and the face its rim finds: a facet of a
/// coarse curve is still the same face, a wall meeting it at an edge is not.
const RIM_SAME_FACE_COS: Scalar = 0.5;

/// The nearest spot at `contact`'s height where a head has room and the contact's circle lies
/// wholly on its face, else where a head has room alone; `None` when none or no move (ADR 0339).
pub(super) fn roomier(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> Option<Vec3> {
    let room = has_room(placed, contact, profile);
    if room && seated(placed, contact, profile) {
        return None;
    }
    let reach_mm = MOVE_REACH_MM.max(profile.contact_radius_mm() + RING_MM);
    let mut roomy = None;
    for spot in spots(placed, contact, reach_mm)
        .filter(|spot| !placed.blocks_at(*spot) && has_room(placed, *spot, profile))
    {
        if seated(placed, spot, profile) {
            return Some(spot);
        }
        if !room && roomy.is_none() && spot.distance(contact) <= MOVE_REACH_MM {
            roomy = Some(spot);
        }
    }
    roomy
}

/// How far a support with no room is moved along its face at its height to stand, mm: past
/// [`MOVE_REACH_MM`], still well inside the spacing of the contacts round it (ADR 0429).
const MOVE_FAR_MM: Scalar = 1.0;

/// How many spots with room a support with no room is tried at, nearest first.
const MOVED_TRIES: usize = 24;

/// The nearest spots at `contact`'s height past where [`roomier`] looked, each with the neck of
/// the straight head it has room for: where a support that stands nowhere else is moved to.
pub(crate) fn moved<'a>(
    placed: &'a Placed<'a>,
    contact: Vec3,
    profile: &'a SupportProfile,
) -> impl Iterator<Item = (Vec3, Vec3)> + 'a {
    spots(placed, contact, MOVE_FAR_MM)
        .filter(move |spot| spot.distance(contact) > MOVE_REACH_MM && !placed.blocks_at(*spot))
        .filter(move |spot| along_face(placed, contact, *spot))
        .filter_map(move |spot| Some((spot, straight_neck(placed, spot, profile).ok()?)))
        .take(MOVED_TRIES)
}

/// Whether `spot` is reached from `contact` along the face they sit on, under it by
/// [`LEVEL_MM`]: a wall hanging between them parts what each holds.
fn along_face(placed: &Placed, contact: Vec3, spot: Vec3) -> bool {
    let from = contact - Vec3::Z * LEVEL_MM;
    let to = spot - Vec3::Z * LEVEL_MM;
    let ray = Ray::new(from, (to - from).normalize_or_zero());
    placed.raycast_within(&ray, from.distance(to)).is_none()
}

/// The spots of faces looking down round `contact`, ring by ring out to `reach_mm`.
fn spots<'a>(
    placed: &'a Placed<'_>,
    contact: Vec3,
    reach_mm: Scalar,
) -> impl Iterator<Item = Vec3> + 'a {
    let rings = (reach_mm / RING_MM).ceil() as u32;
    (1..=rings)
        .flat_map(|ring| {
            turns(AROUND).map(move |turn| turn.extend(0.0) * (ring as Scalar * RING_MM))
        })
        .filter_map(move |offset| face_under(placed, contact + offset))
}

/// Whether the contact's circle round `contact` lies wholly on the face it sits on: a tip half
/// over an edge cures onto the wall past it.
fn seated(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> bool {
    face_normal(placed, contact).is_none_or(|normal| {
        seat_mm(placed, contact, normal, profile.contact_radius_mm()).is_some()
    })
}

/// How far the face under the rim of a tip of `radius_mm` at `contact` rises towards its body
/// along `normal`, mm, none below the contact; `None` when the rim leaves that face (ADR 0426).
pub(crate) fn seat_mm(
    placed: &Placed,
    contact: Vec3,
    normal: Vec3,
    radius_mm: Scalar,
) -> Option<Scalar> {
    let depth_mm = RIM_DEPTH * radius_mm;
    let across = normal.any_orthonormal_vector();
    let along = normal.cross(across);
    turns(RIM_POINTS).try_fold(0.0, |seat_mm: Scalar, turn| {
        let rim = contact + (across * turn.x + along * turn.y) * radius_mm;
        let ray = Ray::new(rim + normal * depth_mm, -normal);
        let hit = placed
            .raycast_within(&ray, 2.0 * depth_mm)
            .filter(|hit| hit.normal.dot(normal) >= RIM_SAME_FACE_COS)?;
        Some(seat_mm.max(depth_mm - hit.distance))
    })
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

#[cfg(test)]
mod tests;
