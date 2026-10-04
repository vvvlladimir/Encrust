use std::collections::HashMap;

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
/// once as an exit, so the chain is a permutation and every loop closes exactly. Where two
/// sheets of surface share an edge it is entered and left twice, and the chain takes the
/// sharpest left turn there, which keeps two loops touching at a point two loops.
pub(crate) fn stitch(crossings: Vec<Crossing>) -> Stitched {
    let mut result = Stitched::default();
    let mut pending: Pending = HashMap::with_capacity(crossings.len());
    for crossing in crossings {
        let leaving = pending.entry(crossing.start_edge).or_default();
        if !leaving.is_empty() {
            result.unlinked += 1;
        }
        leaving.push(crossing);
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

/// The crossings not yet chained, by the edge each one leaves from. Nearly every edge
/// holds one; an edge two sheets of surface share holds two.
type Pending = HashMap<EdgeKey, Vec<Crossing>>;

/// Edges more crossings leave from than arrive at, once for each one over: the open
/// ends of chains.
fn heads(pending: &Pending) -> Vec<EdgeKey> {
    let mut arriving: HashMap<EdgeKey, usize> = HashMap::new();
    for crossing in pending.values().flatten() {
        *arriving.entry(crossing.end_edge).or_default() += 1;
    }
    pending
        .iter()
        .flat_map(|(edge, leaving)| {
            let over = leaving
                .len()
                .saturating_sub(arriving.get(edge).copied().unwrap_or(0));
            std::iter::repeat_n(*edge, over)
        })
        .collect()
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
/// only one there is.
fn take(pending: &mut Pending, edge: EdgeKey, arriving: Option<&Crossing>) -> Option<Crossing> {
    let leaving = pending.get_mut(&edge)?;
    let index = match arriving {
        Some(arriving) if leaving.len() > 1 => {
            let heading = arriving.end - arriving.start;
            let turn = |crossing: &Crossing| {
                let next = crossing.end - crossing.start;
                heading.perp_dot(next).atan2(heading.dot(next))
            };
            (0..leaving.len())
                .max_by(|a, b| turn(&leaving[*a]).total_cmp(&turn(&leaving[*b])))
                .unwrap_or(0)
        }
        _ => 0,
    };
    let crossing = leaving.swap_remove(index);
    if leaving.is_empty() {
        pending.remove(&edge);
    }
    Some(crossing)
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
