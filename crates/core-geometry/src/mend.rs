use crate::{
    Filled, Mesh, MeshDiagnostics, Orientation, collapse_flat_faces, diagnose, fill_holes,
    orient_outward, remove_duplicate_faces,
};

/// What mending one mesh changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mended {
    pub filled: Filled,
    /// Faces that were lying over another face and have gone.
    pub duplicates_removed: usize,
    /// What the patches had to be turned round, since a new face can come out either way.
    pub orientation: Orientation,
    /// The repair would have left more wrong than it found, so the mesh is as it came.
    pub kept_as_found: bool,
}

/// Collapses the faces with no area, patches the holes and drops the faces laid over
/// others, and turns the patches the right way out; leaves the mesh as it came if that
/// would leave more wrong with it, or nothing of it at all.
///
/// A face with no area goes first, by welding the corners it has on one point. The doubled
/// faces go next, since one reads as a branch the boundary walk cannot get
/// through, and again after the filling: a patch over a lone face or a fin is its reverse,
/// and the two cancel. The orientation pass is last because a patch is new surface and
/// nothing has decided its side yet. See `docs/design/mesh-repair.md`.
pub fn mend(mesh: &mut Mesh) -> Mended {
    let found = mesh.clone();
    let before = wrong(&diagnose(mesh));

    collapse_flat_faces(mesh);
    let mut duplicates_removed = remove_duplicate_faces(mesh).len();
    orient_outward(mesh);
    let filled = fill_holes(mesh);
    duplicates_removed += remove_duplicate_faces(mesh).len();
    let orientation = orient_outward(mesh);

    // A sheet closed by its own reverse cancels away; a model that is nothing else would
    // vanish from the plate rather than be repaired.
    let kept_as_found = mesh.faces.is_empty() || wrong(&diagnose(mesh)) > before;
    if kept_as_found {
        *mesh = found;
    }
    Mended {
        filled,
        duplicates_removed,
        orientation,
        kept_as_found,
    }
}

/// Everything that keeps a mesh from being sound, counted together.
fn wrong(diagnostics: &MeshDiagnostics) -> usize {
    diagnostics.boundary_edges + diagnostics.degenerate_faces + diagnostics.duplicate_faces
}

#[cfg(test)]
mod tests;
