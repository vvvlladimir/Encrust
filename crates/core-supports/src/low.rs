use core_geometry::{Mesh, Scalar, Transform, Vec3};
use rayon::prelude::*;

use crate::config::REFERENCE_RISE_MM;
use crate::field::{Grid, Parts};

/// How much lower than every vertex it shares a face with a vertex has to hang to be a
/// low point, millimetres: above float noise at plate scale, under any printable step.
const LOWER_BY_MM: Scalar = 1e-4;

/// How many cells round a low point a piece may begin in and still hang from it: a point is
/// thinner than a cell for its first layers.
const NEAR_CELLS: i32 = 2;

/// The points of a placed model, in plate coordinates, that hang lower than every vertex they
/// share a face with; the mesh is taken as welded, and placement keeps those a part begins at.
pub fn low_points(model: &Mesh, transform: Transform) -> Vec<Vec3> {
    let matrix = transform.to_matrix();
    let placed: Vec<Vec3> = model
        .vertices
        .par_iter()
        .map(|vertex| matrix.transform_point3(*vertex))
        .collect();

    let mut lowest_beside = vec![Scalar::INFINITY; placed.len()];
    for face in &model.faces {
        for corner in 0..3 {
            let (at, one, two) = (
                face[corner] as usize,
                face[(corner + 1) % 3] as usize,
                face[(corner + 2) % 3] as usize,
            );
            if let (Some(one), Some(two)) = (placed.get(one), placed.get(two))
                && let Some(lowest) = lowest_beside.get_mut(at)
            {
                *lowest = lowest.min(one.z).min(two.z);
            }
        }
    }

    placed
        .iter()
        .zip(&lowest_beside)
        .filter(|(point, beside)| beside.is_finite() && point.z + LOWER_BY_MM < **beside)
        .map(|(point, _)| *point)
        .collect()
}

/// The low points of a model as the climb meets them, each waiting for the piece that
/// begins at it.
pub(crate) struct Lows {
    /// Not reached yet, highest first, so the next one is taken off the end.
    ahead: Vec<(Vec3, [i32; 2])>,
    /// Reached, and still within [`REFERENCE_RISE_MM`] of the layer being looked at.
    waiting: Vec<(Vec3, [i32; 2])>,
}

impl Lows {
    pub fn new(points: &[Vec3], grid: &Grid) -> Self {
        let mut ahead: Vec<(Vec3, [i32; 2])> = points
            .iter()
            .map(|point| (*point, grid.cell_of(point.truncate())))
            .collect();
        ahead.sort_by(|a, b| b.0.z.total_cmp(&a.0.z));
        Self {
            ahead,
            waiting: Vec::new(),
        }
    }

    /// Takes on the points a cut at `plane_mm` has passed, and gives up on the ones no
    /// piece has begun at within a millimetre of rise: those were bumps on a surface.
    pub fn reach(&mut self, plane_mm: Scalar) {
        while let Some(low) = self.ahead.pop_if(|low| low.0.z <= plane_mm) {
            self.waiting.push(low);
        }
        self.waiting
            .retain(|low| low.0.z >= plane_mm - REFERENCE_RISE_MM);
    }

    /// Hands each waiting point to the piece of `parts` at or beside its cell, where
    /// `wants` says that piece is still looking for the point it hangs from.
    pub fn claim(&mut self, parts: &Parts, mut wants: impl FnMut(u32) -> bool) -> Vec<(u32, Vec3)> {
        let mut claimed = Vec::new();
        self.waiting.retain(|(point, cell)| {
            let Some(piece) = beside(parts, *cell).filter(|piece| wants(*piece)) else {
                return true;
            };
            claimed.push((piece, *point));
            false
        });
        claimed
    }
}

/// The piece covering `cell`, or failing that the nearest one within [`NEAR_CELLS`].
fn beside(parts: &Parts, cell: [i32; 2]) -> Option<u32> {
    (0..=NEAR_CELLS).find_map(|reach| {
        (-reach..=reach)
            .flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy)))
            .filter(|(dx, dy)| dx.abs().max(dy.abs()) == reach)
            .find_map(|(dx, dy)| parts.at(cell[0] + dx, cell[1] + dy))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::box_mesh;

    /// A four-sided pyramid standing on its point at the origin, `height` tall.
    fn pyramid_on_its_point(height: Scalar) -> Mesh {
        Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(-5.0, -5.0, height),
                Vec3::new(5.0, -5.0, height),
                Vec3::new(5.0, 5.0, height),
                Vec3::new(-5.0, 5.0, height),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [0, 4, 3],
                [0, 1, 4],
                [1, 2, 3],
                [1, 3, 4],
            ],
        )
    }

    #[test]
    fn the_point_a_pyramid_stands_on_is_its_one_low_point() {
        let lifted = Transform::from_translation(Vec3::new(3.0, 4.0, 2.0));
        let found = low_points(&pyramid_on_its_point(10.0), lifted);
        assert_eq!(found, vec![Vec3::new(3.0, 4.0, 2.0)]);
    }

    #[test]
    fn a_box_has_no_point_lower_than_the_rest_of_its_floor() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
        assert!(low_points(&floating, Transform::default()).is_empty());
    }

    #[test]
    fn turning_the_model_over_moves_its_low_point() {
        let turned =
            Transform::default().turned(core_geometry::Quat::from_rotation_x(std::f32::consts::PI));
        let found = low_points(&pyramid_on_its_point(10.0), turned);
        assert!(
            found.is_empty(),
            "upside down the pyramid's lowest face is flat, got {found:?}"
        );
    }
}
