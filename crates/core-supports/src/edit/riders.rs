use core_geometry::{FastSet, Scalar, Vec3};

use crate::Landing;
use crate::branch::{Carrier, meeting_on};
use crate::column::{MAX_LEAN_PER_DROP, MIN_PILLAR_HEIGHT_MM};
use crate::group::Profiles;
use crate::placed::Placed;
use crate::space::Space;
use crate::tree::SupportTree;

/// How far off a host's axis a landing may lie and still be on it, mm: growth puts it
/// on the axis, and a round trip through model space moves it by float error alone.
const SEAT_MM: Scalar = 1e-2;

/// Where a support standing on another meets it: on the strut hanging under `strut`, or the
/// trunk when `None`, a `share` of the way from its top to its bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seat {
    strut: Option<usize>,
    share: Scalar,
}

/// Every tree of `trees` (plate coordinates) standing on `host`, directly or on one that
/// does, `host` left out: what goes with it when a hand takes hold of it (ADR 0431).
pub fn standing_on(trees: &[SupportTree], host: usize, profiles: Profiles) -> Vec<usize> {
    let mut found = Vec::new();
    let mut seen = FastSet::default();
    seen.insert(host);
    let mut hosts = vec![host];
    // Each tree is taken once, so the walk ends within as many rounds as there are trees.
    while let Some(under) = hosts.pop() {
        let Some(host_tree) = trees.get(under) else {
            continue;
        };
        for (rider, _) in seated_on(trees, host_tree, profiles) {
            if seen.insert(rider) {
                found.push(rider);
                hosts.push(rider);
            }
        }
    }
    found
}

/// Every tree of `trees` (plate coordinates) that `rider` stands on, directly or through one
/// that does: what must stay as it is for `rider` to keep its seat.
pub(crate) fn carrying(trees: &[SupportTree], rider: usize, profiles: Profiles) -> Vec<usize> {
    let mut found = Vec::new();
    let mut at = rider;
    // Each tree is taken once, so the walk ends within as many steps as there are trees.
    while let Some(seated) = trees.get(at).filter(|tree| tree.landing().on_support) {
        let base = seated.landing().base;
        let Some(host) = (0..trees.len()).find(|host| {
            *host != rider
                && !found.contains(host)
                && seat_of(base, &trees[*host], profiles).is_some()
        }) else {
            break;
        };
        found.push(host);
        at = host;
    }
    found
}

/// The trees of `trees` (plate coordinates) standing on a support that none of the others is:
/// what was under them has gone or moved.
pub(crate) fn unseated(trees: &[SupportTree], profiles: Profiles) -> Vec<usize> {
    (0..trees.len())
        .filter(|rider| {
            let base = trees[*rider].landing().base;
            trees[*rider].landing().on_support
                && !(trees.iter().enumerate())
                    .any(|(host, tree)| host != *rider && seat_of(base, tree, profiles).is_some())
        })
        .collect()
}

/// Every tree to put back, `carried` first, with its index: `frozen[host]` (model space) as
/// `carried`, then those standing on it, in turn, seated where they were on it (ADR 0431).
pub fn followed(
    frozen: &[SupportTree],
    host: usize,
    carried: &SupportTree,
    placed: &Placed,
    profiles: Profiles,
) -> Option<Vec<(usize, SupportTree)>> {
    let space = Space::of(placed)?;
    let plate: Vec<SupportTree> = frozen.iter().map(|tree| space.plate_tree(tree)).collect();

    let mut moved = vec![(host, carried.clone())];
    let mut seen = FastSet::default();
    seen.insert(host);
    let mut hosts = vec![(plate.get(host)?.clone(), space.plate_tree(carried))];
    // Each tree is taken once, so the walk ends within as many rounds as there are trees.
    while let Some((before, after)) = hosts.pop() {
        for (rider, seat) in seated_on(&plate, &before, profiles) {
            if !seen.insert(rider) {
                continue;
            }
            let base = seat_point(&after, seat, profiles)?;
            let carried = reseated(&plate[rider], base);
            let carried = within_lean(&carried, &after, seat, profiles).unwrap_or(carried);
            moved.push((rider, space.model_tree(&carried)));
            hosts.push((plate[rider].clone(), carried));
        }
    }
    Some(moved)
}

