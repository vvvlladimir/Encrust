use std::collections::HashMap;
use std::hash::Hasher;

use core_geometry::{
    Bvh, ClosestPoint, FastHasher, FastMap, Mesh, Scalar, Vec3,
    glam::{IVec2, IVec3},
    point_triangle,
};
use rayon::prelude::*;

use super::{Press, ReliefSettings};
use crate::boolean::united;
use crate::build::{FieldSettings, INSIDE, NO_FACE, build_nearest};
use crate::error::VolumeError;
use crate::grid::{TILE, VoxelGrid};
use crate::hollow::{BAND_VOXELS, coarsening};
use crate::sdf::{Quantised, Sdf, TILE_VALUES, encode};
use crate::sign::SignMode;

/// A model's nearest face, and its side, at every lattice point of the band a relief is
/// cut in.
///
/// It depends on the model, the lattice and the range the surface moves over, and not on
/// the picture, its tiling, how its grey is read or the mask: kept, it lets another
/// picture be pressed into the same model without the search that is most of a press.
/// Four bytes a voxel; see `docs/design/volume.md`.
pub struct ReliefBasis {
    shape: Shape,
    /// The model's shells unioned into one surface, when it has several, which the press
    /// moves in place of the model so that no face buried in another shell takes a relief.
    united: Option<(Mesh, Bvh)>,
    /// The lattice precision asked for, and the one the budget allowed, mm.
    asked_mm: Scalar,
    budget_bytes: usize,
    grid: VoxelGrid,
    middle_mm: Scalar,
    reach_mm: Scalar,
    band_mm: Scalar,
    tiles: FastMap<IVec3, Box<[u32]>>,
    solid: HashMap<IVec2, Vec<(i32, i32)>>,
}

/// Enough of a mesh to tell a basis was built over a different one: its counts, and a hash
/// of every vertex and face, so a vertex moved inside the same box tells too.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Shape {
    vertices: usize,
    faces: usize,
    content: u64,
}

impl Shape {
    fn of(mesh: &Mesh) -> Self {
        let mut hasher = FastHasher::default();
        for vertex in &mesh.vertices {
            let [x, y, z] = vertex.to_array().map(f32::to_bits);
            hasher.write_u64(u64::from(x) << 32 | u64::from(y));
            hasher.write_u64(u64::from(z));
        }
        for [a, b, c] in &mesh.faces {
            hasher.write_u64(u64::from(*a) << 32 | u64::from(*b));
            hasher.write_u64(u64::from(*c));
        }
        Self {
            vertices: mesh.vertices.len(),
            faces: mesh.faces.len(),
            content: hasher.finish(),
        }
    }
}

impl std::fmt::Debug for ReliefBasis {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReliefBasis")
            .field("voxel_mm", &self.grid.voxel_mm)
            .field("tiles", &self.tiles.len())
            .finish_non_exhaustive()
    }
}

impl ReliefBasis {
    /// What keeping it costs, in bytes, but for the unioned surface when there is one.
    pub fn bytes(&self) -> usize {
        self.tiles.len() * (TILE_VALUES * size_of::<u32>() + size_of::<(IVec3, Box<[u32]>)>())
    }

