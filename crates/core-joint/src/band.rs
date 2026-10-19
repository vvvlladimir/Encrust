use core_geometry::{FastMap, Mesh, Scalar, Vec2, extrude};

use crate::section::Section;

/// How close to a cell's corner an edge crossing may come, as a share of the cell: nearer
/// would leave a sliver triangle the boolean has to read.
const NEAREST_CORNER: Scalar = 0.02;

/// A point of the outline, named by what it is on the grid so neighbouring cells share it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GridPoint {
    Corner(usize, usize),
    /// On the edge from a corner to the one to its right or above it.
    Right(usize, usize),
    Above(usize, usize),
    Middle(usize, usize),
}

/// The region of `section` where `field` is positive, stood up from `bottom_mm` to
/// `top_mm` along the plane's normal into a closed mesh, wound outward.
///
/// The region is traced cell by cell by marching squares over `field` at the cell
/// centres; see `docs/design/joints.md`.
pub(crate) fn band(section: &Section, field: &[Scalar], bottom_mm: Scalar, top_mm: Scalar) -> Mesh {
    let mut plan = Plan::default();
    for y in 0..section.height.saturating_sub(1) {
        for x in 0..section.width.saturating_sub(1) {
            plan.cell(section, field, x, y);
        }
    }
    plan.stood(section, bottom_mm, top_mm)
}

/// The region traced so far: its points, in the frame, and its triangles, anticlockwise.
#[derive(Debug, Default)]
struct Plan {
    index: FastMap<GridPoint, u32>,
    points: Vec<Vec2>,
    triangles: Vec<[u32; 3]>,
}

impl Plan {
    fn point(&mut self, key: GridPoint, at: Vec2) -> u32 {
        *self.index.entry(key).or_insert_with(|| {
            self.points.push(at);
            self.points.len() as u32 - 1
        })
    }

    /// What of the cell from corner (`x`, `y`) to (`x + 1`, `y + 1`) the field holds.
    fn cell(&mut self, section: &Section, field: &[Scalar], x: usize, y: usize) {
        let corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
        let value = |(cx, cy): (usize, usize)| field[cy * section.width + cx];
        let inside = corners.map(|corner| value(corner) > 0.0);
        if inside.iter().all(|is| !is) {
            return;
        }
        let at = |(cx, cy): (usize, usize)| section.point(cy * section.width + cx);
        let mut outline = Vec::with_capacity(6);
        let mut owner = Vec::with_capacity(6);
        for k in 0..4 {
            let (from, to) = (corners[k], corners[(k + 1) % 4]);
            if inside[k] {
                outline.push(self.point(GridPoint::Corner(from.0, from.1), at(from)));
                owner.push(Some(k));
            }
            if inside[k] != inside[(k + 1) % 4] {
                let t = (value(from) / (value(from) - value(to)))
                    .clamp(NEAREST_CORNER, 1.0 - NEAREST_CORNER);
                let point = at(from).lerp(at(to), t);
                outline.push(self.point(edge_key(from, to), point));
                owner.push(None);
            }
        }
        let saddle = inside[0] == inside[2] && inside[1] == inside[3] && inside[0] != inside[1];
        if !saddle {
            for k in 1..outline.len() - 1 {
                self.triangles
                    .push([outline[0], outline[k], outline[k + 1]]);
            }
            return;
        }
        let middle_value = corners.iter().map(|corner| value(*corner)).sum::<Scalar>() / 4.0;
        if middle_value > 0.0 {
            let middle = self.point(
                GridPoint::Middle(x, y),
                (at(corners[0]) + at(corners[2])) / 2.0,
            );
            for k in 0..outline.len() {
                self.triangles
                    .push([middle, outline[k], outline[(k + 1) % outline.len()]]);
            }
            return;
        }
        // The two inside corners stand apart: each keeps the corner of the cell it is in.
        let n = outline.len();
        for (k, corner) in owner.iter().enumerate() {
            if corner.is_some() {
                self.triangles
                    .push([outline[k], outline[(k + 1) % n], outline[(k + n - 1) % n]]);
            }
        }
    }

    /// The traced region as a slab: its triangles at both heights and a wall round every
    /// edge only one triangle has.
    fn stood(self, section: &Section, bottom_mm: Scalar, top_mm: Scalar) -> Mesh {
        let frame = section.frame;
        let count = self.points.len();
        extrude(
            &self.points,
            &self.triangles,
            |point, height| frame.lift(point) + frame.normal * height,
            &vec![bottom_mm; count],
            &vec![top_mm; count],
        )
    }
}

fn edge_key(from: (usize, usize), to: (usize, usize)) -> GridPoint {
    let (low, high) = if from <= to { (from, to) } else { (to, from) };
    if low.1 == high.1 {
        GridPoint::Right(low.0, low.1)
    } else {
        GridPoint::Above(low.0, low.1)
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Plane, Vec3, cuboid, cut, diagnose, signed_volume};

    use super::*;

    #[test]
    fn a_rim_inset_from_a_square_face_is_a_closed_frame_of_its_area() {
        let plane = Plane::at(Vec3::Z, 5.0).expect("+Z is a direction");
        let halves = cut(&cuboid(Vec3::ZERO, Vec3::splat(20.0)), plane);
        let section = Section::of(&halves, plane, 0.1).expect("the cut crosses the box");
        // From 1 mm to 3 mm in from the edge: 18 mm square less 14 mm square.
        let field: Vec<Scalar> = section
            .distance
            .iter()
            .map(|distance| (distance - 1.0).min(3.0 - distance))
            .collect();
        let rim = band(&section, &field, 0.0, 2.0);
        let report = diagnose(&rim);
        assert_eq!(report.boundary_edges, 0, "the rim is open: {report:?}");
        let volume = signed_volume(&rim);
        let expected = (18.0 * 18.0 - 14.0 * 14.0) * 2.0;
        assert!(
            (volume - expected).abs() / expected < 0.03,
            "(18² - 14²) × 2 mm is {expected}, got {volume}"
        );
    }
}
