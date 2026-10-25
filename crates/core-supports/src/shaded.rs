use std::sync::Arc;

use core_geometry::{Mesh, Scalar, SmoothNormals, smooth_normals};
use rayon::prelude::*;

/// How far two faces across an edge may turn from each other and still be shaded as one
/// surface, degrees: a tube's facets and rings blend, the rim of a pad stays a rim.
pub const CREASE_DEG: Scalar = 50.0;

/// A group's column mesh and the normals that shade it round, built together where the
/// mesh is, so the drawing thread only lays them out.
#[derive(Debug, Clone)]
pub struct Shaded {
    pub mesh: Arc<Mesh>,
    pub normals: Arc<SmoothNormals>,
}

/// Each of `meshes` with its normals smoothed within [`CREASE_DEG`].
pub fn shaded(meshes: Vec<Mesh>) -> Vec<Shaded> {
    meshes
        .into_par_iter()
        .map(|mesh| Shaded {
            normals: Arc::new(smooth_normals(&mesh, CREASE_DEG)),
            mesh: Arc::new(mesh),
        })
        .collect()
}
