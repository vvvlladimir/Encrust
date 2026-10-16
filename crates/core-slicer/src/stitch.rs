use std::collections::hash_map::Entry;
use std::hash::BuildHasherDefault;

use core_geometry::{FastMap, Vec2};

use crate::Contour;
use crate::plane::{Crossing, EdgeKey};

/// What one plane's crossings turned into, and what had to be papered over.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Stitched {
    pub contours: Vec<Contour>,
    pub open: usize,
    pub degenerate: usize,
    pub unlinked: usize,
}

/// Chains crossings into closed contours by following the mesh edges they sit on.
///
/// On a closed, consistently wound mesh every crossed edge is used once as an entry and
/// once as an exit, so the chain is a permutation and every loop closes exactly. Where two
/// sheets of surface share an edge it is entered and left twice, and the chain takes the
/// sharpest left turn there, which keeps two loops touching at a point two loops.
pub(crate) fn stitch(crossings: Vec<Crossing>) -> Stitched {
    let mut result = Stitched::default();
    let mut pending = Pending::of(crossings);
    result.unlinked = pending.shared;

    // Chains that start at a boundary have to be walked from their head, or an arbitrary
    // starting point would cut one broken contour into two.
    for head in pending.heads() {
        take_contour(&mut pending, head, &mut result);
    }
    for at in 0..pending.crossings.len() {
        if !pending.used[at] {
            let start = pending.crossings[at].start_edge;
            take_contour(&mut pending, start, &mut result);
        }
    }
    result
}

/// The crossings of one plane, each findable by the edge it leaves from, and which of them
/// are chained already. Nearly every edge has one crossing leaving it; an edge two sheets
/// of surface share has two, linked through `next`.
struct Pending {
    crossings: Vec<Crossing>,
    /// The first crossing leaving each edge.
    first: FastMap<EdgeKey, u32>,
    /// The next crossing leaving the same edge, or `NONE`.
    next: Vec<u32>,
    used: Vec<bool>,
    /// Whether each crossing is the first to leave its edge.
    leads: Vec<bool>,
    /// Crossings that left an edge another had left already.
    shared: usize,
}

const NONE: u32 = u32::MAX;

impl Pending {
    fn of(crossings: Vec<Crossing>) -> Self {
        let mut first: FastMap<EdgeKey, u32> =
            FastMap::with_capacity_and_hasher(crossings.len(), BuildHasherDefault::default());
        let mut next = vec![NONE; crossings.len()];
        let mut last = vec![NONE; crossings.len()];
        let mut leads = vec![false; crossings.len()];
        let mut shared = 0;
        for (at, crossing) in crossings.iter().enumerate() {
            let at = at as u32;
            match first.entry(crossing.start_edge) {
                Entry::Occupied(head) => {
                    let head = *head.get() as usize;
                    shared += 1;
                    let tail = last[head];
                    next[tail as usize] = at;
                    last[head] = at;
                }
                Entry::Vacant(slot) => {
                    slot.insert(at);
                    last[at as usize] = at;
                    leads[at as usize] = true;
                }
            }
        }
        Self {
            used: vec![false; crossings.len()],
            crossings,
            first,
            next,
            leads,
            shared,
        }
    }

    /// The crossings leaving `edge` and not chained yet, first in first.
    fn leaving(&self, edge: EdgeKey) -> impl Iterator<Item = usize> + '_ {
        let mut at = self.first.get(&edge).copied().unwrap_or(NONE);
        std::iter::from_fn(move || {
            while at != NONE {
                let here = at as usize;
                at = self.next[here];
                if !self.used[here] {
                    return Some(here);
                }
            }
            None
        })
    }

    /// Edges more crossings leave from than arrive at, once for each one over: the open
    /// ends of chains, in the order the crossings came.
    fn heads(&self) -> Vec<EdgeKey> {
        // Arrivals are counted against the first crossing leaving the same edge.
        let mut arriving = vec![0usize; self.crossings.len()];
        for crossing in &self.crossings {
            if let Some(&first) = self.first.get(&crossing.end_edge) {
                arriving[first as usize] += 1;
            }
        }
        let mut heads = Vec::new();
        for (at, crossing) in self.crossings.iter().enumerate() {
            if !self.leads[at] {
                continue;
            }
            let over = self.chain_from(at).saturating_sub(arriving[at]);
            heads.extend(std::iter::repeat_n(crossing.start_edge, over));
        }
        heads
    }

    /// How many crossings leave the edge crossing `at` is the first to leave.
    fn chain_from(&self, at: usize) -> usize {
        let mut count = 0;
        let mut here = at as u32;
        while here != NONE {
            count += 1;
            here = self.next[here as usize];
        }
        count
    }
}

fn take_contour(pending: &mut Pending, start: EdgeKey, result: &mut Stitched) {
    let (points, closed) = walk(pending, start);
    if !closed {
        result.open += 1;
    }
    match Contour::from_points(points) {
        Some(contour) => result.contours.push(contour),
        None => result.degenerate += 1,
    }
}

/// Follows the chain from `start` back to itself, consuming every crossing it uses.
fn walk(pending: &mut Pending, start: EdgeKey) -> (Vec<Vec2>, bool) {
    let mut points = Vec::new();
    let mut edge = start;
    let mut last: Option<Crossing> = None;

    while let Some(crossing) = take(pending, edge, last.as_ref()) {
        points.push(crossing.start);
        last = Some(crossing);
        if crossing.end_edge == start {
            return (points, true);
        }
        edge = crossing.end_edge;
    }

    // The chain ran into an open or already-consumed edge. Keep the last point and let
    // the contour's implicit closing edge bridge the gap.
    if let Some(last) = last {
        points.push(last.end);
    }
    (points, false)
}

