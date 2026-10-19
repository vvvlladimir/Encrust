use core_csg::{CsgError, Solid};
use core_geometry::{Aabb, Mesh, Scalar, Vec3, cuboid, cylinder, segments_for};
use rayon::prelude::*;

use super::{Bar, Lattice, ROUND_TOLERANCE_MM, Shapes};

/// Times the cavity is halved: sixteen pieces, two for each of eight threads. Fixed rather
/// than read off the machine, so a cavity carves to the same mesh on any of them.
const HALVINGS: u32 = 4;

/// The shortest side a piece may be cut across, in lattice reaches: below it the cells a
/// cut crosses cost more than the piece does.
const SHORTEST_CUT_REACHES: Scalar = 4.0;

/// The solid `lattice` leaves standing in a cavity, cut exactly from its mesh, the lattice
/// reaching `reach` past it on every side.
///
/// The cavity is halved into pieces carved in parallel and appended; two halves meet face
/// to face, which the fill rule reads as one solid. See `docs/design/hollowing.md`.
pub(super) fn carved(
    cavity_mesh: &Mesh,
    lattice: &dyn Lattice,
    reach: Vec3,
    keep: &(dyn Fn(&Aabb) -> bool + Sync),
) -> Result<Mesh, CsgError> {
    in_pieces(Solid::new(cavity_mesh)?, lattice, reach, keep, HALVINGS)
}

/// `cavity` halved across its longest side `depth` times over, each half carved on its own.
fn in_pieces(
    cavity: Solid,
    lattice: &dyn Lattice,
    reach: Vec3,
    keep: &(dyn Fn(&Aabb) -> bool + Sync),
    depth: u32,
) -> Result<Mesh, CsgError> {
    let Some(bounds) = cavity.aabb() else {
        return Ok(Mesh::default());
    };
    let span = bounds.maxs - bounds.mins;
    let axis = if span.x >= span.y.max(span.z) {
        0
    } else if span.y >= span.z {
        1
    } else {
        2
    };
    if depth == 0 || span[axis] < SHORTEST_CUT_REACHES * reach[axis] {
        return carved_whole(&cavity, lattice, reach, keep);
    }
    // The box overhangs the cavity everywhere but at the cut, so no face of it lies on one
    // of the cavity's.
    let overhang = Vec3::splat(1.0) + span;
    let mut upper = bounds.maxs + overhang;
    upper[axis] = Scalar::midpoint(bounds.mins[axis], bounds.maxs[axis]);
    let lower_box = Solid::new(&cuboid(bounds.mins - overhang, upper))?;
    let (Ok(below), Ok(above)) = rayon::join(
        || cavity.intersection(&lower_box),
        || cavity.difference(&lower_box),
    ) else {
        return carved_whole(&cavity, lattice, reach, keep);
    };
    drop(cavity);
    let (below, above) = rayon::join(
        || in_pieces(below, lattice, reach, keep, depth - 1),
        || in_pieces(above, lattice, reach, keep, depth - 1),
    );
    let mut whole = below?;
    whole.append(&above?);
    Ok(whole)
}

/// What `lattice` leaves standing in all of `cavity`, cut in one.
///
/// A hive or grid is the cavity less its cells, then less each group of holes in turn, so
/// a hole is a real opening through one wall. A scaffold's groups are not unioned: struts
/// that overlap at a node are left for the fill rule the rasteriser slices by, as a
/// support's are (ADR 0041), so the cost follows the struts and not their crossings.
fn carved_whole(
    cavity: &Solid,
    lattice: &dyn Lattice,
    reach: Vec3,
    keep: &(dyn Fn(&Aabb) -> bool + Sync),
) -> Result<Mesh, CsgError> {
    let Some(bounds) = cavity.aabb() else {
        return Ok(Mesh::default());
    };
    let around = Aabb::new(bounds.mins - reach, bounds.maxs + reach);
    match lattice.shapes(&around, keep) {
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

#[cfg(test)]
mod tests {
    use core_geometry::{diagnose, signed_volume};

    use super::super::tests::ball_cavity;
    use super::super::{InfillPattern, InfillSettings, lattice_of};
    use super::*;
    use crate::cancel::Cancel;
    use crate::extract::extract;

    #[test]
    fn a_cavity_carved_in_pieces_keeps_what_it_keeps_carved_whole() {
        let field = ball_cavity(15.0);
        let cavity = Solid::new(&extract(&field, Cancel::never())).expect("a ball is a solid");
        let reach = Vec3::splat(2.0);
        for pattern in InfillPattern::ALL {
            let settings = InfillSettings {
                pattern,
                ..InfillSettings::default()
            };
            let lattice = lattice_of(&settings, -15.0);
            let whole = carved_whole(&cavity, lattice.as_ref(), reach, &|_| true)
                .expect("the ball carves whole");
            let keep = |shape: &Aabb| field.reaches(shape);
            let pieces = in_pieces(cavity.clone(), lattice.as_ref(), reach, &keep, 3)
                .expect("the ball carves in pieces");
            assert_eq!(
                diagnose(&pieces).boundary_edges,
                0,
                "{}: every piece is closed",
                pattern.label()
            );
            let (whole, pieces) = (signed_volume(&whole), signed_volume(&pieces));
            assert!(
                (whole - pieces).abs() < 1e-3 * whole,
                "{}: {whole} mm³ carved whole, {pieces} mm³ in pieces",
                pattern.label()
            );
        }
    }
}
