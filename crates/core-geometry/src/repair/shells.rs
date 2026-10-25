use crate::topology::{edge_groups, edge_uses, shells};
use crate::{Mesh, Scalar};

/// Smallest area a shell that encloses nothing keeps, mm²; under it a shell is a speck.
pub const DEFAULT_SMALLEST_SHELL_MM2: Scalar = 1.0;

/// Drops every shell that encloses nothing and whose faces add up to under `smallest_mm2`,
/// and returns how many went. A closed shell stays however small: it may be a real pin.
pub fn drop_small_shells(mesh: &mut Mesh, smallest_mm2: Scalar) -> usize {
    if mesh.faces.is_empty() || smallest_mm2 <= 0.0 {
        return 0;
    }
    let uses = edge_uses(mesh);
    let (labels, count) = shells(mesh.faces.len(), &uses).labels();
    let mut open = vec![false; count];
    for group in edge_groups(&uses).filter(|group| group.len() % 2 == 1) {
        for edge_use in group {
            open[labels[edge_use.face as usize] as usize] = true;
        }
    }
    let mut area_mm2 = vec![0.0; count];
    for (face, &label) in labels.iter().enumerate() {
        area_mm2[label as usize] += mesh.triangle(face).map_or(0.0, |triangle| triangle.area());
    }
    let speck: Vec<bool> = (0..count)
        .map(|shell| open[shell] && area_mm2[shell] < smallest_mm2)
        .collect();
    let dropped = speck.iter().filter(|&&speck| speck).count();
    if dropped > 0 {
        let mut at = 0;
        mesh.faces.retain(|_| {
            let keep = !speck[labels[at] as usize];
            at += 1;
            keep
        });
    }
    dropped
}

#[cfg(test)]
mod tests;
