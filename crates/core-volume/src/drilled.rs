use std::ops::Range;
use std::sync::Arc;

use core_csg::Solid;
use core_geometry::{Aabb, Bvh, Mesh, Scalar, split};
use rayon::prelude::*;

use crate::boolean::solid_of;
use crate::drain::CUT_WEIGHT;
use crate::error::VolumeError;
use crate::mend::readable;

/// `mesh` with the cuts [`crate::drill`] made taken out of it exactly, through the engine:
/// a hole leaves its bore in the model it was drilled in and in nothing beside it (ADR 0322).
///
/// A model the engine refuses is mended first; one it refuses even then, or one that leaves
/// nothing, is an error, and its cuts are left to the winding rule.
pub fn drilled(mesh: &Mesh, cuts: &Mesh) -> Result<Mesh, VolumeError> {
    let cutter = solid_of(&once_over(cuts)?)?;
    cut_by(mesh, &cutter)
}

/// `mesh`, mended where the engine refuses it, less `cutter`.
fn cut_by(mesh: &Mesh, cutter: &Solid) -> Result<Mesh, VolumeError> {
    let left = readable(mesh)?.difference(cutter)?.mesh();
    if left.faces.is_empty() {
        return Err(VolumeError::NothingLeft);
    }
    Ok(left)
}

/// How far a face of a drilled shell may stand off the cavity it came from and still be part
/// of it, millimetres: the engine hands back what it was given, rounded through `f64`.
const ON_CAVITY_MM: Scalar = 1e-3;

/// A model with its cuts taken out exactly, as a viewport draws it and a plate prints it.
#[derive(Debug, Clone, PartialEq)]
pub struct Drilled {
    pub mesh: Arc<Mesh>,
    /// Which faces of `mesh` bound the cavity, as `Shell::cavity` names them for the shell.
    pub cavity: Option<Range<usize>>,
}

impl Drilled {
    /// `source` drilled as [`drilled`] does; `cavity`, the faces of a hollowed model's shell
    /// that bound its cavity, carried over onto what is left.
    pub fn of(
        source: &Mesh,
        cavity: Option<Range<usize>>,
        cuts: &Mesh,
    ) -> Result<Self, VolumeError> {
        let left = drilled(source, cuts)?;
        let Some(cavity) = cavity else {
            return Ok(Self {
                mesh: Arc::new(left),
                cavity: None,
            });
        };
        let (mesh, cavity) = cavity_last(left, source, cavity);
        Ok(Self {
            mesh: Arc::new(mesh),
            cavity: Some(cavity),
        })
    }
}

/// `drilled` with its faces that lie on `source`'s `cavity` moved to the end, and their range.
fn cavity_last(drilled: Mesh, source: &Mesh, cavity: Range<usize>) -> (Mesh, Range<usize>) {
    let bounding = Mesh::new(source.vertices.clone(), source.faces[cavity].to_vec());
    let bvh = Bvh::build(&bounding);
    let on_cavity: Vec<bool> = drilled
        .faces
        .par_iter()
        .map(|face| {
            let middle = face
                .iter()
                .map(|corner| drilled.vertices[*corner as usize])
                .sum::<core_geometry::Vec3>()
                / 3.0;
            bvh.closest_within(middle, ON_CAVITY_MM).is_some()
        })
        .collect();
    let (mut outer, inner): (Vec<_>, Vec<_>) = drilled
        .faces
        .iter()
        .zip(&on_cavity)
        .partition(|(_, on)| !**on);
    let start = outer.len();
    outer.extend(inner);
    let faces = outer.into_iter().map(|(face, _)| *face).collect::<Vec<_>>();
    let end = faces.len();
    (Mesh::new(drilled.vertices, faces), start..end)
}

