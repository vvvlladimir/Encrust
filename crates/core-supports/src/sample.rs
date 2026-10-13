use std::collections::HashMap;

use core_geometry::Vec2;

use crate::base::hull;
use crate::field::Field;

/// How near a point inside a piece may stand to one already taken, as a share of the
/// spacing: the radius a standing support holds over, see [`crate::config::SampleConfig`].
const FILL_REACH: (i32, i32) = (3, 4);

/// How many steps of the grid that fills the lattice's gaps make one spacing.
const FILL_STEPS: i32 = 4;

/// How far the outline has to turn at a point for it to be a corner, as a cosine: 45
/// degrees, which a square has and a circle wider than a few arms does not.
const CORNER_TURN_COS: f32 = 0.707;

/// How far apart the supports that steady a part stand, as a share of how far across it
/// is: three on a round rim are 0.87 of it apart, so 0.8 keeps three and refuses a fourth.
const STANCE_REACH: (i32, i32) = (4, 5);

/// Where supports go on one piece of unsupported layer, as cells of the grid it was read
/// on.
///
/// `room` is where on that layer a head stands without reaching over the layer's outline,
/// and `inset` is how far in from the outline that is, in cells. A head on the outline
/// itself is half on the wall above it, so the piece is sampled where it lies inside
/// `room`, and on its own cells only where it is too narrow to have any room near it.
///
/// The outline goes first, corners before the rest of it: a corner is the first place a
/// ledge lifts and the edge is where the peel starts, so neither may depend on where a
/// lattice happened to fall. What is left inside is filled from a lattice anchored to the
/// grid's own origin rather than to the piece, so the same place comes up on every layer
/// a slope passes through and the coverage check thins the run to one support per place.
///
/// A piece too small to hold two supports is all corner, so it still comes back with one:
/// this never returns nothing for a piece that has cells.
pub fn sample_piece(piece: &Field, room: &Field, inset: i32, spacing: i32) -> Vec<[i32; 2]> {
    let spacing = spacing.max(1);
    let (clear, narrow) = standing_room(piece, room, inset);
    let mut taken = Thinning::new(spacing);
    let mut points = Vec::new();

    for part in [&clear, &narrow] {
        let rim = part.rim();
        for point in corners(&rim, spacing).into_iter().chain(cells_of(&rim)) {
            if taken.take(point, spacing) {
                points.push(point);
            }
        }
    }

    let fill = (spacing * FILL_REACH.0 / FILL_REACH.1).max(1);
    // The staggered lattice first, then one of a quarter of its step into the gaps the
    // outline's supports opened in it: fine enough that no place is left a spacing away.
    for part in [&clear, &narrow] {
        let inside = lattice_of(part, spacing)
            .into_iter()
            .chain(fill_of(part, spacing));
        for point in inside {
            if taken.take(point, fill) {
                points.push(point);
            }
        }
    }
    points
}

/// Where the supports that steady one part go: a few on its outline, as far apart as it
/// is across, so that a part held at one place is held at three. The outline is taken
/// `inset` cells in, where the part is wide enough to have one there.
pub fn sample_stance(piece: &Field, inset: i32) -> Vec<[i32; 2]> {
    let (clear, narrow) = standing_room(piece, &piece.shrunk(inset), inset);
    let outline = clear.rim().with(&narrow.rim());
    let Some((min, max)) = outline.bounds() else {
        return Vec::new();
    };
    let across = (max[0] - min[0] + 1).max(max[1] - min[1] + 1);
    let reach = (across * STANCE_REACH.0 / STANCE_REACH.1).max(1);
    let mut taken = Thinning::new(reach);
    cells_of(&outline)
        .filter(|point| taken.take(*point, reach))
        .collect()
}

/// The cells of `piece` inside `room`, and the cells of it further than a head's width
/// from any of those: what is too narrow for a head to stand clear of the outline.
fn standing_room(piece: &Field, room: &Field, inset: i32) -> (Field, Field) {
    if inset <= 0 {
        return (piece.clone(), Field::default());
    }
    let clear = piece.without(&piece.without(room));
    // Twice the inset, because a square corner is 1.41 of it from the room inside it and
    // is not a narrow place.
    let narrow = piece.uncarried_by(&clear, 2 * inset);
    (clear, narrow)
}

/// The cells taken so far, in buckets one spacing across, so that asking whether a cell
/// is too close to one of them reads nine buckets rather than the whole list.
struct Thinning {
    spacing: i32,
    buckets: HashMap<[i32; 2], Vec<[i32; 2]>>,
}

