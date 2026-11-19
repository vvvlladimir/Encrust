use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::project::{
    BuiltCavity, HollowState, Manifest, ModelMeshes, ObjectHollowState, ObjectState,
    ObjectSupportState, Project, Summary, assign_ids,
};
use core_geometry::{Mesh, Orientation, Region, Transform, diagnose};

use core_volume::{Channel, DrainHole};

use crate::piece::{Piece, Role};

/// A model as imported and placed: its name, its mesh and what import found in it.
#[derive(Debug)]
pub struct ImportedModel {
    pub name: String,
    pub mesh: Mesh,
    pub summary: Summary,
}

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

    /// The plate of `models` as imported and placed, on `manifest`'s first plate, ids from 1.
    pub fn of_models(mut manifest: Manifest, models: Vec<ImportedModel>) -> Self {
        let mut meshes = Vec::with_capacity(models.len());
        manifest.objects.clear();
        for (index, model) in models.into_iter().enumerate() {
            let id = ObjectId(index as u64 + 1);
            let state = bare_object(id, model.name, 0, Transform::default(), model.summary);
            manifest.objects.push(state);
            meshes.push(ModelMeshes {
                source: Arc::new(model.mesh),
                shell: None,
            });
        }
        let project = Project {
            manifest,
            models: meshes,
            preview: None,
        };
        Self::from_project(project)
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
        let (old, _) = self.take_at(index);
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
            self.put_at(index + offset, state, meshes);
        }
        added
    }

    /// Takes object `id` off the plate with the plugs laid for it, answering every id taken,
    /// the object's first; empty when it is not on the plate.
    pub(crate) fn remove(&mut self, id: ObjectId) -> Vec<ObjectId> {
        if self.index_of(id).is_none() {
            return Vec::new();
        }
        let mut gone = vec![id];
        gone.extend(self.take_plugs_of(id));
        if let Some(index) = self.index_of(id) {
            self.take_at(index);
        }
        gone
    }

    /// Gives object `id` the surface `mesh`, taking off its supports, cavity and painted
    /// regions; its plugs stay. Nothing changes when `id` is not on the plate.
    pub(crate) fn reshape(&mut self, id: ObjectId, mesh: Mesh) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let state = &mut self.project.manifest.objects[index];
        state.summary.diagnostics = diagnose(&mesh);
        state.supports = ObjectSupportState::default();
        state.hollow = ObjectHollowState::default();
        state.relief_painted = Region::default();
        state.shown_painted = Region::default();
        self.project.models[index] = ModelMeshes {
            source: Arc::new(mesh),
            shell: None,
        };
    }

    /// Stands `shell` on object `id`, built as `built` records and asked for by `settings`,
    /// which the object keeps as its own; supports stay. Nothing changes when `id` is not on
    /// the plate.
    pub(crate) fn set_shell(
        &mut self,
        id: ObjectId,
        built: BuiltCavity,
        shell: Arc<Mesh>,
        settings: HollowState,
    ) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let state = &mut self.project.manifest.objects[index];
        state.hollow.built = Some(built);
        state.own_hollow = Some(settings);
        self.project.models[index].shell = Some(shell);
    }

    /// Gives object `id` the drain holes and channels it is cut with; the shell stays as built.
    /// Nothing changes when `id` is not on the plate.
    pub(crate) fn set_cuts(
        &mut self,
        id: ObjectId,
        drains: Vec<DrainHole>,
        channels: Vec<Channel>,
    ) {
        if let Some(index) = self.index_of(id) {
            let state = &mut self.project.manifest.objects[index].hollow;
            state.drains = drains;
            state.channels = channels;
        }
    }

    /// Takes the plugs laid for object `id` off the plate, answering their ids.
    fn take_plugs_of(&mut self, id: ObjectId) -> Vec<ObjectId> {
        let named = self
            .index_of(id)
            .and_then(|index| u32::try_from(index).ok());
        let plugs: Vec<ObjectId> = (self.project.manifest.objects.iter())
            .filter(|object| object.plug_of.is_some() && object.plug_of == named)
            .map(|object| object.id)
            .collect();
        for plug in &plugs {
            if let Some(index) = self.index_of(*plug) {
                self.take_at(index);
            }
        }
        plugs
    }

    /// Puts a copy of object `id` and its meshes right after it under a new id, `changed`
    /// first; answers the new id, or `None` when `id` is not on the plate.
    pub(crate) fn copy_after(
        &mut self,
        id: ObjectId,
        changed: impl FnOnce(&mut ObjectState),
    ) -> Option<ObjectId> {
        let index = self.index_of(id)?;
        let mut state = self.project.manifest.objects[index].clone();
        let meshes = self.project.models[index].clone();
        state.id = ObjectId(self.next_id);
        self.next_id += 1;
        changed(&mut state);
        let copy = state.id;
        self.put_at(index + 1, state, meshes);
        Some(copy)
    }

    /// Stands object `id` at `transform`; nothing changes when it is not on the plate.
    pub(crate) fn set_transform(&mut self, id: ObjectId, transform: Transform) {
        if let Some(index) = self.index_of(id) {
            self.project.manifest.objects[index].transform = transform;
        }
    }

    /// Takes the object at `index` off the plate, and with it every plug's claim to it; a
    /// plug naming a later object is renumbered.
    fn take_at(&mut self, index: usize) -> (ObjectState, ModelMeshes) {
        let state = self.project.manifest.objects.remove(index);
        let meshes = self.project.models.remove(index);
        let gone = u32::try_from(index).ok();
        for object in &mut self.project.manifest.objects {
            object.plug_of = match object.plug_of {
                Some(at) if Some(at) == gone => None,
                Some(at) if Some(at) > gone => Some(at - 1),
                other => other,
            };
        }
        (state, meshes)
    }

    /// Stands an object at `index`, renumbering each plug that names one from there on.
    fn put_at(&mut self, index: usize, state: ObjectState, meshes: ModelMeshes) {
        let from = u32::try_from(index).unwrap_or(u32::MAX);
        for object in &mut self.project.manifest.objects {
            object.plug_of = object
                .plug_of
                .map(|at| if at >= from { at + 1 } else { at });
        }
        self.project.manifest.objects.insert(index, state);
        self.project.models.insert(index, meshes);
    }

    fn index_of(&self, id: ObjectId) -> Option<usize> {
        (self.project.manifest.objects.iter()).position(|object| object.id == id)
    }
}

/// A piece made from `old` as an object of its own: on its plate, named after it, with no
/// supports, cavity or settings of its own.
fn new_object(id: ObjectId, old: &ObjectState, piece: &Piece) -> ObjectState {
    let summary = Summary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: piece.diagnostics.clone(),
    };
    let name = name_of(&old.name, piece.role);
    bare_object(id, name, old.plate, piece.transform, summary)
}

/// An object on `plate` with no supports, cavity or settings of its own.
fn bare_object(
    id: ObjectId,
    name: String,
    plate: u32,
    transform: Transform,
    summary: Summary,
) -> ObjectState {
    ObjectState {
        id,
        name,
        plate,
        transform,
        visible: true,
        summary,
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
        Role::Kept | Role::Copy | Role::Combined => name.to_owned(),
        Role::Lower => format!("{name} (lower)"),
        Role::Upper => format!("{name} (upper)"),
        Role::Dowel(index) => format!("{name} dowel {index}"),
        Role::Part(index) => format!("{name} (part {index})"),
    }
}

#[cfg(test)]
mod tests;
