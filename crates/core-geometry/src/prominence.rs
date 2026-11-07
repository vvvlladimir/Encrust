mod cells;
mod exposure;
mod saliency;
#[cfg(test)]
mod tests;

use rayon::prelude::*;

use crate::{Bvh, Mesh, Region, Scalar};
use cells::{Grid, Sample, samples};
use exposure::exposure;
use saliency::saliency;

/// The finest cell, as a share of the model's diagonal: twice the 0.3 % Lee et al. take as
/// their smallest scale. Each coarser level doubles it.
const CELL_SHARE: Scalar = 0.006;
/// Levels of cells, the finest to 32 times it: five centre-surround differences.
const LEVELS: usize = 6;
/// The level whose cells are cast from to read how open the surface is.
const EXPOSURE_LEVEL: usize = 1;
/// How much an open face shows with no detail at all: a scar on a smooth face still shows.
const SMOOTH_SHOWS: Scalar = 1.0 / 3.0;

// TODO(step-3): `core-supports` placement moves contacts off prominent faces (ADR 0409).

/// How much each face of a mesh shows on the finished model, from 0 hidden in a fold to 1
/// open and finely detailed: where a support's scar costs most. See `docs/design/prominence.md`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prominence {
    per_face: Vec<Scalar>,
}

impl Prominence {
    /// What the user painted as showing, read as 1 and every other face as 0; read off the
    /// shape where nothing is painted (ADR 0410).
    pub fn shown(mesh: &Mesh, painted: Option<&Region>) -> Self {
        match painted.filter(|painted| !painted.is_empty()) {
            Some(painted) => Self {
                per_face: (0..mesh.faces.len())
                    .map(|face| if painted.contains(face) { 1.0 } else { 0.0 })
                    .collect(),
            },
            None => Self::of(mesh),
        }
    }

    pub fn of(mesh: &Mesh) -> Self {
        let Some(bounds) = mesh.aabb() else {
            return Self::default();
        };
        let diagonal_mm = (bounds.maxs - bounds.mins).length();
        if diagonal_mm <= Scalar::EPSILON {
            return Self {
                per_face: vec![0.0; mesh.faces.len()],
            };
        }
        let samples = samples(mesh);
        let levels: Vec<Grid> = (0..LEVELS)
            .map(|level| Grid::of(&samples, diagonal_mm * CELL_SHARE * (1 << level) as Scalar))
            .collect();
        let detail = saliency(&levels, diagonal_mm);
        let open = exposure(&Bvh::build(mesh), &levels[EXPOSURE_LEVEL], diagonal_mm);
        let finest = &levels[0];
        let per_face = samples
            .par_iter()
            .map(|sample: &Sample| {
                if sample.area_mm2 <= 0.0 {
                    return 0.0;
                }
                let detail = detail.get(&finest.key_of(sample.centroid)).copied();
                let open = open.get(&levels[EXPOSURE_LEVEL].facing_key_of(sample));
                open.copied().unwrap_or(0.0)
                    * (SMOOTH_SHOWS + (1.0 - SMOOTH_SHOWS) * detail.unwrap_or(0.0))
            })
            .collect();
        Self { per_face }
    }

    /// One face's prominence, 0 to 1, by its index in the mesh; zero past the last face.
    pub fn face(&self, face: usize) -> Scalar {
        self.per_face.get(face).copied().unwrap_or(0.0)
    }
}
