use core_geometry::Vec3;

use super::grow_beside;
use super::small::small_pillar;
use crate::column::column;
use crate::group::Profiles;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// What growing builds for point `point` of `group` at `contact` beside `standing`, both plate:
/// a whole support or a small pillar, never a mix (ADR 0438, 0439).
pub(crate) fn grown_alone(
    contact: Vec3,
    point: usize,
    group: u16,
    standing: &[SupportTree],
    placed: &Placed,
    profiles: Profiles,
) -> Option<SupportTree> {
    column(placed, point, group, contact, profiles.of(group))
        .ok()
        .and_then(|column| {
            grow_beside(&[column], standing, placed, profiles)
                .into_iter()
                .find(|tree| tree.points().any(|held| held == point))
        })
        .or_else(|| small_pillar(contact, point, group, placed, profiles))
}
