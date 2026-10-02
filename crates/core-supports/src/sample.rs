use std::collections::HashMap;

use crate::field::Field;

/// Where supports go on one piece of unsupported layer, as cells of the grid it was read
/// on.
///
/// A lattice of `spacing` cells, anchored to the grid's own origin rather than to the
/// piece, holds anything with room in it: the same lattice point comes up on every layer
/// a slope passes through, which is what lets the coverage check thin the run down to one
/// support per place rather than one per layer.
///
/// The outline is walked as well as filled, because a lattice leaves its widest gap
/// exactly where the piece ends, and the edge of an overhang is where the peel starts.
/// A piece too small or too thin to catch a lattice point is all outline, so it still
/// comes back with a support: this never returns nothing for a piece that has cells.
pub fn sample_piece(piece: &Field, spacing: i32) -> Vec<[i32; 2]> {
    let spacing = spacing.max(1);
    let mut points = lattice_of(piece, spacing);

    // The lattice is what a rim point has to keep clear of; the rim points then keep
    // clear of each other as they are walked.
    let mut taken = Thinning::new(spacing);
    for point in &points {
        taken.add(*point);
    }
    for point in walk(&piece.rim(), spacing) {
        // A rim point on top of a lattice point holds nothing the lattice does not.
        if !taken.crowded(point) {
            taken.add(point);
            points.push(point);
        }
    }
    points
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

    /// Whether `point` is inside `spacing` of anything already taken.
    fn crowded(&self, point: [i32; 2]) -> bool {
        let home = self.bucket_of(point);
        let reach = i64::from(self.spacing) * i64::from(self.spacing);
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

    fn add(&mut self, point: [i32; 2]) {
        self.buckets
            .entry(self.bucket_of(point))
            .or_default()
            .push(point);
    }

    fn bucket_of(&self, point: [i32; 2]) -> [i32; 2] {
        [
            point[0].div_euclid(self.spacing),
            point[1].div_euclid(self.spacing),
        ]
    }
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

/// Cells along every span, `spacing` apart and including each span's far end, thinned so
/// that nothing lands within `spacing` of a cell already taken.
///
/// Walking the ends as well as the steps is what puts a support in the corner of a ledge,
/// which is the first place a corner lifts.
fn walk(piece: &Field, spacing: i32) -> Vec<[i32; 2]> {
    let mut taken = Thinning::new(spacing);
    let mut kept: Vec<[i32; 2]> = Vec::new();
    for (y, spans) in piece.rows() {
        for span in spans {
            let mut x = span.x0;
            loop {
                let at = [x.min(span.x1 - 1), y];
                if !taken.crowded(at) {
                    taken.add(at);
                    kept.push(at);
                }
                if x >= span.x1 - 1 {
                    break;
                }
                x += spacing;
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GRID_PITCH_MM;
    use crate::field::Grid;
    use core_geometry::{Scalar, Vec2};
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

    /// Fifty cells of a tenth of a millimetre: the spacing the medium preset asks for.
    const SPACING: i32 = 50;

    #[test]
    fn a_wide_piece_is_covered_at_the_spacing_it_was_given() {
        let piece = rectangle(10.0, 10.0, 20.0, 20.0);
        let points = sample_piece(&piece, SPACING);

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
        let points = sample_piece(&piece, SPACING);

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
    fn a_speck_gets_exactly_one_support() {
        let piece = rectangle(10.0, 10.0, 0.4, 0.4);
        let points = sample_piece(&piece, SPACING);
        assert_eq!(points.len(), 1);
        assert!(piece.contains(points[0][0], points[0][1]));
    }

    #[test]
    fn the_lattice_does_not_move_when_the_piece_does() {
        // Two steps, because every other row of the lattice is offset half a step: a
        // piece moved by an odd number of steps meets the other half of it.
        let here = sample_piece(&rectangle(10.0, 10.0, 20.0, 20.0), SPACING);
        let there = sample_piece(&rectangle(20.0, 20.0, 20.0, 20.0), SPACING);
        assert_eq!(
            here.len(),
            there.len(),
            "the lattice is anchored to the plate, so a whole number of steps changes nothing"
        );
    }

    #[test]
    fn a_denser_spacing_puts_down_more_supports() {
        let piece = rectangle(10.0, 10.0, 20.0, 20.0);
        let sparse = sample_piece(&piece, SPACING);
        let dense = sample_piece(&piece, SPACING / 2);
        assert!(dense.len() > sparse.len());
    }

    #[test]
    fn an_empty_piece_is_sampled_as_nothing() {
        assert!(sample_piece(&Field::default(), SPACING).is_empty());
    }
}
