use core_geometry::{Scalar, Vec3, turns};
use printer_profiles::SupportProfile;

use crate::Landing;
use crate::column::{LAST_RESORT_SLOPE_DEG, MAX_LANDING_SLOPE_DEG, landing_within};
use crate::placed::Placed;

/// What a carried trunk of `radius_mm` carrying `tips`, dropped from `from` (plate), stands on:
/// a face growing would choose, else one it takes as a last resort, as growing does (ADR 0437).
pub(super) fn footing_under(
    placed: &Placed,
    from: Vec3,
    radius_mm: Scalar,
    tips: usize,
    profile: &SupportProfile,
) -> Option<Landing> {
    [MAX_LANDING_SLOPE_DEG, LAST_RESORT_SLOPE_DEG]
        .into_iter()
        .find_map(|steepest_deg| {
            landing_within(placed, from, radius_mm, tips, profile, steepest_deg)
        })
}

/// [`footing_under`] `from`, else within [`NEARBY_MM`], with where the trunk drops from; what the
/// foot `last` stood on (plate) is kept while in reach, so an edge is crossed once (ADR 0437).
pub(super) fn footing_near(
    placed: &Placed,
    from: Vec3,
    (radius_mm, tips): (Scalar, usize),
    profile: &SupportProfile,
    last: Landing,
) -> Option<(Vec3, Landing)> {
    let rings = NEARBY_MM.iter().flat_map(|radius_mm| {
        turns(NEARBY_DIRECTIONS).map(move |way| (way * *radius_mm).extend(0.0))
    });
    let mut found = Vec::new();
    for at in std::iter::once(from).chain(rings.map(|shift| from + shift)) {
        let Some(landing) = footing_under(placed, at, radius_mm, tips, profile) else {
            continue;
        };
        if at == from && landing.on_model == last.on_model {
            return Some((at, landing));
        }
        found.push((at, landing));
    }
    let off_last = |at: Vec3| (at - last.base).truncate().length();
    let nearest = |kept: bool| {
        found
            .iter()
            .filter(|(_, landing)| !kept || landing.on_model == last.on_model)
            .min_by(|(a, _), (b, _)| off_last(*a).total_cmp(&off_last(*b)))
    };
    nearest(true)
        .or_else(|| found.iter().find(|(at, _)| *at == from))
        .or_else(|| nearest(false))
        .copied()
}

/// How far around a carried trunk its foot is looked for when nothing under it takes it, mm:
/// further is a different support.
const NEARBY_MM: [Scalar; 2] = [1.0, 2.0];
/// How many directions each of [`NEARBY_MM`] tries.
const NEARBY_DIRECTIONS: usize = 8;

#[cfg(test)]
mod tests;
