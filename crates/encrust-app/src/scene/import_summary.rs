use crate::i18n::tr;
use core_geometry::{
    Bvh, Heightmap, Mesh, MeshDiagnostics, Orientation, Transform, UvMap, Vec3, Welded,
    center_of_mass,
};
use std::sync::Arc;

/// What mesh repair had to do to an imported model, kept so the Scene panel can show it.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportSummary {
    pub vertices_merged: usize,
    pub faces_removed: usize,
    pub orientation: Orientation,
    pub diagnostics: MeshDiagnostics,
}

impl ImportSummary {
    /// `doubled` is how many faces laid over others went after welding.
    pub fn new(
        welded: &Welded,
        doubled: usize,
        orientation: Orientation,
        diagnostics: MeshDiagnostics,
    ) -> Self {
        Self {
            vertices_merged: welded.vertices_merged,
            faces_removed: welded.faces_removed() + doubled,
            orientation,
            diagnostics,
        }
    }

    /// The summary of a mesh a tool made rather than a file brought: nothing was welded,
    /// dropped or turned on its way in.
    pub fn made(diagnostics: MeshDiagnostics) -> Self {
        Self {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics,
        }
    }

    /// Nothing about this model would make slicing produce garbage.
    pub fn is_sound(&self) -> bool {
        self.diagnostics.is_sound()
    }

    /// What is still wrong with the mesh, in the words the window says it in. Empty for a
    /// model that will slice as it stands.
    pub fn defects(&self) -> Vec<String> {
        let diagnostics = &self.diagnostics;
        let mut lines = Vec::new();
        if diagnostics.boundary_edges > 0 {
            lines.push(tr!("defect-open", count = diagnostics.boundary_edges));
        }
        if diagnostics.degenerate_faces > 0 {
            lines.push(tr!(
                "defect-degenerate",
                count = diagnostics.degenerate_faces
            ));
        }
        if diagnostics.duplicate_faces > 0 {
            lines.push(tr!("defect-duplicate", count = diagnostics.duplicate_faces));
        }
        lines
    }

    /// What is odd about the mesh but slices as it stands, so is said without asking.
    pub fn notes(&self) -> Vec<String> {
        let folded = self.diagnostics.folded_edges();
        if folded == 0 {
            return Vec::new();
        }
        vec![tr!("note-folded", count = folded)]
    }

    /// What repair changed on the way in.
    ///
    /// Merged vertices are not among them: an STL stores every triangle on its own, so
    /// every STL merges thousands and it says nothing about the model.
    pub fn repairs(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.faces_removed > 0 {
            lines.push(tr!("repaired-faces-removed", count = self.faces_removed));
        }
        if self.orientation.flipped_faces > 0 {
            lines.push(tr!(
                "repaired-flipped",
                count = self.orientation.flipped_faces
            ));
        }
        if self.orientation.inverted_shells > 0 {
            lines.push(tr!(
                "repaired-inverted",
                count = self.orientation.inverted_shells
            ));
        }
        lines
    }
}

/// The texture an imported file carried, kept beside the model until a tool presses it in.
///
/// Both halves are in the model's own space and indexed by its faces, so they hold only
/// while the mesh does: pressing the relief replaces the mesh, and the map goes with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapped {
    /// Where each image came from, for the panel to name.
    pub names: Vec<String>,
    pub uvs: UvMap,
    /// One per image the map names, in the order it indexes them: a model built from
    /// materials textures each part from its own.
    pub heights: Vec<Heightmap>,
}

/// A model that has been loaded, repaired and placed, ready to go into the scene.
///
/// The hierarchy is built here, in [`Imported::new`], and this is the only way a mesh
/// reaches a [`SceneObject`]: the two can therefore never be out of step. Building one is
/// the expensive half of an import, and it happens on the worker thread that made it.
#[derive(Debug, Clone)]
pub struct Imported {
    pub name: String,
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    pub transform: Transform,
    pub summary: ImportSummary,
    /// Where the model's mass sits in its own space, for the gizmo to stand on.
    pub center_of_mass: Vec3,
    /// The texture the file carried, if it carried one.
    pub mapped: Option<Arc<Mapped>>,
}

impl Imported {
    pub fn new(
        name: String,
        mesh: Arc<Mesh>,
        transform: Transform,
        summary: ImportSummary,
    ) -> Self {
        Self {
            bvh: Arc::new(Bvh::build(&mesh)),
            center_of_mass: center_of_mass(&mesh).unwrap_or(Vec3::ZERO),
            name,
            mesh,
            transform,
            summary,
            mapped: None,
        }
    }
}
