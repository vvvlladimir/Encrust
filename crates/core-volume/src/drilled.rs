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
    /// Which faces of `mesh` bound the cavity, as `Shell::cavity` names them for the shell;
    /// its infill follows it to the end, as in the shell.
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

/// `drilled` laid out as `source` is: its own faces, then those on `source`'s `cavity`, then
/// those on the infill after it; with the cavity's range.
fn cavity_last(drilled: Mesh, source: &Mesh, cavity: Range<usize>) -> (Mesh, Range<usize>) {
    let bvh = Bvh::build(source);
    let part: Vec<u8> = drilled
        .faces
        .par_iter()
        .map(|face| {
            let middle = face
                .iter()
                .map(|corner| drilled.vertices[*corner as usize])
                .sum::<core_geometry::Vec3>()
                / 3.0;
            match bvh.closest_within(middle, ON_CAVITY_MM) {
                Some(on) if on.face >= cavity.end => 2,
                Some(on) if on.face >= cavity.start => 1,
                _ => 0,
            }
        })
        .collect();
    let mut faces = Vec::with_capacity(drilled.faces.len());
    let mut ends = [0; 3];
    for (wanted, end) in ends.iter_mut().enumerate() {
        faces.extend(
            drilled
                .faces
                .iter()
                .zip(&part)
                .filter(|(_, part)| usize::from(**part) == wanted)
                .map(|(face, _)| *face),
        );
        *end = faces.len();
    }
    (Mesh::new(drilled.vertices, faces), ends[0]..ends[1])
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
mod tests;
