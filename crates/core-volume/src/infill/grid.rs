use core_geometry::{Aabb, Mesh, Scalar, Vec2, Vec3, prism};

use super::{Bar, InfillSettings, Lattice, Rows, Shapes};

/// Square cells on the global lattice, their walls cut through at the middle of every side
/// by a round crossing hole along the wall's normal. All in millimetres.
pub(super) struct GridCells {
    pitch: Scalar,
    half_wall: Scalar,
    hole_radius: Scalar,
    rows: Rows,
}

impl GridCells {
    pub(super) fn new(settings: &InfillSettings, rows: Rows) -> Self {
        Self {
            pitch: settings.cell_mm + settings.wall_mm,
            half_wall: settings.wall_mm / 2.0,
            hole_radius: settings.hole_mm / 2.0,
            rows,
        }
    }
}

impl Lattice for GridCells {
    fn distance(&self, point: Vec3) -> Scalar {
        let pitch = self.pitch;
        let off_wall = |at: Scalar| at - pitch * (at / pitch).round();
        let off_middle = |at: Scalar| at - pitch * ((at / pitch).floor() + 0.5);
        let walls = off_wall(point.x).abs().min(off_wall(point.y).abs()) - self.half_wall;

        // A hole along x runs through the walls standing on x = k pitch halfway between
        // their junctions, and one along y through the walls across them.
        let dz = self.rows.off_row(point.z);
        let holes = off_middle(point.y)
            .hypot(dz)
            .min(off_middle(point.x).hypot(dz))
            - self.hole_radius;
        walls.max(-holes)
    }

    fn shapes(&self, bounds: &Aabb) -> Shapes {
        let (pitch, cell) = (self.pitch, self.pitch - 2.0 * self.half_wall);
        let steps = |axis: usize| {
            let first = (bounds.mins[axis] / pitch).floor() as i32 - 1;
            first..=(bounds.maxs[axis] / pitch).ceil() as i32
        };
        let (bottom, top) = (bounds.mins.z, bounds.maxs.z);

        let mut cells = Mesh::default();
        for i in steps(0) {
            for j in steps(1) {
                let center = Vec2::new(i as Scalar + 0.5, j as Scalar + 0.5) * pitch;
                let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .map(|(x, y)| center + Vec2::new(x, y) * cell / 2.0);
                cells.append(&prism(&corners, bottom, top));
            }
        }

        // A hole spans the wall from face to face, so it and the cells either side add up
        // to the void without counting any of it twice.
        let reach = self.half_wall;
        let rows: Vec<Scalar> = self
            .rows
            .between(bottom - self.hole_radius, top + self.hole_radius)
            .collect();
        let holes = (0..2)
            .map(|axis| {
                let mut along = Vec::new();
                for wall in steps(axis) {
                    for middle in steps(1 - axis) {
                        for z in &rows {
                            let mut at = Vec3::new(0.0, 0.0, *z);
                            at[axis] = wall as Scalar * pitch;
                            at[1 - axis] = (middle as Scalar + 0.5) * pitch;
                            let mut off = Vec3::ZERO;
                            off[axis] = reach;
                            along.push(Bar {
                                from: at - off,
                                to: at + off,
                                radius: self.hole_radius,
                            });
                        }
                    }
                }
                along
            })
            .collect();
        Shapes::Openings { cells, holes }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::solid_share;
    use super::*;
    use crate::infill::InfillPattern;

    /// 6 mm cells and 2 mm walls, an 8 mm pitch, with 3 mm holes from z = 4 every 8 mm.
    fn cells() -> GridCells {
        let settings = InfillSettings {
            pattern: InfillPattern::Grid,
            ..InfillSettings::default()
        };
        GridCells::new(
            &settings,
            Rows {
                first: 4.0,
                spacing: 8.0,
            },
        )
    }

    #[test]
    fn a_grid_under_its_holes_fills_what_its_walls_cover() {
        // A square pitch c with walls t is solid over 1 - ((c - t) / c)^2: 7/16 at 8 and 2.
        let share = solid_share(&cells(), Vec3::ZERO, Vec3::new(16.0, 16.0, 1.0), 0.05);
        assert!(
            (share - 7.0 / 16.0).abs() < 0.01,
            "a grid of 8 mm pitch and 2 mm walls is 7/16 solid, got {share}"
        );
    }

    #[test]
    fn a_crossing_hole_opens_the_middle_of_a_wall_at_its_row() {
        let cells = cells();
        assert!(
            cells.distance(Vec3::new(0.0, 4.0, 4.0)) > 0.0,
            "a hole runs through it"
        );
        assert!(
            cells.distance(Vec3::new(4.0, 0.0, 12.0)) > 0.0,
            "and the row above it"
        );
        assert!(
            cells.distance(Vec3::new(0.0, 4.0, 1.0)) < 0.0,
            "under the first row the wall is whole"
        );
        assert!(
            cells.distance(Vec3::new(0.0, 0.0, 4.0)) < 0.0,
            "a junction keeps its post"
        );
    }
}
