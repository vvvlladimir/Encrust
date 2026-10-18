use core_csg::{CsgError, Solid};
use core_geometry::{Aabb, Bvh, Mesh, Scalar, cylinder, segments_for};
use rayon::prelude::*;

use super::{Bar, Carved, Lattice, ROUND_TOLERANCE_MM, Shapes};
use crate::sdf::Sdf;

/// The void and the solid `lattice` makes of a cavity, cut exactly from its meshes over
/// `bounds`.
///
/// Nothing is unioned: the pieces are closed shells that may overlap, and the fill rule the
/// rasteriser slices by unions them, as it does a support's struts (ADR 0041). Each group
/// is one boolean against the cavity, so the cost follows the cavity and the cells, not
/// the thousands of places where a lattice crosses itself.
pub(super) fn carved(
    cavity_mesh: &Mesh,
    cavity: &Sdf,
    lattice: &dyn Lattice,
    bounds: &Aabb,
) -> Result<Carved, CsgError> {
    let solid = Solid::new(cavity_mesh)?;
    match lattice.shapes(bounds) {
        Shapes::Openings { cells, holes } => {
            let inside = Inside::new(cavity_mesh, cavity);
            let mut pieces = vec![Ok(solid.intersection(&Solid::new(&cells)?)?.mesh())];
            pieces.par_extend(holes.par_iter().map(|group| opened(&solid, group, &inside)));
            Ok(Carved {
                void: joined(pieces)?,
                solid: Mesh::default(),
            })
        }
        Shapes::Struts(groups) => {
            let pieces = groups
                .par_iter()
                .map(|group| Ok(solid.intersection(&Solid::new(group)?)?.mesh()))
                .collect();
            Ok(Carved {
                void: cavity_mesh.clone(),
                solid: joined(pieces)?,
            })
        }
    }
}

/// The holes of one group as the void they open: whole where they stand clear inside the
/// cavity, cut by it where they reach its side.
fn opened(cavity: &Solid, group: &[Bar], inside: &Inside<'_>) -> Result<Mesh, CsgError> {
    let (mut clear, mut crossing) = (Mesh::default(), Mesh::default());
    for bar in group {
        let segments = segments_for(bar.radius, ROUND_TOLERANCE_MM);
        let mesh = cylinder(bar.from, bar.to, bar.radius, segments);
        if inside.holds(bar) {
            clear.append(&mesh);
        } else {
            crossing.append(&mesh);
        }
    }
    clear.append(&cavity.intersection(&Solid::new(&crossing)?)?.mesh());
    Ok(clear)
}

/// Whether a bar stands wholly inside the cavity: its middle is inside, and the cavity's
/// surface is further from it than any point of the bar.
struct Inside<'a> {
    mesh: &'a Mesh,
    bvh: Bvh,
    field: &'a Sdf,
}

impl<'a> Inside<'a> {
    fn new(mesh: &'a Mesh, field: &'a Sdf) -> Self {
        Self {
            mesh,
            bvh: Bvh::build(mesh),
            field,
        }
    }

    fn holds(&self, bar: &Bar) -> bool {
        let middle = (bar.from + bar.to) / 2.0;
        let reach: Scalar = ((bar.to - bar.from).length() / 2.0).hypot(bar.radius);
        self.field.sample(middle) < 0.0
            && self
                .bvh
                .closest(self.mesh, middle)
                .is_none_or(|nearest| nearest.distance > reach)
    }
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
    use core_geometry::Vec3;

    use super::*;
    use crate::cancel::Cancel;
    use crate::extract::extract;
    use crate::infill::tests::ball_cavity;

    #[test]
    fn a_bar_reaching_the_side_of_the_cavity_is_not_clear_inside_it() {
        let cavity = ball_cavity(10.0);
        let mesh = extract(&cavity, Cancel::never());
        let inside = Inside::new(&mesh, &cavity);
        let bar = |x: Scalar| Bar {
            from: Vec3::new(x - 2.0, 0.0, 0.0),
            to: Vec3::new(x + 2.0, 0.0, 0.0),
            radius: 1.0,
        };
        assert!(inside.holds(&bar(0.0)), "in the middle of a 10 mm ball");
        assert!(!inside.holds(&bar(8.5)), "its end is past the side");
        assert!(!inside.holds(&bar(30.0)), "nowhere near the ball");
    }
}
