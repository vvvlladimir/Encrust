use core_geometry::{Scalar, Vec2};

use crate::section::Section;

/// Spots one piece gives up on after this many have been turned down: the piece is
/// too thin through for a socket nearly everywhere, and trying every cell is not worth it.
const MOST_REFUSALS: usize = 64;

/// What a spot must have round it, mm.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Room {
    /// From its centre to the edge of the face.
    pub reach_mm: Scalar,
    /// From its centre to the centre of any other spot.
    pub gap_mm: Scalar,
}

/// Up to `count` spots on one piece of `section`, made of `cells`, each with the `room`
/// it needs and taken by `fits`, spread as far apart as the piece allows.
///
/// One spot goes where the piece is widest. More start from the two ends of the piece
/// and add the cell furthest from those already taken; see `docs/design/joints.md`.
pub(crate) fn spread<T>(
    section: &Section,
    cells: &[usize],
    count: usize,
    room: Room,
    mut fits: impl FnMut(Vec2) -> Option<T>,
) -> Vec<(Vec2, T)> {
    let mut open: Vec<Candidate> = cells
        .iter()
        .filter(|cell| section.distance[**cell] >= room.reach_mm)
        .map(|cell| Candidate {
            point: section.point(*cell),
            depth: section.distance[*cell],
            gap: Scalar::MAX,
        })
        .collect();
    let mut taken = Vec::new();
    let mut refusals = 0;
    while taken.len() < count && refusals < MOST_REFUSALS {
        let Some(best) = next(&open, taken.len(), count) else {
            break;
        };
        let point = open[best].point;
        if let Some(fit) = fits(point) {
            taken.push((point, fit));
            for candidate in &mut open {
                candidate.gap = candidate.gap.min(candidate.point.distance(point));
            }
            open.retain(|candidate| candidate.gap >= room.gap_mm);
        } else {
            refusals += 1;
            let near = room.reach_mm / 2.0;
            open.retain(|candidate| candidate.point.distance(point) > near);
        }
    }
    taken
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    point: Vec2,
    /// From the edge of the face, mm.
    depth: Scalar,
    /// From the nearest spot taken, mm.
    gap: Scalar,
}

/// The candidate to try next: the widest place for a single spot, the furthest from
/// the middle for the first of several, then the furthest from those taken.
fn next(open: &[Candidate], taken: usize, count: usize) -> Option<usize> {
    let best = |score: &dyn Fn(&Candidate) -> Scalar| {
        open.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| score(a).total_cmp(&score(b)))
            .map(|(index, _)| index)
    };
    match (taken, count) {
        (0, 1) => best(&|candidate| candidate.depth),
        (0, _) => {
            let middle = open.iter().map(|candidate| candidate.point).sum::<Vec2>()
                / open.len().max(1) as Scalar;
            best(&|candidate| candidate.point.distance(middle))
        }
        _ => best(&|candidate| candidate.gap),
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Plane, Vec3, cuboid, cut};

    use super::*;

    fn long_face() -> Section {
        let plane = Plane::at(Vec3::Z, 5.0).expect("+Z is a direction");
        let half = cut(&cuboid(Vec3::ZERO, Vec3::new(40.0, 10.0, 10.0)), plane).below;
        Section::of(&half, plane, 0.25).expect("the cut crosses the box")
    }

    fn room() -> Room {
        Room {
            reach_mm: 3.0,
            gap_mm: 6.0,
        }
    }

    #[test]
    fn two_spots_stand_at_the_two_ends_of_a_long_face() {
        let section = long_face();
        let cells = &section.cells_by_piece()[0];
        let spots = spread(&section, cells, 2, room(), |_| Some(()));
        assert_eq!(spots.len(), 2);
        let apart = spots[0].0.distance(spots[1].0);
        assert!(
            apart > 30.0,
            "a 40 mm face less 3 mm at each end leaves 34 mm, got {apart}"
        );
        for (point, ()) in &spots {
            assert!(
                section.distance_at(*point) >= 3.0 - section.cell_mm,
                "each keeps its reach from the edge"
            );
        }
    }

    #[test]
    fn a_spot_turned_down_is_replaced_by_one_elsewhere() {
        let section = long_face();
        let cells = &section.cells_by_piece()[0];
        let frame = section.frame;
        // Everything left of the middle is too shallow, say, for a socket.
        let spots = spread(&section, cells, 1, room(), |point| {
            (frame.lift(point).x > 20.0).then_some(())
        });
        assert_eq!(spots.len(), 1);
        assert!(frame.lift(spots[0].0).x > 20.0);
    }

    #[test]
    fn a_face_narrower_than_the_reach_takes_no_spot() {
        let section = long_face();
        let cells = &section.cells_by_piece()[0];
        let wide = Room {
            reach_mm: 6.0,
            gap_mm: 12.0,
        };
        assert!(spread(&section, cells, 2, wide, |_| Some(())).is_empty());
    }
}
