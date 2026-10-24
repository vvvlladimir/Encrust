use super::{SURFACE_EPSILON_MM, face_normal, has_room};
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

/// How many points round its rim a contact's circle is checked on for lying on its face.
const RIM_POINTS: u32 = 8;

/// How far off the contact's plane its rim may find the face, as a share of the radius: a face
/// curving away by up to 26 degrees over one radius still holds the whole tip.
const RIM_DEPTH: Scalar = 0.5;

/// The least cosine between the contact's face and the face its rim finds: a facet of a
/// coarse curve is still the same face, a wall meeting it at an edge is not.
const RIM_SAME_FACE_COS: Scalar = 0.5;

/// The nearest spot on a face looking down at `contact`'s height where a head has room and the
/// contact's circle lies wholly on its face, within [`MOVE_REACH_MM`] or one radius and a ring;
/// failing that, where a head has room alone. `None` when `contact` needs no move or no spot
/// will do (ADR 0348, 0354).
pub(super) fn roomier(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> Option<Vec3> {
    let room = has_room(placed, contact, profile);
    eprintln!("DBG room={room} seated={}", seated(placed, contact, profile));
    if room && seated(placed, contact, profile) {
        return None;
    }
    let reach_mm = MOVE_REACH_MM.max(profile.contact_radius_mm() + RING_MM);
    let spots = || {
        let rings = (reach_mm / RING_MM).ceil() as u32;
        (1..=rings)
            .flat_map(|ring| {
                (0..AROUND).map(move |step| {
                    let turn = std::f32::consts::TAU * step as Scalar / AROUND as Scalar;
                    Vec3::new(turn.cos(), turn.sin(), 0.0) * (ring as Scalar * RING_MM)
                })
            })
            .filter_map(move |offset| face_under(placed, contact + offset))
            .filter(|spot| !placed.blocks_at(*spot) && has_room(placed, *spot, profile))
    };
    let held = spots().find(|spot| seated(placed, *spot, profile));
    match (held, room) {
        (Some(spot), _) => Some(spot),
        (None, true) => None,
        (None, false) => spots().find(|spot| spot.distance(contact) <= MOVE_REACH_MM),
    }
}

/// Whether the contact's circle round `contact` lies wholly on the face it sits on: a tip half
/// over an edge cures onto the wall past it.
fn seated(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> bool {
    let Some(normal) = face_normal(placed, contact) else {
        eprintln!("DBG no normal at {contact:?}");
        return true;
    };
    eprintln!("DBG normal {normal:?} at {contact:?}");
    let radius_mm = profile.contact_radius_mm();
    let depth_mm = RIM_DEPTH * radius_mm;
    let across = normal.any_orthonormal_vector();
    let along = normal.cross(across);
    (0..RIM_POINTS).all(|step| {
        let turn = std::f32::consts::TAU * step as Scalar / RIM_POINTS as Scalar;
        let rim = contact + (across * turn.cos() + along * turn.sin()) * radius_mm;
        let ray = Ray::new(rim + normal * depth_mm, -normal);
        placed
            .raycast_within(&ray, 2.0 * depth_mm)
            .inspect(|hit| eprintln!("DBG rim {rim:?} hit {:?} n {:?}", hit.point, hit.normal))
            .is_some_and(|hit| hit.normal.dot(normal) >= RIM_SAME_FACE_COS)
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
