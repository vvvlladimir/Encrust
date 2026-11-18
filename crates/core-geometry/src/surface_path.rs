//! The shortest walk over a mesh's surface between two of its points: a corridor of faces
//! by A*, unfolded flat and pulled taut. See `docs/design/surface-path.md` and ADR 0400.

mod corridor;
mod funnel;
mod unfold;

use std::ops::ControlFlow;

use thiserror::Error;

use crate::{Adjacency, Mat4, Mesh, Scalar, Vec3};

/// A point on a mesh's surface: the face it lies on, and where, in placed coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfacePoint {
    pub face: usize,
    pub point: Vec3,
}

/// A walk over a surface between two of its points, as a polyline lying on it.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfacePath {
    /// From the first point to the second, through every edge crossed, placed coordinates.
    pub points: Vec<Vec3>,
}

impl SurfacePath {
    /// How long the walk is, in millimetres.
    pub fn length_mm(&self) -> Scalar {
        self.points
            .windows(2)
            .map(|pair| pair[0].distance(pair[1]))
            .sum()
    }

    /// The point half its length along the walk; `None` for a walk with no points.
    pub fn halfway(&self) -> Option<Vec3> {
        let mut left_mm = self.length_mm() / 2.0;
        for pair in self.points.windows(2) {
            let step_mm = pair[0].distance(pair[1]);
            if step_mm >= left_mm && step_mm > 0.0 {
                return Some(pair[0].lerp(pair[1], left_mm / step_mm));
            }
            left_mm -= step_mm;
        }
        self.points.last().copied()
    }
}

/// Why no walk was found between two points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SurfacePathError {
    #[error("face {face} is not one of the mesh's {faces} faces")]
    FaceOutOfRange { face: usize, faces: usize },
    #[error("faces {from} and {to} lie on surfaces no shared edge joins")]
    Unreachable { from: usize, to: usize },
    #[error("the walk was given up on before it was found")]
    Stopped,
}

/// Told now and then how far a walk has got, from 0 to 1; answering `Break` gives it up.
pub type WalkWatch<'a> = &'a mut dyn FnMut(Scalar) -> ControlFlow<()>;

/// The shortest walk over `mesh`, placed by `placement`, from `from` to `to` within the
/// corridor of faces A* finds: a little long where the true one leaves that corridor.
pub fn surface_path(
    mesh: &Mesh,
    adjacency: &Adjacency,
    placement: Mat4,
    from: SurfacePoint,
    to: SurfacePoint,
    watch: WalkWatch,
) -> Result<SurfacePath, SurfacePathError> {
    let faces = mesh.faces.len();
    if let Some(face) = [from.face, to.face].into_iter().find(|face| *face >= faces) {
        return Err(SurfacePathError::FaceOutOfRange { face, faces });
    }
    let unreachable = SurfacePathError::Unreachable {
        from: from.face,
        to: to.face,
    };
    if !adjacency.joins(from.face, to.face) {
        return Err(unreachable);
    }
    let corridor = corridor::find((mesh, placement), adjacency, (from, to), watch)?;
    let strip = unfold::unfold(&corridor, from.point, to.point).ok_or(unreachable)?;
    let points = std::iter::once(from.point)
        .chain(funnel::crossings(&strip))
        .chain(std::iter::once(to.point))
        .collect();
    Ok(SurfacePath { points })
}

/// One face of a corridor: its corners' indices and where they stand.
struct Face {
    ids: [u32; 3],
    at: [Vec3; 3],
}

impl Face {
    /// Face `face` of `mesh` placed by `placement`; `None` for a face or corner out of range.
    fn placed(mesh: &Mesh, placement: Mat4, face: usize) -> Option<Self> {
        let ids = *mesh.faces.get(face)?;
        let corner = |id: u32| {
            let vertex = mesh.vertices.get(id as usize)?;
            Some(placement.transform_point3(*vertex))
        };
        Some(Self {
            ids,
            at: [corner(ids[0])?, corner(ids[1])?, corner(ids[2])?],
        })
    }

    /// Where the corner with index `id` stands, if it is one of this face's.
    fn corner(&self, id: u32) -> Option<Vec3> {
        self.ids
            .iter()
            .zip(self.at)
            .find_map(|(other, at)| (*other == id).then_some(at))
    }
}

/// The edge two faces share, as its two corners, and the corner of each face off it.
struct Shared {
    edge: [u32; 2],
    apex_of_first: u32,
    apex_of_second: u32,
}

fn shared(first: [u32; 3], second: [u32; 3]) -> Option<Shared> {
    let mut edge = first.into_iter().filter(|id| second.contains(id));
    let edge = [edge.next()?, edge.next()?];
    let off = |ids: [u32; 3]| ids.into_iter().find(|id| !edge.contains(id));
    Some(Shared {
        edge,
        apex_of_first: off(first)?,
        apex_of_second: off(second)?,
    })
}

#[cfg(test)]
mod tests;
