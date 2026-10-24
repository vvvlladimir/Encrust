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

/// Where supports go on one unsupported piece, as cells: corners, outline, then a grid-anchored
/// lattice so layers agree; `room` is `inset` cells in, a narrow piece uses its own; never empty.
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
    if points.len() == 1 {
        let room = if clear.is_empty() { &narrow } else { &clear };
        return middle_of(room).into_iter().collect();
    }
    points
}

/// The cell of `field` nearest the mean of its cells, or `None` for an empty one.
fn middle_of(field: &Field) -> Option<[i32; 2]> {
    let (mut sum, mut count) = ([0i64; 2], 0i64);
    for [x, y] in cells_of(field) {
        sum[0] += i64::from(x);
        sum[1] += i64::from(y);
        count += 1;
    }
    let mean = sum.map(|total| total as f32 / count.max(1) as f32);
    let distance = |[x, y]: [i32; 2]| (x as f32 - mean[0]).powi(2) + (y as f32 - mean[1]).powi(2);
    cells_of(field).min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
}

/// Where the supports that steady one part go: a few on its outline, as far apart as it is
/// across, `inset` cells in where the part is wide enough, so one place held is held at three.
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

/// The corners of an outline: cells of its convex hull turning at least 45 degrees between arms
/// of a quarter of the spacing; a hull, since a ring or round rim turns too little for a corner.
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
mod tests;