    /// The surface the press moves: the unioned shells, or `mesh` itself.
    pub(super) fn surface<'a>(&'a self, mesh: &'a Mesh, bvh: &'a Bvh) -> (&'a Mesh, &'a Bvh) {
        self.united
            .as_ref()
            .map_or((mesh, bvh), |(united, hierarchy)| (united, hierarchy))
    }

    pub(super) fn is_united(&self) -> bool {
        self.united.is_some()
    }

    /// The lattice it was built on, mm.
    pub fn voxel_mm(&self) -> Scalar {
        self.grid.voxel_mm
    }

    /// Whether a press of `mesh` with `settings` would build this very basis again.
    #[expect(clippy::float_cmp, reason = "the same settings compute the same bits")]
    pub(super) fn fits(&self, mesh: &Mesh, settings: &ReliefSettings) -> bool {
        let (middle_mm, reach_mm) = settings.range_mm();
        self.shape == Shape::of(mesh)
            && self.asked_mm == settings.voxel_mm(mesh.surface_area())
            && self.budget_bytes == settings.budget_bytes
            && self.middle_mm == middle_mm
            && self.reach_mm == reach_mm
    }

    /// The basis of a press of `mesh` with `settings`, on the finest lattice the budget
    /// allows.
    pub(super) fn build(
        mesh: &Mesh,
        bvh: &Bvh,
        settings: &ReliefSettings,
    ) -> Result<Self, VolumeError> {
        let asked_mm = settings.voxel_mm(mesh.surface_area());
        // A mesh the exact cut cannot read is pressed as it stands, buried faces and all.
        let united = match united(mesh) {
            Some(Ok(united)) => {
                let hierarchy = Bvh::build(&united);
                Some((united, hierarchy))
            }
            None | Some(Err(_)) => None,
        };
        let (surface, hierarchy) = united
            .as_ref()
            .map_or((mesh, bvh), |(united, hierarchy)| (united, hierarchy));
        let (basis, _) = coarsening(asked_mm, |voxel_mm| {
            Self::on_lattice(surface, hierarchy, settings, asked_mm, voxel_mm)
        })?;
        Ok(basis.over(mesh, united))
    }

    /// The basis known as one of `mesh`, whatever surface it was built over.
    fn over(self, mesh: &Mesh, united: Option<(Mesh, Bvh)>) -> Self {
        Self {
            shape: Shape::of(mesh),
            united,
            ..self
        }
    }

    /// One attempt at a given lattice. The band is built around the middle of the range
    /// the surface can move over, and is as wide as half that range on top of what
    /// marching the cells needs.
    fn on_lattice(
        mesh: &Mesh,
        bvh: &Bvh,
        settings: &ReliefSettings,
        asked_mm: Scalar,
        voxel_mm: Scalar,
    ) -> Result<Self, VolumeError> {
        let (middle_mm, reach_mm) = settings.range_mm();
        let band_voxels = reach_mm / voxel_mm + BAND_VOXELS;
        let built = build_nearest(
            mesh,
            bvh,
            &FieldSettings {
                voxel_mm,
                band_voxels,
                iso_mm: middle_mm,
                sign: SignMode::Auto,
                clip: None,
                budget_bytes: settings.budget_bytes,
            },
        )?;
        Ok(Self {
            shape: Shape::of(mesh),
            united: None,
            asked_mm,
            budget_bytes: settings.budget_bytes,
            grid: built.grid,
            middle_mm,
            reach_mm,
            band_mm: band_voxels * voxel_mm,
            tiles: built.tiles.into_iter().collect(),
            solid: built.solid,
        })
    }

    pub(super) fn coarsened(&self) -> bool {
        self.grid.voxel_mm > self.asked_mm
    }
}

impl Press<'_> {
    /// The field of the pressed surface over `basis`: each voxel's distance read back off
    /// its nearest face, and moved by the texture there.
    pub(super) fn field_on(&self, basis: &ReliefBasis) -> Sdf {
        let tiles: HashMap<IVec3, Box<[Quantised]>> = basis
            .tiles
            .par_iter()
            .map(|(tile, records)| (*tile, self.tile_on(basis, *tile, records)))
            .collect();
        Sdf::new(basis.grid, basis.band_mm, tiles, basis.solid.clone())
    }

    fn tile_on(&self, basis: &ReliefBasis, tile: IVec3, records: &[u32]) -> Box<[Quantised]> {
        let base = tile * TILE;
        let band_mm = basis.band_mm;
        (0..TILE * TILE * TILE)
            .map(|index| {
                let record = records[index as usize];
                let local = IVec3::new(index % TILE, (index / TILE) % TILE, index / (TILE * TILE));
                let point = basis.grid.position(base + local);
                let found = (record != NO_FACE)
                    .then(|| self.nearest_on(record & !INSIDE, point))
                    .flatten();
                let Some(found) = found else {
                    return Quantised::MAX;
                };
                let signed = if record & INSIDE == 0 {
                    found.distance
                } else {
                    -found.distance
                };
                // Past the band no displacement can reach back across zero, so neither
                // the texture nor the mask is worth reading.
                let value = if (signed - basis.middle_mm).abs() >= band_mm {
                    signed
                } else {
                    self.moved(signed, point, &found)
                };
                encode(value, band_mm)
            })
            .collect()
    }

    fn nearest_on(&self, face: u32, point: Vec3) -> Option<ClosestPoint> {
        let triangle = self.mesh.triangle(face as usize)?;
        let on_face = point_triangle(point, &triangle);
        Some(ClosestPoint {
            point: on_face,
            distance: (on_face - point).length(),
            face: face as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::cuboid;

    use super::*;

    #[test]
    fn a_vertex_moved_inside_the_same_box_is_another_shape() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let mut dented = cube.clone();
        let corner = dented
            .vertices
            .iter_mut()
            .find(|vertex| **vertex == Vec3::splat(10.0))
            .expect("the cube has its far corner");
        *corner = Vec3::splat(9.0);
        assert_eq!(Shape::of(&cube), Shape::of(&cube.clone()));
        assert_ne!(
            Shape::of(&cube),
            Shape::of(&dented),
            "same counts, and the box still reaches 10 mm on every axis through the others"
        );
    }
}