/// `mesh` with each of its separate pieces that `cuts` reach drilled as [`drilled`] does, and
/// the rest as they were: the support trees under a model, few of which stand in a hole.
///
/// A piece the engine cannot cut is kept whole, so a tree it refuses still stands.
pub fn drilled_pieces(mesh: &Mesh, cuts: &Mesh) -> Mesh {
    let Ok(once) = once_over(cuts) else {
        return mesh.clone();
    };
    let bodies: Vec<Aabb> = split(&once).iter().filter_map(Mesh::aabb).collect();
    let reached = |piece: &Mesh| {
        piece
            .aabb()
            .is_some_and(|bounds| bodies.iter().any(|body| meets(&bounds, body)))
    };
    if !reached(mesh) {
        return mesh.clone();
    }
    let pieces = split(mesh);
    if !pieces.iter().any(reached) {
        return mesh.clone();
    }
    let Ok(cutter) = solid_of(&once) else {
        return mesh.clone();
    };
    let pieces: Vec<Mesh> = pieces
        .into_par_iter()
        .map(|piece| {
            if reached(&piece) {
                cut_by(&piece, &cutter).unwrap_or(piece)
            } else {
                piece
            }
        })
        .collect();
    let mut whole = Mesh::default();
    for piece in &pieces {
        whole.append(piece);
    }
    whole
}

/// The bodies of `cuts` once over and wound outward: [`crate::drill`] lays them
/// [`CUT_WEIGHT`] deep, wound inward, each copy on vertices of its own.
fn once_over(cuts: &Mesh) -> Result<Mesh, VolumeError> {
    let (vertices, faces) = (cuts.vertices.len(), cuts.faces.len());
    let once = &cuts.faces[..faces / CUT_WEIGHT];
    let laid = vertices % CUT_WEIGHT == 0 && faces % CUT_WEIGHT == 0 && faces > 0;
    let own = (vertices / CUT_WEIGHT) as u32;
    if !laid || once.iter().flatten().any(|corner| *corner >= own) {
        return Err(VolumeError::NotCuts);
    }
    Ok(Mesh::new(
        cuts.vertices[..own as usize].to_vec(),
        once.iter().map(|[a, b, c]| [*a, *c, *b]).collect(),
    ))
}

