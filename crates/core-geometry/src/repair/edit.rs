//! Changes made to a mesh by hand: the faces picked turned or dropped, and two open edges
//! bridged by a pair of faces.

use thiserror::Error;

use crate::topology::{edge_groups, edge_uses};
use crate::{Mesh, Region, Triangle};

/// Why two edges could not be bridged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BridgeError {
    /// The edge, as its two vertices, is not used by exactly one face.
    #[error("edge {0:?} is not an open edge")]
    NotOpen([u32; 2]),
    #[error("an edge cannot be bridged to itself")]
    SameEdge,
    /// The faces bridging them would have no area: the two edges lie on one line.
    #[error("the edges lie on one line, so nothing spans them")]
    Degenerate,
}

/// Turns every face `faces` marks to face the other way, and returns how many turned.
pub fn flip_faces(mesh: &mut Mesh, faces: &Region) -> usize {
    let mut turned = 0;
    for face in faces.faces() {
        if let Some(corners) = mesh.faces.get_mut(face) {
            corners.swap(1, 2);
            turned += 1;
        }
    }
    turned
}

/// Drops every face `faces` marks, and returns how many went. The vertices stay, so the
/// remaining faces keep their corners.
pub fn remove_faces(mesh: &mut Mesh, faces: &Region) -> usize {
    let before = mesh.faces.len();
    let mut at = 0;
    mesh.faces.retain(|_| {
        let keep = !faces.contains(at);
        at += 1;
        keep
    });
    before - mesh.faces.len()
}

/// Lays faces between the open edges `a` and `b`, each as its two vertices in either order,
/// wound as the faces beside them (ADR 0356); returns how many it added.
pub fn bridge(mesh: &mut Mesh, a: [u32; 2], b: [u32; 2]) -> Result<usize, BridgeError> {
    let key = |[p, q]: [u32; 2]| [p.min(q), p.max(q)];
    if key(a) == key(b) {
        return Err(BridgeError::SameEdge);
    }
    let [a0, a1] = wanting(mesh, a)?;
    let [b0, b1] = wanting(mesh, b)?;
    let faces: Vec<[u32; 3]> = if a1 == b0 {
        vec![[a0, a1, b1]]
    } else if b1 == a0 {
        vec![[b0, b1, a1]]
    } else {
        let first = [[a0, a1, b0], [b0, b1, a0]];
        let other = [[a0, a1, b1], [a1, b0, b1]];
        let area =
            |faces: &[[u32; 3]; 2]| faces.iter().map(|face| area_of(mesh, *face)).sum::<f32>();
        let chosen = if area(&first) <= area(&other) {
            first
        } else {
            other
        };
        chosen.to_vec()
    };
    if faces
        .iter()
        .any(|face| area_of(mesh, *face) <= f32::EPSILON)
    {
        return Err(BridgeError::Degenerate);
    }
    mesh.faces.extend(&faces);
    Ok(faces.len())
}

/// The open edge `edge` in the direction a face laid against it must walk it: the way
/// round its one face does not.
fn wanting(mesh: &Mesh, edge: [u32; 2]) -> Result<[u32; 2], BridgeError> {
    let (low, high) = (edge[0].min(edge[1]), edge[0].max(edge[1]));
    let uses = edge_uses(mesh);
    let group = edge_groups(&uses).find(|group| group[0].edge == (low, high));
    match group {
        Some([only]) if only.forward => Ok([high, low]),
        Some([_]) => Ok([low, high]),
        _ => Err(BridgeError::NotOpen(edge)),
    }
}

fn area_of(mesh: &Mesh, [a, b, c]: [u32; 3]) -> f32 {
    let at = |corner: u32| mesh.vertices[corner as usize];
    Triangle::new(at(a), at(b), at(c)).area()
}

#[cfg(test)]
mod tests;