/// Takes the crossing leaving `edge` that turns furthest left from `arriving`, or the
/// first there is.
fn take(pending: &mut Pending, edge: EdgeKey, arriving: Option<&Crossing>) -> Option<Crossing> {
    let chosen = {
        let mut leaving = pending.leaving(edge);
        let first = leaving.next()?;
        match (arriving, leaving.next()) {
            (Some(arriving), Some(second)) => {
                let heading = arriving.end - arriving.start;
                let turn = |at: usize| {
                    let crossing = &pending.crossings[at];
                    let next = crossing.end - crossing.start;
                    heading.perp_dot(next).atan2(heading.dot(next))
                };
                [first, second]
                    .into_iter()
                    .chain(leaving)
                    .max_by(|a, b| turn(*a).total_cmp(&turn(*b)))
                    .unwrap_or(first)
            }
            _ => first,
        }
    };
    pending.used[chosen] = true;
    Some(pending.crossings[chosen])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Winding;

    fn crossing(start_edge: EdgeKey, end_edge: EdgeKey, start: Vec2, end: Vec2) -> Crossing {
        Crossing {
            start_edge,
            end_edge,
            start,
            end,
        }
    }

    /// A unit square walked counter-clockwise over four edges numbered 0 to 3.
    fn square() -> Vec<Crossing> {
        let corners = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        (0..4)
            .map(|i| {
                crossing(
                    (i, i),
                    ((i + 1) % 4, (i + 1) % 4),
                    corners[i as usize],
                    corners[(i as usize + 1) % 4],
                )
            })
            .collect()
    }

    #[test]
    fn a_closed_chain_becomes_one_counter_clockwise_contour() {
        let stitched = stitch(square());

        assert_eq!(stitched.contours.len(), 1);
        assert_eq!(stitched.contours[0].winding, Winding::Outer);
        assert_eq!(stitched.contours[0].points.len(), 4);
        assert_eq!(
            (stitched.open, stitched.degenerate, stitched.unlinked),
            (0, 0, 0)
        );
    }

    #[test]
    fn a_broken_chain_is_closed_and_counted() {
        let mut crossings = square();
        crossings.pop();
        let stitched = stitch(crossings);

        assert_eq!(stitched.open, 1);
        assert_eq!(stitched.contours.len(), 1);
        assert_eq!(
            stitched.contours[0].points.len(),
            4,
            "three segments carry three starts plus the last end point"
        );
    }

    /// Two unit squares meeting at one corner, over edges each of them uses: the shared
    /// corner is entered twice and left twice, the way it is where two sheets of a cavity
    /// touch along an edge of the mesh.
    fn touching_squares() -> Vec<Crossing> {
        let low = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        let high = low.map(|corner| corner + Vec2::ONE);
        // Edge 2 is the shared corner: the low square's third, the high square's first.
        let low_edges = [0, 1, 2, 3];
        let high_edges = [2, 11, 12, 13];
        let mut crossings = Vec::new();
        for (corners, edges) in [(low, low_edges), (high, high_edges)] {
            for i in 0..4 {
                crossings.push(crossing(
                    (edges[i], edges[i]),
                    (edges[(i + 1) % 4], edges[(i + 1) % 4]),
                    corners[i],
                    corners[(i + 1) % 4],
                ));
            }
        }
        crossings
    }

    #[test]
    fn an_edge_two_sheets_share_is_reported_and_still_closes_both_loops() {
        let stitched = stitch(touching_squares());

        assert_eq!(stitched.unlinked, 1, "the shared edge is left twice");
        assert_eq!(stitched.open, 0, "no chain is closed over a gap");
        assert_eq!(
            stitched.contours.len(),
            2,
            "two squares, not one figure of eight"
        );
        assert!(
            stitched
                .contours
                .iter()
                .all(|contour| contour.points.len() == 4 && contour.winding == Winding::Outer),
            "each is a whole square wound as an outer contour"
        );
    }

    #[test]
    fn a_chain_enclosing_no_area_is_dropped() {
        let there_and_back = vec![
            crossing((0, 0), (1, 1), Vec2::ZERO, Vec2::X),
            crossing((1, 1), (0, 0), Vec2::X, Vec2::ZERO),
        ];
        let stitched = stitch(there_and_back);

        assert!(stitched.contours.is_empty());
        assert_eq!(stitched.degenerate, 1);
    }

    #[test]
    fn two_separate_rings_become_two_contours() {
        let mut crossings = square();
        crossings.extend((10..14).map(|i| {
            let corners = [
                Vec2::new(5.0, 5.0),
                Vec2::new(6.0, 5.0),
                Vec2::new(6.0, 6.0),
                Vec2::new(5.0, 6.0),
            ];
            let k = (i - 10) as usize;
            crossing(
                (i, i),
                (10 + (i - 10 + 1) % 4, 10 + (i - 10 + 1) % 4),
                corners[k],
                corners[(k + 1) % 4],
            )
        }));
        let stitched = stitch(crossings);

        assert_eq!(stitched.contours.len(), 2);
        assert_eq!(stitched.open, 0);
    }

    #[test]
    fn no_crossings_yield_no_contours() {
        assert_eq!(stitch(Vec::new()), Stitched::default());
    }
}