/// Whether two boxes share any point, their faces included.
pub(crate) fn meets(one: &Aabb, other: &Aabb) -> bool {
    one.mins.cmple(other.maxs).all() && one.maxs.cmpge(other.mins).all()
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid, signed_volume};

    use super::*;
    use crate::drain::{Channel, DrainHole, HoleShape, drill};

    /// A square hole 2 mm a side straight down through the middle of a 20 by 20 by 10 slab.
    fn square_hole_through() -> DrainHole {
        DrainHole {
            at: Vec3::new(10.0, 10.0, 10.0),
            axis: Vec3::NEG_Z,
            diameter_mm: 2.0 * std::f32::consts::SQRT_2,
            depth_mm: 12.0,
            taper: 1.0,
            lift_mm: 0.0,
            shape: HoleShape::Polygon { sides: 4 },
            turn_deg: 45.0,
        }
    }

    fn slab() -> Mesh {
        cuboid(Vec3::ZERO, Vec3::new(20.0, 20.0, 10.0))
    }

    #[test]
    fn a_hole_through_a_slab_takes_out_its_bore_exactly() {
        let cuts = drill(&[square_hole_through()], &[]).expect("the hole drills");
        let left = drilled(&slab(), &cuts).expect("a slab is a solid");
        let volume = signed_volume(&left);
        // 4000 mm3 of slab less a 2 by 2 mm square bore through its 10 mm.
        assert!(
            (volume - 3960.0).abs() < 1e-2,
            "the slab less its 40 mm3 bore is 3960 mm3, got {volume}"
        );
    }

    #[test]
    fn a_channel_and_the_holes_it_crosses_are_taken_out_as_one() {
        let channel = Channel {
            points: vec![Vec3::new(2.0, 10.0, 5.0), Vec3::new(18.0, 10.0, 5.0)],
            diameter_mm: 2.0 * std::f32::consts::SQRT_2,
            shape: HoleShape::Polygon { sides: 4 },
            turn_deg: 45.0,
        };
        let cuts = drill(&[square_hole_through()], &[channel]).expect("both cut");
        let left = drilled(&slab(), &cuts).expect("a slab is a solid");
        let volume = signed_volume(&left);
        // The 40 mm3 bore and a 2 by 2 by 16 mm tunnel, the up to 8 mm3 they share once.
        assert!(
            volume > 4000.0 - 40.0 - 64.0 + 1.0 && volume <= 4000.0 - 40.0 - 64.0 + 8.0 + 1e-2,
            "the crossing is taken out once, got {volume}"
        );
    }

    #[test]
    fn a_surface_with_no_inside_leaves_nothing_to_drill() {
        let sheet = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 10.0),
                Vec3::new(20.0, 0.0, 10.0),
                Vec3::new(20.0, 20.0, 10.0),
            ],
            vec![[0, 1, 2]],
        );
        let cuts = drill(&[square_hole_through()], &[]).expect("the hole drills");
        assert_eq!(drilled(&sheet, &cuts), Err(VolumeError::NothingLeft));
    }

    #[test]
    fn a_hole_that_takes_the_whole_model_leaves_nothing() {
        let tiny = cuboid(Vec3::new(9.5, 9.5, 4.0), Vec3::new(10.5, 10.5, 5.0));
        let cuts = drill(&[square_hole_through()], &[]).expect("the hole drills");
        assert_eq!(drilled(&tiny, &cuts), Err(VolumeError::NothingLeft));
    }

    #[test]
    fn a_drilled_shell_keeps_its_cavity_apart_from_the_bore() {
        // A 20 mm box with a 10 mm cavity in its middle wound inward, and the hole down
        // through the top wall into it.
        let mut shell = cuboid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(20.0, 20.0, 20.0));
        let outer_faces = shell.faces.len();
        let cavity = cuboid(Vec3::new(5.0, 5.0, 5.0), Vec3::new(15.0, 15.0, 15.0));
        let offset = shell.vertices.len() as u32;
        shell.vertices.extend(&cavity.vertices);
        shell.faces.extend(
            cavity
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
        );
        let hole = DrainHole {
            at: Vec3::new(10.0, 10.0, 20.0),
            depth_mm: 8.0,
            ..square_hole_through()
        };
        let cuts = drill(&[hole], &[]).expect("the hole drills");
        let cut = Drilled::of(&shell, Some(outer_faces..shell.faces.len()), &cuts)
            .expect("a box with a cavity is a solid");
        let range = cut.cavity.expect("the cavity is carried over");
        let cavity_mesh = Mesh::new(cut.mesh.vertices.clone(), cut.mesh.faces[range].to_vec());
        // The cavity's faces less the 4 mm2 of roof the bore opened at z = 15, and no bore
        // wall: the open roof counts the 1000 mm3 short by 4 * 15 / 3 = 20.
        let volume = -signed_volume(&cavity_mesh);
        assert!(
            (volume - 980.0).abs() < 1e-2,
            "only the cavity's own faces, open where the bore meets it, got {volume}"
        );
    }

    #[test]
    fn cuts_not_laid_as_drill_lays_them_are_refused() {
        let block = cuboid(Vec3::new(9.0, 9.0, 4.0), Vec3::new(11.0, 11.0, 12.0));
        assert_eq!(drilled(&slab(), &block), Err(VolumeError::NotCuts));
    }

    #[test]
    fn only_the_pieces_a_cut_reaches_are_drilled() {
        let mut two = slab();
        two.append(&cuboid(
            Vec3::new(40.0, 0.0, 0.0),
            Vec3::new(50.0, 10.0, 10.0),
        ));
        let cuts = drill(&[square_hole_through()], &[]).expect("the hole drills");
        let left = drilled_pieces(&two, &cuts);
        let volume = signed_volume(&left);
        // The slab loses its 40 mm3 bore; the 1000 mm3 block beside it is untouched.
        assert!(
            (volume - (3960.0 + 1000.0)).abs() < 1e-2,
            "only the slab is drilled, got {volume}"
        );
    }

    #[test]
    fn a_piece_between_two_holes_is_not_drilled() {
        let far = DrainHole {
            at: Vec3::new(70.0, 10.0, 10.0),
            ..square_hole_through()
        };
        let mut three = slab();
        three.append(&cuboid(
            Vec3::new(60.0, 0.0, 0.0),
            Vec3::new(80.0, 20.0, 10.0),
        ));
        let between = cuboid(Vec3::new(30.0, 5.0, 2.0), Vec3::new(50.0, 15.0, 8.0));
        three.append(&between);
        let cuts = drill(&[square_hole_through(), far], &[]).expect("the holes drill");
        let left = drilled_pieces(&three, &cuts);
        let block = split(&between);
        assert!(
            split(&left).iter().any(|piece| block.contains(piece)),
            "the block in the box round both holes, but in neither, comes back as it went in"
        );
        assert!(
            (signed_volume(&left) - (2.0 * 3960.0 + 1200.0)).abs() < 1e-2,
            "each slab loses its 40 mm3 bore, the 1200 mm3 block nothing"
        );
    }
}
