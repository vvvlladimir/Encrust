use core_geometry::{ClosestPoint, Mesh, Scalar, Vec3, glam::IVec3, point_triangle};
use rayon::prelude::*;

use super::Press;
use crate::sdf::Sdf;
use crate::sign::{SignMode, Signer};

/// Newton steps each vertex takes onto the pressed surface.
const SNAP_STEPS: usize = 3;

/// How far all its steps together may take a vertex, in voxels: half a voxel is what the
/// lattice already promises, so a vertex asked to go further is on a wall too steep for
/// the field's own slope to aim it.
const SNAP_LIMIT_VOXELS: Scalar = 0.5;

/// Rounds of putting back the corners of faces snapping turned over. Each round can turn
/// a neighbour back the other way, and a handful settles every case seen.
const UNFLIP_ROUNDS: usize = 4;

impl Press<'_> {
    /// Takes each vertex of `mesh`, which `field` was extracted into, onto the surface the
    /// texture describes, so the relief is placed by the model and the picture rather than
    /// by the lattice; see `docs/design/volume.md`.
    pub(super) fn snap(&self, mesh: &mut Mesh, field: &Sdf) {
        let signer = Signer::new(self.mesh, SignMode::Auto);
        let limit_mm = SNAP_LIMIT_VOXELS * field.grid().voxel_mm;
        let placed = mesh.vertices.clone();
        let t0 = std::time::Instant::now(); // [DEBUG-q7x2]
        let err = |vs: &[Vec3]| {
            let e: Vec<Scalar> = vs
                .par_iter()
                .filter_map(|v| {
                    let f = self.bvh.closest(self.mesh, *v)?;
                    Some(self.pressed_value(*v, &f, &signer))
                })
                .map(Scalar::abs)
                .collect();
            (
                e.iter().sum::<Scalar>() / e.len() as Scalar,
                e.iter().copied().fold(0.0, Scalar::max),
            )
        }; // [DEBUG-q7x2]
        let before = if std::env::var("ERR").is_ok() {
            err(&mesh.vertices)
        } else {
            (0.0, 0.0)
        }; // [DEBUG-q7x2]
        if std::env::var("NOSNAP").is_err() {
            // [DEBUG-q7x2]
            mesh.vertices
                .par_iter_mut()
                .for_each(|vertex| *vertex = self.snapped(*vertex, field, &signer, limit_mm));
            unflip(mesh, &placed);
        } // [DEBUG-q7x2]
        let after = if std::env::var("ERR").is_ok() {
            err(&mesh.vertices)
        } else {
            (0.0, 0.0)
        }; // [DEBUG-q7x2]
        eprintln!(
            "[DEBUG-q7x2] snap {:.2}s verts {} err before mean {:.4} max {:.4} after mean {:.4} max {:.4}",
            t0.elapsed().as_secs_f32(),
            mesh.vertices.len(),
            before.0,
            before.1,
            after.0,
            after.1
        ); // [DEBUG-q7x2]
    }

    /// Where Newton's method along the field's slope takes `start`, stopping at the first
    /// step that does not bring it nearer the surface.
    fn snapped(&self, start: Vec3, field: &Sdf, signer: &Signer, limit_mm: Scalar) -> Vec3 {
        let Some(found) = self.bvh.closest(self.mesh, start) else {
            return start;
        };
        let (mut at, mut face) = (start, found.face);
        let mut miss = self.pressed_value(start, &found, signer);
        for _ in 0..SNAP_STEPS {
            let slope = gradient(field, at);
            let steep = slope.length_squared();
            if steep <= Scalar::EPSILON {
                break;
            }
            let next = at - slope * (miss / steep);
            let next = start + (next - start).clamp_length_max(limit_mm);
            let Some(found) = self.nearest_from(next, face) else {
                break;
            };
            let value = self.pressed_value(next, &found, signer);
            if value.abs() >= miss.abs() {
                break;
            }
            (at, miss, face) = (next, value, found.face);
        }
        at
    }

    /// The nearest point of the model to `point`, searched no further than the face the
    /// last step stood nearest: a step is short, so that face is nearly always still it.
    fn nearest_from(&self, point: Vec3, face: usize) -> Option<ClosestPoint> {
        let triangle = self.mesh.triangle(face)?;
        let on_face = point_triangle(point, &triangle);
        let bound = ClosestPoint {
            point: on_face,
            distance: (on_face - point).length(),
            face,
        };
        Some(
            self.bvh
                .closest_within(self.mesh, point, bound.distance)
                .unwrap_or(bound),
        )
    }

    /// The signed distance from `point` to the pressed surface, read exactly from its
    /// nearest point of the model and the texture there, not from the lattice.
    fn pressed_value(&self, point: Vec3, found: &ClosestPoint, signer: &Signer) -> Scalar {
        let signed = if signer.is_inside(self.mesh, point, found) {
            -found.distance
        } else {
            found.distance
        };
        self.moved(signed, point, found)
    }
}

