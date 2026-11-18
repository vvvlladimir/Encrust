use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::project::{
    ModelMeshes, ObjectHollowState, ObjectState, ObjectSupportState, Project, Summary, assign_ids,
};
use core_geometry::{Orientation, Region};

use crate::piece::{Piece, Role};

/// The plate as `.encrust` holds it, every object with an id of its own.
#[derive(Debug)]
pub struct Document {
    project: Project,
    /// The id the next object made takes, above every one the document has held.
    next_id: u64,
}

impl Document {
    /// The plate `project` holds, an id given to each object that has none or a taken one.
    pub fn from_project(mut project: Project) -> Self {
        assign_ids(&mut project.manifest.objects);
        let next_id = (project.manifest.objects.iter())
            .map(|object| object.id.0 + 1)
            .max()
            .unwrap_or(1);
        Self { project, next_id }
    }

    /// The plate as a project file holds it, ids and all.
    pub fn into_project(self) -> Project {
        self.project
    }

    /// The object `id` names, with its meshes, if it is on the plate.
    pub fn object(&self, id: ObjectId) -> Option<(&ObjectState, &ModelMeshes)> {
        let index = self.index_of(id)?;
        Some((
            &self.project.manifest.objects[index],
            &self.project.models[index],
        ))
    }

    /// Puts `pieces` where object `id` stood, on its plate, each a new object named after
    /// it. Answers their ids; nothing changes when `id` is not on the plate.
    pub(crate) fn replace(&mut self, id: ObjectId, pieces: Vec<Piece>) -> Vec<(ObjectId, Role)> {
        let Some(index) = self.index_of(id) else {
            return Vec::new();
        };
        let old = self.project.manifest.objects.remove(index);
        self.project.models.remove(index);
        let mut added = Vec::with_capacity(pieces.len());
        for (offset, piece) in pieces.into_iter().enumerate() {
            let id = ObjectId(self.next_id);
            self.next_id += 1;
            added.push((id, piece.role));
            let state = new_object(id, &old, &piece);
            let meshes = ModelMeshes {
                source: Arc::new(piece.mesh),
                shell: None,
            };
            self.project.manifest.objects.insert(index + offset, state);
            self.project.models.insert(index + offset, meshes);
        }
        added
    }

    fn index_of(&self, id: ObjectId) -> Option<usize> {
        (self.project.manifest.objects.iter()).position(|object| object.id == id)
    }
}

/// A piece made from `old` as an object of its own: on its plate, named after it, with no
/// supports, cavity or settings of its own.
fn new_object(id: ObjectId, old: &ObjectState, piece: &Piece) -> ObjectState {
    ObjectState {
        id,
        name: name_of(&old.name, piece.role),
        plate: old.plate,
        transform: piece.transform,
        visible: true,
        summary: Summary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: piece.diagnostics.clone(),
        },
        supports: ObjectSupportState::default(),
        hollow: ObjectHollowState::default(),
        own_hollow: None,
        own_drain: None,
        own_supports: Vec::new(),
        own_raft: None,
        plug_of: None,
        relief_painted: Region::default(),
        shown_painted: Region::default(),
    }
}

/// A new object's name in English; the window names it again in the user's language.
fn name_of(name: &str, role: Role) -> String {
    match role {
        Role::Kept => name.to_owned(),
        Role::Lower => format!("{name} (lower)"),
        Role::Upper => format!("{name} (upper)"),
        Role::Dowel(index) => format!("{name} dowel {index}"),
    }
}

#[cfg(test)]
mod tests;
