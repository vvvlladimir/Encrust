use core_csg::{CsgError, Solid};
use core_geometry::{Aabb, Mesh, cylinder, segments_for};
use rayon::prelude::*;

use super::{Bar, Lattice, ROUND_TOLERANCE_MM, Shapes};

/// The solid `lattice` leaves standing in a cavity, cut exactly from its mesh over
/// `bounds`.
///
/// A hive or grid is the cavity less its cells, then less each group of holes in turn, so
/// a hole is a real opening through one wall. A scaffold's groups are not unioned: struts
/// that overlap at a node are left for the fill rule the rasteriser slices by, as a
/// support's are (ADR 0041), so the cost follows the struts and not their crossings.
pub(super) fn carved(
    cavity_mesh: &Mesh,
    lattice: &dyn Lattice,
    bounds: &Aabb,
) -> Result<Mesh, CsgError> {
    let cavity = Solid::new(cavity_mesh)?;
    match lattice.shapes(bounds) {
        Shapes::Openings { cells, holes } => {
            let groups: Vec<Solid> = holes
                .par_iter()
                .map(|group| Solid::new(&bars(group)))
                .collect::<Result<_, _>>()?;
            let mut walls = cavity.difference(&Solid::new(&cells)?)?;
            for group in &groups {
                walls = walls.difference(group)?;
            }
            Ok(walls.mesh())
        }
        Shapes::Struts(groups) => joined(
            groups
                .par_iter()
                .map(|group| Ok(cavity.intersection(&Solid::new(group)?)?.mesh()))
                .collect(),
        ),
    }
}

fn bars(group: &[Bar]) -> Mesh {
    let mut mesh = Mesh::default();
    for bar in group {
        let segments = segments_for(bar.radius, ROUND_TOLERANCE_MM);
        mesh.append(&cylinder(bar.from, bar.to, bar.radius, segments));
    }
    mesh
}

fn joined(pieces: Vec<Result<Mesh, CsgError>>) -> Result<Mesh, CsgError> {
    let mut whole = Mesh::default();
    for piece in pieces {
        whole.append(&piece?);
    }
    Ok(whole)
}