/// The trees of `trees` standing on `host`, all in plate coordinates, each with its seat.
fn seated_on(trees: &[SupportTree], host: &SupportTree, profiles: Profiles) -> Vec<(usize, Seat)> {
    trees
        .iter()
        .enumerate()
        .filter(|(_, tree)| tree.landing().on_support)
        .filter_map(|(index, tree)| Some((index, seat_of(tree.landing().base, host, profiles)?)))
        .collect()
}

/// The seat on `host` that `at` lies on, the nearest when two struts meet there.
fn seat_of(at: Vec3, host: &SupportTree, profiles: Profiles) -> Option<Seat> {
    axes(host, profiles)
        .filter_map(|(strut, top, bottom)| {
            let along = bottom - top;
            let share = ((at - top).dot(along) / along.length_squared().max(Scalar::EPSILON))
                .clamp(0.0, 1.0);
            let off_mm = at.distance(top + along * share);
            (off_mm <= SEAT_MM).then_some((Seat { strut, share }, off_mm))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(seat, _)| seat)
}

/// Where `seat` is on `host` (plate coordinates) as it now stands.
fn seat_point(host: &SupportTree, seat: Seat, profiles: Profiles) -> Option<Vec3> {
    axes(host, profiles)
        .find(|(strut, _, _)| *strut == seat.strut)
        .map(|(_, top, bottom)| top.lerp(bottom, seat.share))
}

/// The axes another support may stand on, as growth offers them (`branch::carriers`): every
/// strut not under a tip, named by the node it hangs under, and the trunk down to the foot.
fn axes<'a>(
    host: &'a SupportTree,
    profiles: Profiles,
) -> impl Iterator<Item = (Option<usize>, Vec3, Vec3)> + use<'a> {
    let nodes = host.nodes();
    let struts = host
        .struts()
        .filter(|(child, _)| !nodes[*child].is_leaf())
        .map(|(child, parent)| (Some(child), nodes[child].position, nodes[parent].position));
    let (root, landing) = (host.root(), host.landing());
    let bottom = landing
        .base
        .with_z(landing.pillar_bottom_z(profiles.of(host.group())));
    let trunk = (!root.is_leaf()).then_some((None, root.position, bottom));
    struts.chain(trunk)
}

/// `rider` (plate) seated as growth seats it on `seat`'s axis of `host` where the seat it kept
/// leans past 45 degrees (ADR 0431); `None` where it does not, or no point is in reach.
fn within_lean(
    rider: &SupportTree,
    host: &SupportTree,
    seat: Seat,
    profiles: Profiles,
) -> Option<SupportTree> {
    let from = rider.root().position;
    let lean = from - rider.landing().base;
    if lean.truncate().length() <= lean.z * MAX_LEAN_PER_DROP {
        return None;
    }
    let (_, top, bottom) = axes(host, profiles).find(|(strut, _, _)| *strut == seat.strut)?;
    let carrier = Carrier {
        top,
        bottom,
        radius_mm: host.root().radius_mm,
        floor_mm: if seat.strut.is_none() {
            MIN_PILLAR_HEIGHT_MM
        } else {
            0.0
        },
        tree: 0,
    };
    let meet = meeting_on(from, &carrier, profiles.of(rider.group()))?;
    Some(reseated(rider, meet))
}

/// `rider` standing at `base` on its host, plate coordinates, the rest of it where it was.
fn reseated(rider: &SupportTree, base: Vec3) -> SupportTree {
    let mut carried = rider.clone();
    carried.stand_on(Landing {
        base,
        ..rider.landing()
    });
    carried
}

#[cfg(test)]
mod tests;
