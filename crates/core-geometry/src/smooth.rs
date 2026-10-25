use rayon::iter::{IntoParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
use rayon::slice::ParallelSlice;

use crate::topology::{EdgeUse, edge_uses};
use crate::{Components, Mesh, Scalar, Vec3};

/// Normals at the corners of a mesh, each shared by the faces round its vertex that no
/// crease parts from it: what shades a faceted surface as the smooth one it stands for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SmoothNormals {
    /// For corner `slot` of face `face`, at `face * 3 + slot`, its index into `normals`;
    /// `u32::MAX` on a face of no area, which has no way to face.
    pub corners: Vec<u32>,
    /// One unit normal for each run of faces round a vertex joined by smooth edges.
    pub normals: Vec<Vec3>,
    /// The mesh vertex each of `normals` stands at.
    pub vertices: Vec<u32>,
}

/// The normals of `mesh` smoothed across every edge whose two faces turn less than
/// `crease_deg` from each other, each face counted by its angle at the corner.
pub fn smooth_normals(mesh: &Mesh, crease_deg: Scalar) -> SmoothNormals {
    let facing: Vec<Option<Facing>> = (0..mesh.faces.len())
        .into_par_iter()
        .map(|face| Facing::of(mesh, face))
        .collect();
    let crease = crease_deg.to_radians().cos();
    let uses = edge_uses(mesh);
    let mut runs = Components::joined(
        mesh.faces.len() * 3,
        uses.par_chunk_by(|one, other| one.edge == other.edge)
            .flat_map_iter(|edge| joins(mesh, &facing, crease, edge).into_iter().flatten()),
    );
    let (runs, count) = runs.labels();
    gathered(mesh, &facing, &runs, count)
}

/// A face's unit normal and its angle at each corner, radians.
struct Facing {
    normal: Vec3,
    angles: [Scalar; 3],
}

impl Facing {
    fn of(mesh: &Mesh, face: usize) -> Option<Self> {
        let triangle = mesh.triangle(face)?;
        let normal = triangle.normal_unnormalized().normalize_or_zero();
        if normal == Vec3::ZERO {
            return None;
        }
        let corners = [triangle.a, triangle.b, triangle.c];
        let angles = std::array::from_fn(|slot| {
            let at = corners[slot];
            let angle = (corners[(slot + 1) % 3] - at).angle_between(corners[(slot + 2) % 3] - at);
            if angle.is_finite() { angle } else { 0.0 }
        });
        Some(Self { normal, angles })
    }
}

/// The corners an edge joins at each of its ends: those of its two faces, when it has two
/// with area and they turn less than the crease. An edge of three faces or more stays sharp.
fn joins(
    mesh: &Mesh,
    facing: &[Option<Facing>],
    crease: Scalar,
    edge: &[EdgeUse],
) -> Option<[[u32; 2]; 2]> {
    let mut kept = edge
        .iter()
        .filter(|edge_use| facing[edge_use.face as usize].is_some());
    let (Some(one), Some(other), None) = (kept.next(), kept.next(), kept.next()) else {
        return None;
    };
    let normal = |edge_use: &EdgeUse| facing[edge_use.face as usize].as_ref().map(|f| f.normal);
    if normal(one)?.dot(normal(other)?) < crease {
        return None;
    }
    let (from, to) = one.edge;
    Some([
        [
            corner(mesh, one.face, from)?,
            corner(mesh, other.face, from)?,
        ],
        [corner(mesh, one.face, to)?, corner(mesh, other.face, to)?],
    ])
}

/// The corner of face `face` standing on vertex `vertex`, as `face * 3 + slot`.
fn corner(mesh: &Mesh, face: u32, vertex: u32) -> Option<u32> {
    let slot = mesh.faces[face as usize]
        .iter()
        .position(|&at| at == vertex)?;
    Some(face * 3 + slot as u32)
}

/// One normal per run of corners, numbered as the faces first reach them, summed from the
/// faces of the run and made unit.
fn gathered(mesh: &Mesh, facing: &[Option<Facing>], runs: &[u32], count: usize) -> SmoothNormals {
    let mut index = vec![u32::MAX; count];
    let mut smooth = SmoothNormals {
        corners: vec![u32::MAX; runs.len()],
        ..SmoothNormals::default()
    };
    for (face, facing) in facing.iter().enumerate() {
        let Some(facing) = facing else {
            continue;
        };
        for slot in 0..3 {
            let corner = face * 3 + slot;
            let at = &mut index[runs[corner] as usize];
            if *at == u32::MAX {
                *at = smooth.normals.len() as u32;
                smooth.normals.push(Vec3::ZERO);
                smooth.vertices.push(mesh.faces[face][slot]);
            }
            smooth.normals[*at as usize] += facing.normal * facing.angles[slot];
            smooth.corners[corner] = *at;
        }
    }
    smooth
        .normals
        .par_iter_mut()
        .for_each(|normal| *normal = normal.normalize_or(Vec3::Z));
    smooth
}

#[cfg(test)]
mod tests;
