use std::collections::{HashMap, HashSet};

use core_geometry::Vec2;

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
/// once as an exit, so the chain is a permutation and every loop closes exactly.
pub(crate) fn stitch(crossings: Vec<Crossing>) -> Stitched {
    let mut result = Stitched::default();
    let mut pending: HashMap<EdgeKey, Crossing> = HashMap::with_capacity(crossings.len());
    for crossing in crossings {
        if pending.insert(crossing.start_edge, crossing).is_some() {
            result.unlinked += 1;
        }
    }

    // Chains that start at a boundary have to be walked from their head, or an arbitrary
    // starting point would cut one broken contour into two.
    for head in heads(&pending) {
        take_contour(&mut pending, head, &mut result);
    }
    while let Some(start) = pending.keys().next().copied() {
        take_contour(&mut pending, start, &mut result);
    }
    result
}

/// Crossings nothing leads into: the open end of a chain.
fn heads(pending: &HashMap<EdgeKey, Crossing>) -> Vec<EdgeKey> {
    let reachable: HashSet<EdgeKey> = pending.values().map(|c| c.end_edge).collect();
    pending
        .keys()
        .filter(|edge| !reachable.contains(*edge))
        .copied()
        .collect()
}

fn take_contour(pending: &mut HashMap<EdgeKey, Crossing>, start: EdgeKey, result: &mut Stitched) {
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
fn walk(pending: &mut HashMap<EdgeKey, Crossing>, start: EdgeKey) -> (Vec<Vec2>, bool) {
    let mut points = Vec::new();
    let mut edge = start;
    let mut last_end = None;

    while let Some(crossing) = pending.remove(&edge) {
        points.push(crossing.start);
        last_end = Some(crossing.end);
        if crossing.end_edge == start {
            return (points, true);
        }
        edge = crossing.end_edge;
    }

    // The chain ran into an open or already-consumed edge. Keep the last point and let
    // the contour's implicit closing edge bridge the gap.
    if let Some(end) = last_end {
        points.push(end);
    }
    (points, false)
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

    #[test]
    fn a_second_crossing_on_one_edge_is_reported_as_unlinked() {
        let mut crossings = square();
        crossings.push(crossing((0, 0), (2, 2), Vec2::ZERO, Vec2::ONE));
        let stitched = stitch(crossings);

        assert_eq!(stitched.unlinked, 1);
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
