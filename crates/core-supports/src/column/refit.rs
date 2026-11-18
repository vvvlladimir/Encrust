use super::{
    MAX_HEAD_LEAN_DEG, STEEPEST_SQUARE_HEAD_DEG, has_head, head_clear, neck_along_mm,
    over_its_contact, tip_direction,
};
use crate::placed::Placed;
use core_geometry::{Scalar, Vec3, turns};
use printer_profiles::SupportProfile;

/// How far a head with no room is turned off the way it would leave, degrees, nearest first.
const TURNS_DEG: [Scalar; 2] = [15.0, 30.0];

/// How many ways round each turn is tried: every 45 degrees.
const AROUND: usize = 8;

/// The shares of its length a head is cut to once no turn of it fits, longest first.
const SHARES: [Scalar; 3] = [1.0, 0.75, 0.5];

/// How far past a limit a way may be and still be within it, as a cosine: float noise.
const NOISE: Scalar = 1e-4;

/// The necks of the heads off `contact` that fit when the one its face sends it along does not,
/// nearest that one first (ADR 0429).
pub(crate) fn refitted_necks<'a>(
    placed: &'a Placed<'a>,
    contact: Vec3,
    profile: &'a SupportProfile,
) -> impl Iterator<Item = Vec3> + 'a {
    let (direction, normal) = tip_direction(placed, contact);
    normal.into_iter().flat_map(move |normal| {
        refits(direction, normal, profile)
            .map(move |(way, length_mm)| neck_along_mm(contact, way, length_mm, profile))
            .filter(move |neck| {
                has_head(contact, *neck)
                    && !over_its_contact(contact, *neck, profile)
                    && head_clear(placed, contact, *neck, normal, profile)
            })
    })
}

/// The ways and lengths, mm, a head refused along `direction` off a face with outward normal
/// `normal` is tried at: turned, then shorter, none further off its face or the plumb (ADR 0429).
fn refits(
    direction: Vec3,
    normal: Vec3,
    profile: &SupportProfile,
) -> impl Iterator<Item = (Vec3, Scalar)> + use<> {
    let down = -Vec3::Z;
    // A head leaving a face that looks up stays refused: no turn of it comes down.
    let downwards = direction.z < 0.0;
    let plumb_cos = direction
        .dot(down)
        .min(STEEPEST_SQUARE_HEAD_DEG.to_radians().cos());
    let face_cos = MAX_HEAD_LEAN_DEG.to_radians().cos();
    let (across, over) = direction.any_orthonormal_pair();
    let turned = TURNS_DEG.into_iter().flat_map(move |turn_deg| {
        let (sin, cos) = turn_deg.to_radians().sin_cos();
        turns(AROUND).map(move |round| direction * cos + (across * round.x + over * round.y) * sin)
    });
    let ways: Vec<Vec3> = std::iter::once(direction)
        .chain(turned)
        .filter(|way| downwards && way.z < 0.0)
        .filter(|way| way.dot(normal) >= face_cos - NOISE && way.dot(down) >= plumb_cos - NOISE)
        .collect();
    let length_mm = profile.top.length_mm;
    SHARES
        .into_iter()
        .flat_map(move |share| {
            let ways = ways.clone();
            ways.into_iter().map(move |way| (way, length_mm * share))
        })
        // The first is the head already refused.
        .skip(1)
}