impl Thinning {
    fn new(spacing: i32) -> Self {
        Self {
            spacing,
            buckets: HashMap::new(),
        }
    }

    /// Takes `point` unless it is inside `reach` of anything already taken, and answers
    /// whether it did. `reach` is no more than the spacing the buckets were made for.
    fn take(&mut self, point: [i32; 2], reach: i32) -> bool {
        if self.crowded(point, reach) {
            return false;
        }
        self.buckets
            .entry(self.bucket_of(point))
            .or_default()
            .push(point);
        true
    }

    fn crowded(&self, point: [i32; 2], reach: i32) -> bool {
        let home = self.bucket_of(point);
        let reach = i64::from(reach) * i64::from(reach);
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                self.buckets
                    .get(&[home[0] + dx, home[1] + dy])
                    .is_some_and(|taken| {
                        taken.iter().any(|other| {
                            let dx = i64::from(other[0] - point[0]);
                            let dy = i64::from(other[1] - point[1]);
                            dx * dx + dy * dy < reach
                        })
                    })
            })
        })
    }

    fn bucket_of(&self, point: [i32; 2]) -> [i32; 2] {
        [
            point[0].div_euclid(self.spacing),
            point[1].div_euclid(self.spacing),
        ]
    }
}

/// Every cell of `field`, bottom row first and left to right along it.
fn cells_of(field: &Field) -> impl Iterator<Item = [i32; 2]> + '_ {
    field.rows().flat_map(|(y, spans)| {
        spans
            .iter()
            .flat_map(move |span| (span.x0..span.x1).map(move |x| [x, y]))
    })
}

/// The lattice points inside the piece. Every other row is offset half a step, which
/// covers the same ground with fewer supports than a square grid does.
fn lattice_of(piece: &Field, spacing: i32) -> Vec<[i32; 2]> {
    let mut points = Vec::new();
    for (y, spans) in piece.rows() {
        if y.rem_euclid(spacing) != 0 {
            continue;
        }
        let shift = if (y / spacing).rem_euclid(2) == 0 {
            0
        } else {
            spacing / 2
        };
        for span in spans {
            let first = (span.x0 - shift).div_euclid(spacing) * spacing + shift;
            let mut x = if first < span.x0 {
                first + spacing
            } else {
                first
            };
            while x < span.x1 {
                points.push([x, y]);
                x += spacing;
            }
        }
    }
    points
}

/// The cells inside the piece on a square grid of a quarter of the spacing, anchored to
/// the grid's origin like the lattice it fills the gaps of.
fn fill_of(piece: &Field, spacing: i32) -> Vec<[i32; 2]> {
    // Every fourth one falls on the lattice's own step, whatever the spacing divides by.
    let quarter = |count: i32| (count * spacing).div_euclid(FILL_STEPS);
    let first_from = |cell: i32| {
        let count = (cell * FILL_STEPS).div_euclid(spacing);
        if quarter(count) < cell {
            count + 1
        } else {
            count
        }
    };

    let mut points = Vec::new();
    for (y, spans) in piece.rows() {
        if quarter(first_from(y)) != y {
            continue;
        }
        for span in spans {
            let mut count = first_from(span.x0);
            while quarter(count) < span.x1 {
                points.push([quarter(count), y]);
                count += 1;
            }
        }
    }
    points
}

/// The corners of an outline: the cells of its convex hull where the hull turns at least
/// 45 degrees between one arm back and one arm on, an arm being a quarter of the spacing.
///
/// Read off the hull because a piece is cells, not a polygon: a hull has a direction to
/// turn from, and a ring or a round rim turns too little anywhere on it to have a corner.
fn corners(rim: &Field, spacing: i32) -> Vec<[i32; 2]> {
    let ends: Vec<Vec2> = rim
        .rows()
        .filter_map(|(y, spans)| Some((y, spans.first()?.x0, spans.last()?.x1 - 1)))
        .flat_map(|(y, low, high)| {
            [
                Vec2::new(low as f32, y as f32),
                Vec2::new(high as f32, y as f32),
            ]
        })
        .collect();
    // A hull of points on one line comes back as their bounding box, whose corners are
    // only cells of the outline where they are its two ends.
    let hull: Vec<Vec2> = hull(&ends)
        .into_iter()
        .map(Vec2::round)
        .filter(|point| ends.contains(point))
        .collect();

    let arm = (spacing / 4).max(3) as f32;
    (0..hull.len())
        .filter(|&at| {
            let before = hull[at] - arm_end(&hull, at, arm, hull.len() - 1);
            let after = arm_end(&hull, at, arm, 1) - hull[at];
            before.dot(after) <= CORNER_TURN_COS * before.length() * after.length()
        })
        .map(|at| [hull[at].x as i32, hull[at].y as i32])
        .collect()
}

