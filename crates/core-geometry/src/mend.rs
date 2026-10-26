use crate::{
    DEFAULT_SEAM_TOLERANCE, DEFAULT_SMALLEST_SHELL_MM2, Filled, Mesh, MeshDiagnostics, Orientation,
    Scalar, Stitched, collapse_flat_faces, diagnose, drop_small_shells, fill_holes,
    fill_holes_within, orient_outward, remove_duplicate_faces, stitch_seams,
};
use serde::{Deserialize, Serialize};

/// Most corners a hole Close small holes patches: one face missing.
const SMALL_HOLE_CORNERS: usize = 3;

/// Which repair to run on a mesh.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MendMethod {
    /// Every step, in the order [`mend`] gives: what a Repair press runs.
    #[default]
    Repair,
    /// Every hole patched, the doubled faces dropped and the patches turned outward.
    CloseHoles,
    /// Only the holes a single missing face leaves, and nothing turned.
    CloseSmallHoles,
    /// The faces wound the wrong way turned, and nothing added.
    Turn,
}

/// The lengths a repair works to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MendSettings {
    /// How far a vertex may stand off an open edge to be stitched into it, mm.
    pub seam_tolerance_mm: Scalar,
    /// Smallest area a shell enclosing nothing keeps, mm².
    pub smallest_shell_mm2: Scalar,
}

impl Default for MendSettings {
    fn default() -> Self {
        Self {
            seam_tolerance_mm: DEFAULT_SEAM_TOLERANCE,
            smallest_shell_mm2: DEFAULT_SMALLEST_SHELL_MM2,
        }
    }
}

/// What mending one mesh changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mended {
    pub filled: Filled,
    pub stitched: Stitched,
    /// Shells enclosing nothing that were too small to keep.
    pub shells_dropped: usize,
    /// Faces that were lying over another face and have gone.
    pub duplicates_removed: usize,
    /// What the last orientation pass turned; `None` for a method that does not orient.
    pub orientation: Option<Orientation>,
    /// The repair would have left more wrong than it found, so the mesh is as it came.
    pub kept_as_found: bool,
}

impl Mended {
    /// What this mend and `later`, run on the mesh it left, changed together.
    #[must_use]
    pub fn then(self, later: Mended) -> Mended {
        let (first, later) = (self.done(), later.done());
        Mended {
            filled: Filled {
                loops_filled: first.filled.loops_filled + later.filled.loops_filled,
                faces_added: first.filled.faces_added + later.filled.faces_added,
                loops_left: later.filled.loops_left,
            },
            stitched: Stitched {
                edges_split: first.stitched.edges_split + later.stitched.edges_split,
                vertices_inserted: first.stitched.vertices_inserted
                    + later.stitched.vertices_inserted,
            },
            shells_dropped: first.shells_dropped + later.shells_dropped,
            duplicates_removed: first.duplicates_removed + later.duplicates_removed,
            orientation: later.orientation.or(first.orientation),
            kept_as_found: first.kept_as_found && later.kept_as_found,
        }
    }

    /// What stands of this mend: nothing but the holes it found where it was undone.
    fn done(self) -> Mended {
        if !self.kept_as_found {
            return self;
        }
        let filled = Filled {
            loops_left: self.filled.loops_filled + self.filled.loops_left,
            ..Filled::default()
        };
        Mended {
            filled,
            kept_as_found: true,
            ..Mended::default()
        }
    }
}

/// Runs `method` on the mesh; leaves it as it came if that would leave more wrong with it,
/// or nothing of it at all. See `docs/design/mesh-repair.md`.
pub fn mend(mesh: &mut Mesh, method: MendMethod, settings: MendSettings) -> Mended {
    let found = mesh.clone();
    let before = wrong(&diagnose(mesh));

    let mut mended = Mended::default();
    match method {
        MendMethod::Repair => repair(mesh, settings, &mut mended),
        MendMethod::CloseHoles => {
            mended.duplicates_removed = remove_duplicate_faces(mesh).len();
            close_holes(mesh, &mut mended);
        }
        MendMethod::CloseSmallHoles => {
            mended.filled = fill_holes_within(mesh, SMALL_HOLE_CORNERS);
        }
        MendMethod::Turn => mended.orientation = Some(orient_outward(mesh)),
    }

    // A sheet closed by its own reverse cancels away; a model that is nothing else would
    // vanish from the plate rather than be repaired.
    mended.kept_as_found = mesh.faces.is_empty() || wrong(&diagnose(mesh)) > before;
    if mended.kept_as_found {
        *mesh = found;
    }
    mended
}

/// Every step, in the order `docs/design/mesh-repair.md` argues for: collapse, drop the
/// doubled faces, stitch, drop the specks, patch.
fn repair(mesh: &mut Mesh, settings: MendSettings, mended: &mut Mended) {
    collapse_flat_faces(mesh);
    mended.duplicates_removed = remove_duplicate_faces(mesh).len();
    mended.stitched = stitch_seams(mesh, settings.seam_tolerance_mm);
    mended.shells_dropped = drop_small_shells(mesh, settings.smallest_shell_mm2);
    close_holes(mesh, mended);
}

/// Patches every hole, drops a patch that cancels a fin or a lone face, and turns the
/// patches outward, which nothing had decided.
fn close_holes(mesh: &mut Mesh, mended: &mut Mended) {
    orient_outward(mesh);
    mended.filled = fill_holes(mesh);
    mended.duplicates_removed += remove_duplicate_faces(mesh).len();
    mended.orientation = Some(orient_outward(mesh));
}

/// Everything that keeps a mesh from being sound, counted together.
fn wrong(diagnostics: &MeshDiagnostics) -> usize {
    diagnostics.boundary_edges + diagnostics.degenerate_faces + diagnostics.duplicate_faces
}

#[cfg(test)]
mod tests;
