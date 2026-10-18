use core_geometry::{Aabb, Mesh, Scalar, Vec3, oriented_box};

use super::{InfillSettings, Lattice, Shapes};

/// The edges of a cubic lattice turned so its body diagonal stands upright: each strut leans
/// 35° off level, so none of them starts in mid-air. All three are orthonormal, and they
/// sum to `sqrt(3)` up.
const AXES: [Vec3; 3] = [
    Vec3::new(0.816_496_6, 0.0, 0.577_350_3),
    Vec3::new(-0.408_248_3, std::f32::consts::FRAC_1_SQRT_2, 0.577_350_3),
    Vec3::new(-0.408_248_3, -std::f32::consts::FRAC_1_SQRT_2, 0.577_350_3),
];

/// Square struts along every edge of a cubic lattice stood on a corner. In millimetres.
pub(super) struct Scaffold {
    cell: Scalar,
    half_strut: Scalar,
}

impl Scaffold {
    pub(super) fn new(settings: &InfillSettings) -> Self {
        Self {
            cell: settings.cell_mm,
            half_strut: settings.cell_mm * strut_share(settings.density) / 2.0,
        }
    }
}

impl Lattice for Scaffold {
    fn distance(&self, point: Vec3) -> Scalar {
        let cell = self.cell;
        let off = |at: Scalar| (at - cell * (at / cell).round()).abs();
        let local = AXES.map(|axis| off(point.dot(axis)));
        (0..3)
            .map(|along| local[(along + 1) % 3].max(local[(along + 2) % 3]))
            .fold(Scalar::MAX, Scalar::min)
            - self.half_strut
    }

    fn shapes(&self, bounds: &Aabb) -> Shapes {
        let corners: Vec<Vec3> = (0..8)
            .map(|corner| {
                let pick = |bit: usize, axis: usize| match corner & (1 << bit) {
                    0 => bounds.mins[axis],
                    _ => bounds.maxs[axis],
                };
                Vec3::new(pick(0, 0), pick(1, 1), pick(2, 2))
            })
            .collect();
        let lines = |axis: Vec3| {
            let (low, high) = corners
                .iter()
                .fold((Scalar::MAX, Scalar::MIN), |(low, high), at| {
                    let along = at.dot(axis);
                    (low.min(along), high.max(along))
                });
            (low / self.cell).floor() as i32..=(high / self.cell).ceil() as i32
        };

        // The struts along the first axis run whole; the other two stop at each node,
        // where the three meet in one cube, so no piece overlaps another.
        let groups = (0..3)
            .map(|along| {
                let (one, other) = (AXES[(along + 1) % 3], AXES[(along + 2) % 3]);
                let mut struts = Mesh::default();
                for first in lines(one) {
                    for second in lines(other) {
                        let line = Line {
                            through: (one * first as Scalar + other * second as Scalar) * self.cell,
                            direction: AXES[along],
                            across: [one, other],
                        };
                        for (from, to) in self.pieces(&line, bounds, along > 0) {
                            struts.append(&self.strut(&line, from, to));
                        }
                    }
                }
                struts
            })
            .collect();
        Shapes::Struts(groups)
    }
}

/// One line of the lattice: a point on it, the axis it runs along and the two across it.
struct Line {
    through: Vec3,
    direction: Vec3,
    across: [Vec3; 2],
}

impl Scaffold {
    /// The stretches of `line` inside `bounds` a strut stands on, as distances along it
    /// from `through`: one, or one between every two nodes when `between_nodes`.
    fn pieces(&self, line: &Line, bounds: &Aabb, between_nodes: bool) -> Vec<(Scalar, Scalar)> {
        let Some((enter, leave)) = span(line, bounds) else {
            return Vec::new();
        };
        if !between_nodes {
            return vec![(enter, leave)];
        }
        // `through` lies across the line from the origin, so the nodes are where the
        // distance along it is a whole number of cells.
        let first = (enter / self.cell).floor() as i32;
        let last = (leave / self.cell).ceil() as i32;
        (first..last)
            .map(|node| {
                let from = node as Scalar * self.cell + self.half_strut;
                let to = (node + 1) as Scalar * self.cell - self.half_strut;
                (from.max(enter), to.min(leave))
            })
            .filter(|(from, to)| from < to)
            .collect()
    }

    fn strut(&self, line: &Line, from: Scalar, to: Scalar) -> Mesh {
        oriented_box(
            line.through + line.direction * Scalar::midpoint(from, to),
            [
                line.direction * ((to - from) / 2.0),
                line.across[0] * self.half_strut,
                line.across[1] * self.half_strut,
            ],
        )
    }
}

/// Where `line` enters and leaves `bounds`, as distances along it from `through`.
fn span(line: &Line, bounds: &Aabb) -> Option<(Scalar, Scalar)> {
    let (mut enter, mut leave) = (Scalar::MIN, Scalar::MAX);
    for axis in 0..3 {
        let (low, high) = (bounds.mins[axis], bounds.maxs[axis]);
        let (at, step) = (line.through[axis], line.direction[axis]);
        if step.abs() < Scalar::EPSILON {
            if at < low || at > high {
                return None;
            }
            continue;
        }
        let (a, b) = ((low - at) / step, (high - at) / step);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    (enter < leave).then_some((enter, leave))
}

/// The strut's side as a share `x` of the cell that fills `density` of it.
///
/// Three square bars through a cube, each `x` across, fill `3x² - 2x³` of it: the three
/// bars less the cube where all three meet, counted twice too many. That rises from 0 to 1
/// over `x` from 0 to 1, so halving finds it.
fn strut_share(density: Scalar) -> Scalar {
    let fill = |x: Scalar| 3.0 * x * x - 2.0 * x * x * x;
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..32 {
        let middle = Scalar::midpoint(low, high);
        if fill(middle) < density {
            low = middle;
        } else {
            high = middle;
        }
    }
    Scalar::midpoint(low, high)
}

#[cfg(test)]
mod tests {
    use super::super::tests::solid_share;
    use super::*;
    use crate::infill::InfillPattern;

    #[test]
    fn the_axes_are_a_frame_with_its_diagonal_upright() {
        for (index, axis) in AXES.iter().enumerate() {
            assert!((axis.length() - 1.0).abs() < 1e-5);
            assert!(
                (axis.z - (1.0 / 3.0 as Scalar).sqrt()).abs() < 1e-5,
                "every strut climbs at the same angle"
            );
            for other in &AXES[index + 1..] {
                assert!(
                    axis.dot(*other).abs() < 1e-5,
                    "the axes are square to each other"
                );
            }
        }
    }

    #[test]
    fn a_scaffold_fills_the_density_it_was_asked_for() {
        for density in [0.15, 0.3, 0.5] {
            let scaffold = Scaffold::new(&InfillSettings {
                pattern: InfillPattern::Scaffold,
                density,
                ..InfillSettings::default()
            });
            let share = solid_share(&scaffold, Vec3::ZERO, Vec3::splat(18.0), 0.1);
            assert!(
                (share - density).abs() < 0.015,
                "3x² - 2x³ is exact for square struts, so {density} comes out, got {share}"
            );
        }
    }
}
