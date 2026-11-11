use core_geometry::{FastSet, Scalar, Vec3};

use crate::Landing;
use crate::group::Profiles;
use crate::placed::Placed;
use crate::tree::SupportTree;

use super::fits;

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
/// does, `host` left out: what goes with it when a hand takes hold of it (ADR 0433).
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

/// The trees of `frozen` (model space) standing on `frozen[host]`, seated where they were on it
/// once it is `carried`, and those on them in turn; `None` when one that fit no longer does.
/// Answers every tree to put back, `carried` first, with its index.
pub fn followed(
    frozen: &[SupportTree],
    host: usize,
    carried: &SupportTree,
    placed: &Placed,
    profiles: Profiles,
) -> Option<Vec<(usize, SupportTree)>> {
    let matrix = placed.transform.to_matrix();
    let inverse = placed.transform.inverse_matrix()?;
    let plate: Vec<SupportTree> = frozen.iter().map(|tree| tree.moved(matrix)).collect();

    let mut moved = vec![(host, carried.clone())];
    let mut seen = FastSet::default();
    seen.insert(host);
    let mut hosts = vec![(plate.get(host)?.clone(), carried.moved(matrix))];
    // Each tree is taken once, so the walk ends within as many rounds as there are trees.
    while let Some((before, after)) = hosts.pop() {
        for (rider, seat) in seated_on(&plate, &before, profiles) {
            if !seen.insert(rider) {
                continue;
            }
            let profile = profiles.of(plate[rider].group());
            let base = seat_point(&after, seat, profiles)?;
            let carried = reseated(&plate[rider], base);
            if !fits(&carried, placed, profile) && fits(&plate[rider], placed, profile) {
                return None;
            }
            moved.push((rider, carried.moved(inverse)));
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