/// The hull vertex at least `arm` away from `hull[at]`, walking `step` vertices at a time
/// round it; the furthest one reached when the whole hull is shorter than that.
fn arm_end(hull: &[Vec2], at: usize, arm: f32, step: usize) -> Vec2 {
    let mut end = at;
    for _ in 1..hull.len() {
        end = (end + step) % hull.len();
        if (hull[end] - hull[at]).length() >= arm {
            break;
        }
    }
    hull[end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GRID_PITCH_MM;
    use crate::field::Grid;
    use core_geometry::Scalar;
    use core_slicer::{Contour, Layer};

    fn grid() -> Grid {
        Grid::covering(Vec2::ZERO, Vec2::splat(60.0), GRID_PITCH_MM)
    }

    fn rectangle(x: Scalar, y: Scalar, width: Scalar, height: Scalar) -> Field {
        let points = vec![
            Vec2::new(x, y),
            Vec2::new(x + width, y),
            Vec2::new(x + width, y + height),
            Vec2::new(x, y + height),
        ];
        let layer = Layer {
            z: 1.0,
            contours: vec![Contour::from_points(points).expect("a rectangle encloses area")],
            extra: Vec::new(),
        };
        Field::of_layer(&layer, &grid())
    }

    fn circle(centre: Vec2, radius: Scalar) -> Field {
        let points = (0..96)
            .map(|side| {
                let angle = std::f32::consts::TAU * side as Scalar / 96.0;
                centre + Vec2::new(angle.cos(), angle.sin()) * radius
            })
            .collect();
        let layer = Layer {
            z: 1.0,
            contours: vec![Contour::from_points(points).expect("a circle encloses area")],
            extra: Vec::new(),
        };
        Field::of_layer(&layer, &grid())
    }

    /// Fifty cells of a tenth of a millimetre: the spacing the medium preset asks for.
    const SPACING: i32 = 50;

    /// Three cells: a head 0.4 mm across and the cell it keeps clear of the outline.
    const INSET: i32 = 3;

    /// `piece` sampled as a whole layer of its own, so its outline is the layer's.
    fn sampled(piece: &Field, spacing: i32) -> Vec<[i32; 2]> {
        sample_piece(piece, &piece.shrunk(INSET), INSET, spacing)
    }

    #[test]
    fn a_wide_piece_is_covered_at_the_spacing_it_was_given() {
        let piece = rectangle(10.0, 10.0, 20.0, 20.0);
        let points = sampled(&piece, SPACING);

        assert!(points.len() > 8, "a 400 mm2 piece needs a lattice");
        for point in &points {
            assert!(
                piece.contains(point[0], point[1]),
                "{point:?} fell outside the piece"
            );
        }

        // Nowhere inside is further from a support than the lattice allows.
        let (min, max) = piece.bounds().expect("the piece has cells");
        for y in (min[1]..=max[1]).step_by(7) {
            for x in (min[0]..=max[0]).step_by(7) {
                if !piece.contains(x, y) {
                    continue;
                }
                let nearest = points
                    .iter()
                    .map(|point| {
                        let dx = (point[0] - x) as f32;
                        let dy = (point[1] - y) as f32;
                        dx.hypot(dy)
                    })
                    .fold(f32::INFINITY, f32::min);
                // The lattice covers the inside and the walk covers the rim, each to
                // within its own spacing; where the two meet, a rim point suppressed by
                // a lattice point can leave half a step more.
                assert!(
                    nearest <= 1.5 * SPACING as f32,
                    "cell {x},{y} is {nearest} cells from the nearest support"
                );
            }
        }
    }

    #[test]
    fn a_sliver_too_thin_for_the_lattice_still_gets_a_line_of_supports() {
        let piece = rectangle(10.0, 10.02, 30.0, 0.2);
        let points = sampled(&piece, SPACING);

        assert!(points.len() >= 5, "a 30 mm sliver needs a row of supports");
        for point in &points {
            assert!(piece.contains(point[0], point[1]), "{point:?} fell outside");
        }
        for (index, point) in points.iter().enumerate() {
            for other in &points[index + 1..] {
                let dx = (point[0] - other[0]) as f32;
                let dy = (point[1] - other[1]) as f32;
                assert!(
                    dx.hypot(dy) >= SPACING as f32 - 1.0,
                    "two supports landed on top of each other"
                );
            }
        }
    }

    #[test]
    fn every_corner_of_a_ledge_gets_a_support_wherever_the_lattice_falls() {
        // Off the lattice on every side, so no corner has a lattice point to lean on.
        let piece = rectangle(1.3, 2.7, 17.0, 13.0);
        let points = sampled(&piece, SPACING);
        let (min, max) = piece.bounds().expect("the piece has cells");

        for corner in [
            [min[0] + INSET, min[1] + INSET],
            [max[0] - INSET, min[1] + INSET],
            [max[0] - INSET, max[1] - INSET],
            [min[0] + INSET, max[1] - INSET],
        ] {
            assert!(
                points.contains(&corner),
                "a corner is the first place a ledge lifts, and {corner:?} has no support"
            );
        }
    }

    #[test]
    fn no_support_of_a_wide_piece_stands_nearer_its_outline_than_a_head_reaches() {
        let piece = rectangle(1.3, 2.7, 17.0, 13.0);
        let (min, max) = piece.bounds().expect("the piece has cells");

        for point in sampled(&piece, SPACING) {
            let room = (point[0] - min[0])
                .min(max[0] - point[0])
                .min(point[1] - min[1])
                .min(max[1] - point[1]);
            assert!(
                room >= INSET,
                "{point:?} is {room} cells from the outline and the head reaches {INSET}"
            );
        }
    }

    #[test]
    fn an_arm_too_narrow_for_a_head_is_still_held_along_its_length() {
        // A 10 mm square with a 0.3 mm arm running 20 mm out of its side.
        let piece = rectangle(10.0, 10.0, 10.0, 10.0).with(&rectangle(20.0, 15.0, 20.0, 0.3));
        let points = sampled(&piece, SPACING);

        let far = points.iter().filter(|point| point[0] > 300).count();
        assert!(
            far >= 1,
            "the arm ends 20 mm from the square and nothing stands in its last 10 mm"
        );
    }

    #[test]
    fn a_ledge_is_inset_from_the_outline_and_not_from_what_carries_it() {
        // The right half of a layer hangs over nothing; its left side joins the half
        // that is carried, which is no edge of the layer.
        let layer = rectangle(10.0, 10.0, 20.0, 10.0);
        let piece = rectangle(20.0, 10.0, 10.0, 10.0);
        let (min, _) = piece.bounds().expect("the piece has cells");

        let points = sample_piece(&piece, &layer.shrunk(INSET), INSET, SPACING);
        assert!(
            points.iter().any(|point| point[0] == min[0]),
            "the side a ledge is carried along has material on both sides of it"
        );
    }

    #[test]
    fn a_round_piece_has_no_corners_to_crowd_its_rim() {
        let rim = circle(Vec2::new(30.0, 30.0), 15.0).rim();
        assert!(
            corners(&rim, SPACING).is_empty(),
            "a 30 mm circle turns 4 degrees over a millimetre of rim"
        );
    }

    #[test]
    fn a_part_held_at_one_place_is_steadied_at_three() {
        let points = sample_stance(&circle(Vec2::new(30.0, 30.0), 3.0), INSET);
        assert_eq!(
            points.len(),
            3,
            "three points 0.8 of a circle apart fit round it and a fourth does not"
        );
    }

    #[test]
    fn a_speck_gets_exactly_one_support() {
        let piece = rectangle(10.0, 10.0, 0.4, 0.4);
        let points = sampled(&piece, SPACING);
        assert_eq!(points.len(), 1);
        assert!(piece.contains(points[0][0], points[0][1]));
    }

    #[test]
    fn the_lattice_does_not_move_when_the_piece_does() {
        // Two steps, because every other row of the lattice is offset half a step: a
        // piece moved by an odd number of steps meets the other half of it.
        let here = sampled(&rectangle(10.0, 10.0, 20.0, 20.0), SPACING);
        let there = sampled(&rectangle(20.0, 20.0, 20.0, 20.0), SPACING);
        assert_eq!(
            here.len(),
            there.len(),
            "the lattice is anchored to the plate, so a whole number of steps changes nothing"
        );
    }

    #[test]
    fn a_denser_spacing_puts_down_more_supports() {
        let piece = rectangle(10.0, 10.0, 20.0, 20.0);
        let sparse = sampled(&piece, SPACING);
        let dense = sampled(&piece, SPACING / 2);
        assert!(dense.len() > sparse.len());
    }

    #[test]
    fn an_empty_piece_is_sampled_as_nothing() {
        assert!(sampled(&Field::default(), SPACING).is_empty());
    }
}
