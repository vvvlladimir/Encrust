use core_geometry::{Aabb, Mesh, Scalar, Vec2, Vec3, prism};

use super::{Bar, InfillSettings, Lattice, Rows, Shapes};

/// The three directions from a cell to its neighbours, each the normal of two of its sides.
const NORMALS: [Vec2; 3] = [
    Vec2::new(1.0, 0.0),
    Vec2::new(0.5, 0.866_025_4),
    Vec2::new(-0.5, 0.866_025_4),
];

/// Hexagonal cells on a triangular lattice anchored at the origin, joined to each of their
/// six neighbours by a round crossing hole along the line between their centres. All in
/// millimetres.
pub(super) struct HiveCells {
    /// From one cell's centre to the next: the hexagon across its flats, plus a wall.
    pitch: Scalar,
    inradius: Scalar,
    /// From a cell's centre to its corners.
    circumradius: Scalar,
    hole_radius: Scalar,
    rows: Rows,
}

impl HiveCells {
    pub(super) fn new(settings: &InfillSettings, rows: Rows) -> Self {
        let inradius = settings.cell_mm * (3.0 as Scalar).sqrt() / 4.0;
        Self {
            pitch: 2.0 * inradius + settings.wall_mm(),
            inradius,
            circumradius: settings.cell_mm / 2.0,
            hole_radius: settings.hole_mm() / 2.0,
            rows,
        }
    }

    /// Where `at` stands from the centre of the cell nearest it.
    ///
    /// A point inside a rhombus of the lattice is nearest one of its four corners.
    fn off_nearest(&self, at: Vec2) -> Vec2 {
        let (across, up) = (self.pitch, self.pitch * NORMALS[1].y);
        let row = (at.y / up).floor();
        let column = ((at.x - row * across / 2.0) / across).floor();
        let corner = |i: Scalar, j: Scalar| Vec2::new(i * across + j * across / 2.0, j * up);
        [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
            .map(|(i, j)| at - corner(column + i, row + j))
            .into_iter()
            .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
            .unwrap_or(at)
    }

    /// The centre of every cell whose hexagon reaches into `bounds`.
    fn centres(&self, bounds: &Aabb) -> Vec<Vec2> {
        let (across, up) = (self.pitch, self.pitch * NORMALS[1].y);
        let rows = (bounds.mins.y / up).floor() as i32 - 1..=(bounds.maxs.y / up).ceil() as i32 + 1;
        rows.flat_map(|row| {
            let shift = row as Scalar * across / 2.0;
            let first = ((bounds.mins.x - shift) / across).floor() as i32 - 1;
            let last = ((bounds.maxs.x - shift) / across).ceil() as i32 + 1;
            (first..=last)
                .map(move |column| Vec2::new(column as Scalar * across + shift, row as Scalar * up))
        })
        .collect()
    }
}

impl Lattice for HiveCells {
    fn distance(&self, point: Vec3) -> Scalar {
        let local = self.off_nearest(point.truncate());
        let cell = NORMALS
            .iter()
            .map(|normal| local.dot(*normal).abs())
            .fold(Scalar::MIN, Scalar::max)
            - self.inradius;

        let dz = self.rows.off_row(point.z);
        let holes = NORMALS
            .iter()
            .map(|normal| local.dot(normal.perp()).hypot(dz))
            .fold(Scalar::MAX, Scalar::min)
            - self.hole_radius;
        (-cell).max(-holes)
    }

    fn shapes(&self, bounds: &Aabb, keep: &dyn Fn(&Aabb) -> bool) -> Shapes {
        let centres = self.centres(bounds);
        let (bottom, top) = (bounds.mins.z, bounds.maxs.z);
        let corner = |step: usize| {
            Vec2::from_angle(std::f32::consts::FRAC_PI_6 * (2 * step + 1) as Scalar)
                * self.circumradius
        };
        let hexagon: [Vec2; 6] = std::array::from_fn(corner);
        let mut cells = Mesh::default();
        let half = Vec2::splat(self.circumradius);
        for centre in &centres {
            let footprint = Aabb::new(
                (*centre - half).extend(bottom),
                (*centre + half).extend(top),
            );
            if keep(&footprint) {
                cells.append(&prism(&hexagon.map(|at| at + *centre), bottom, top));
            }
        }

        // One group a direction of NORMALS, whose holes never cross; each reaches half an
        // inradius into the cells either side, so none of its faces lies on a wall's.
        let reach = self.inradius / 2.0;
        let (near, far) = (self.inradius - reach, self.pitch - self.inradius + reach);
        let rows: Vec<Scalar> = self
            .rows
            .between(bottom - self.hole_radius, top + self.hole_radius)
            .collect();
        let holes = NORMALS
            .iter()
            .map(|normal| {
                let mut group = Vec::new();
                for centre in &centres {
                    for z in &rows {
                        let at = |reach: Scalar| (*centre + *normal * reach).extend(*z);
                        let bar = Bar {
                            from: at(near),
                            to: at(far),
                            radius: self.hole_radius,
                        };
                        if keep(&bar.aabb()) {
                            group.push(bar);
                        }
                    }
                }
                group
            })
            .collect();
        Shapes::Openings { cells, holes }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::solid_share;
    use super::*;

    /// The default hexagons, with rows of holes from z = 4 every 8 mm.
    fn cells() -> HiveCells {
        HiveCells::new(
            &InfillSettings::default(),
            Rows {
                first: 4.0,
                spacing: 8.0,
            },
        )
    }

    #[test]
    fn a_hive_under_its_holes_fills_the_density_it_was_asked_for() {
        // A hexagon of inradius r on a triangular lattice of pitch p leaves
        // 1 - 2 sqrt(3) r^2 / (sqrt(3)/2 p^2) solid, which the wall was chosen to make.
        let cells = cells();
        let p = cells.pitch;
        let expected = InfillSettings::default().density;
        let share = solid_share(&cells, Vec3::ZERO, Vec3::new(3.0 * p, 3.0 * p, 1.0), 0.05);
        assert!(
            (share - expected).abs() < 0.01,
            "the hive's walls were sized to {expected} solid, got {share}"
        );
    }

    #[test]
    fn a_crossing_hole_joins_two_cells_and_spares_their_junction() {
        let cells = cells();
        let pitch = cells.pitch;
        let between = Vec3::new(pitch / 2.0, 0.0, 4.0);
        assert!(
            cells.distance(between.with_z(1.0)) < 0.0,
            "the wall between two cells"
        );
        assert!(cells.distance(between) > 0.0, "is open at its row");
        let junction = Vec3::new(pitch / 2.0, pitch / (2.0 * (3.0 as Scalar).sqrt()), 4.0);
        assert!(
            cells.distance(junction) < 0.0,
            "where three cells meet stays solid"
        );
    }

    #[test]
    fn the_centre_of_a_cell_is_open() {
        let cells = cells();
        assert!(cells.distance(Vec3::new(0.0, 0.0, 1.0)) > 0.0);
        assert!(
            cells.distance(Vec3::new(cells.pitch, 0.0, 1.0)) > 0.0,
            "and so is its neighbour's"
        );
    }
}