/// The slope of `field` at `point`: the gradient of the trilinear field of the cell it
/// stands in, which is eight lattice values rather than a sample per direction.
fn gradient(field: &Sdf, point: Vec3) -> Vec3 {
    let grid = field.grid();
    let low = grid.voxel(point);
    let at = ((point - grid.position(low)) / grid.voxel_mm).clamp(Vec3::ZERO, Vec3::ONE);
    let corner = |x: i32, y: i32, z: i32| field.value(low + IVec3::new(x, y, z));
    let values: [[[Scalar; 2]; 2]; 2] = std::array::from_fn(|z| {
        std::array::from_fn(|y| std::array::from_fn(|x| corner(x as i32, y as i32, z as i32)))
    });
    let lerp = |a: Scalar, b: Scalar, t: Scalar| a + (b - a) * t;
    // Each component is the difference across one axis, blended over the other two.
    let across = |pick: &dyn Fn(usize, usize) -> (Scalar, Scalar), u: Scalar, v: Scalar| {
        let rows = [0, 1].map(|j| {
            let (near0, far0) = pick(0, j);
            let (near1, far1) = pick(1, j);
            lerp(far0 - near0, far1 - near1, u)
        });
        lerp(rows[0], rows[1], v)
    };
    let dx = across(&|i, j| (values[j][i][0], values[j][i][1]), at.y, at.z);
    let dy = across(&|i, j| (values[j][0][i], values[j][1][i]), at.x, at.z);
    let dz = across(&|i, j| (values[0][j][i], values[1][j][i]), at.x, at.y);
    Vec3::new(dx, dy, dz) / grid.voxel_mm
}

/// Puts back where the lattice placed them the corners of every face snapping turned over.
fn unflip(mesh: &mut Mesh, placed: &[Vec3]) {
    let normal = |vertices: &[Vec3], face: &[u32; 3]| {
        let [a, b, c] = face.map(|corner| vertices[corner as usize]);
        (b - a).cross(c - a)
    };
    for _ in 0..UNFLIP_ROUNDS {
        let turned: Vec<u32> = mesh
            .faces
            .par_iter()
            .filter(|face| normal(placed, face).dot(normal(&mesh.vertices, face)) < 0.0)
            .flat_map_iter(|face| face.iter().copied())
            .collect();
        if turned.is_empty() {
            return;
        }
        for corner in turned {
            mesh.vertices[corner as usize] = placed[corner as usize];
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::grid::{TILE, VoxelGrid};
    use crate::sdf::{Quantised, TILE_VALUES, encode};

    #[test]
    fn the_slope_of_a_field_rising_through_a_plane_is_its_normal() {
        // Rises one millimetre a millimetre along a tilted direction, so its gradient is
        // that direction whatever cell it is read in.
        let rise = Vec3::new(0.48, -0.6, 0.64);
        let grid = VoxelGrid::new(0.1);
        let band_mm = 2.0;
        let values: Box<[Quantised]> = (0..TILE_VALUES as i32)
            .map(|index| {
                let voxel = IVec3::new(index % TILE, (index / TILE) % TILE, index / (TILE * TILE));
                encode(grid.position(voxel).dot(rise) - 0.3, band_mm)
            })
            .collect();
        let field = Sdf::new(
            grid,
            band_mm,
            HashMap::from([(IVec3::ZERO, values)]),
            HashMap::new(),
        );
        let slope = gradient(&field, Vec3::new(0.33, 0.27, 0.41));
        assert!(
            (slope - rise).length() < 1e-3,
            "got {slope}, expected {rise}"
        );
    }
}
