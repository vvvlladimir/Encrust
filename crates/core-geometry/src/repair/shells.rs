use crate::topology::{edge_groups, edge_uses, shells};
use crate::{Mesh, Scalar};

/// Smallest area a shell that encloses nothing keeps, mm²; under it a shell is a speck.
pub const DEFAULT_SMALLEST_SHELL_MM2: Scalar = 1.0;

/// One shell of a mesh: the faces joined to each other across shared edges.
#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    /// Its faces, ascending.
    pub faces: Vec<u32>,
    /// The faces' area together, mm².
    pub area_mm2: Scalar,
    /// Whether every edge of it pairs off, so it encloses something.
    pub closed: bool,
}

/// The shells of a mesh in the order of their first face, which is the order the window
/// lists them in and `remove_shells` counts them by.
pub fn list_shells(mesh: &Mesh) -> Vec<Shell> {
    if mesh.faces.is_empty() {
        return Vec::new();
    }
    let uses = edge_uses(mesh);
    let (labels, count) = shells(mesh.faces.len(), &uses).labels();
    let mut listed = vec![
        Shell {
            faces: Vec::new(),
            area_mm2: 0.0,
            closed: true,
        };
        count
    ];
    for group in edge_groups(&uses).filter(|group| group.len() % 2 == 1) {
        for edge_use in group {
            listed[labels[edge_use.face as usize] as usize].closed = false;
        }
    }
    for (face, &label) in labels.iter().enumerate() {
        let shell = &mut listed[label as usize];
        shell.faces.push(face as u32);
        shell.area_mm2 += mesh.triangle(face).map_or(0.0, |triangle| triangle.area());
    }
    listed
}

/// Drops the shells numbered `which` in [`list_shells`] order, and returns how many went.
/// A number past the last shell drops nothing.
pub fn remove_shells(mesh: &mut Mesh, which: &[usize]) -> usize {
    let listed = list_shells(mesh);
    let going: Vec<&Shell> = which.iter().filter_map(|&at| listed.get(at)).collect();
    drop_faces_of(mesh, &going);
    going.len()
}

/// Drops every shell that encloses nothing and whose faces add up to under `smallest_mm2`,
/// and returns how many went. A closed shell stays however small: it may be a real pin.
pub fn drop_small_shells(mesh: &mut Mesh, smallest_mm2: Scalar) -> usize {
    if smallest_mm2 <= 0.0 {
        return 0;
    }
    let listed = list_shells(mesh);
    let specks: Vec<&Shell> = listed
        .iter()
        .filter(|shell| !shell.closed && shell.area_mm2 < smallest_mm2)
        .collect();
    drop_faces_of(mesh, &specks);
    specks.len()
}

fn drop_faces_of(mesh: &mut Mesh, going: &[&Shell]) {
    if going.is_empty() {
        return;
    }
    let mut dropped = vec![false; mesh.faces.len()];
    for face in going.iter().flat_map(|shell| &shell.faces) {
        dropped[*face as usize] = true;
    }
    let mut at = 0;
    mesh.faces.retain(|_| {
        let keep = !dropped[at];
        at += 1;
        keep
    });
}

#[cfg(test)]
mod tests;
