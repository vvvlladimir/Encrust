use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::project::{
    BuiltCavity, Group, HollowState, Manifest, ModelMeshes, ObjectHollowState, ObjectState,
    ObjectSupportState, Project, Summary, SupportState, assign_ids,
};
use core_engine::{Model, object_model, plate_models};
use core_geometry::{Mesh, Orientation, Region, Transform, diagnose};

use core_supports::{SupportPoint, SupportTree};
use core_volume::{Blocker, Channel, DrainHole};

use crate::piece::{Piece, Role};

/// A model as imported and placed: its name, its mesh and what import found in it.
#[derive(Debug)]
pub struct ImportedModel {
    pub name: String,
    pub mesh: Mesh,
    pub summary: Summary,
}

/// Each object an edit made, with what it is to the one it came from.
type Added = Vec<(ObjectId, Role)>;

/// The plate as `.encrust` holds it, every object with an id of its own.
#[derive(Debug, Clone)]
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
            .chain(project.manifest.next_id.map(|next| next.0))
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

    /// Takes `group` up as the group in hand, in place of the plate's group of its name, as
    /// the Supports panel takes up a profile chosen; answers its number.
    pub fn hold_group(&mut self, group: Group) -> u16 {
        let supports = &mut self.project.manifest.supports;
        let named =
            (supports.groups.iter()).position(|held| held.profile.name == group.profile.name);
        let at = if let Some(at) = named {
            supports.groups[at] = group;
            at
        } else {
            supports.groups.push(group);
            supports.groups.len() - 1
        };
        supports.active = at as u16;
        supports.active
    }

    /// The plate as a project file holds it, ids and all, and the id the next object takes.
    pub fn into_project(mut self) -> Project {
        self.project.manifest.next_id = Some(ObjectId(self.next_id));
        self.project
    }

    /// Object `id` as a run cuts it, its supports under it; `None` off the plate.
    pub fn model(&self, id: ObjectId) -> Option<Model> {
        object_model(&self.project, self.index_of(id)?)
    }

    /// The models of the active plate as a run cuts them, in plate order.
    pub fn plate_models(&self) -> Vec<Model> {
        plate_models(&self.project, self.project.manifest.active_plate)
    }

    /// The object `id` names, with its meshes, if it is on the plate.
    pub fn object(&self, id: ObjectId) -> Option<(&ObjectState, &ModelMeshes)> {
        let index = self.index_of(id)?;
        Some((
            &self.project.manifest.objects[index],
            &self.project.models[index],
        ))
    }

    /// Every object's id, in plate order.
    pub fn ids(&self) -> Vec<ObjectId> {
        (self.project.manifest.objects.iter())
            .map(|object| object.id)
            .collect()
    }

    /// Every object of the active plate that is not hidden, in plate order.
    pub(crate) fn printable(&self) -> Vec<ObjectId> {
        let active = self.project.manifest.active_plate;
        (self.project.manifest.objects.iter())
            .filter(|object| object.visible && object.plate == active)
            .map(|object| object.id)
            .collect()
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

    /// Stands `shell` on object `id`, built as `built` records and asked for by `settings`, which
    /// it keeps as its own; supports stay, and nothing changes off the plate.
    pub(crate) fn set_shell(
        &mut self,
        id: ObjectId,
        built: BuiltCavity,
        shell: Arc<Mesh>,
        settings: HollowState,
    ) {
        if let Some(index) = self.index_of(id) {
            self.project.manifest.objects[index].own_hollow = Some(settings);
        }
        self.set_built(id, built, shell);
    }

    /// Stands `shell` on object `id`, built as `built` records; the numbers it keeps as its own
    /// and its supports stay. Nothing changes when `id` is not on the plate.
    pub(crate) fn set_built(&mut self, id: ObjectId, built: BuiltCavity, shell: Arc<Mesh>) {
        if let Some(index) = self.index_of(id) {
            self.project.manifest.objects[index].hollow.built = Some(built);
            self.project.models[index].shell = Some(shell);
        }
    }

    /// Gives object `id` the blockers its wall is kept solid by, in its own space.
    pub(crate) fn set_blockers(&mut self, id: ObjectId, blockers: Vec<Blocker>) {
        if let Some(index) = self.index_of(id) {
            self.project.manifest.objects[index].hollow.blockers = blockers;
        }
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

    /// The support tool's groups and what the plate's supports stand on, as the file keeps them.
    pub(crate) fn support_state(&self) -> &SupportState {
        &self.project.manifest.supports
    }

    /// Gives object `id` the support `points` and `grown` trees (its own space), and the plate
    /// the `groups` they are built to, `active` in hand. Nothing changes off the plate.
    pub(crate) fn set_supports(
        &mut self,
        id: ObjectId,
        (groups, active): (Vec<Group>, u16),
        (points, grown): (Vec<SupportPoint>, Vec<SupportTree>),
    ) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let plate = &mut self.project.manifest.supports;
        plate.groups = groups;
        plate.active = active;
        let state = &mut self.project.manifest.objects[index].supports;
        state.points = points;
        state.grown = grown;
    }

    /// Gives object `id` its support `points`, `frozen` and `grown` trees (its own space).
    pub(crate) fn set_every_support(
        &mut self,
        id: ObjectId,
        (points, frozen, grown): (Vec<SupportPoint>, Vec<SupportTree>, Vec<SupportTree>),
    ) {
        if let Some(index) = self.index_of(id) {
            let state = &mut self.project.manifest.objects[index].supports;
            state.points = points;
            state.frozen = frozen;
            state.grown = grown;
        }
    }

    /// Gives object `id` the support `points` and `frozen` trees left after a hole drilled
    /// some out; its grown trees go, to grow again without them (ADR 0443).
    pub(crate) fn set_drilled_supports(
        &mut self,
        id: ObjectId,
        (points, frozen): (Vec<SupportPoint>, Vec<SupportTree>),
    ) {
        if let Some(index) = self.index_of(id) {
            let state = &mut self.project.manifest.objects[index].supports;
            state.points = points;
            state.frozen = frozen;
            state.grown.clear();
        }
    }

    /// Lays `plugs` (plate coordinates) as new objects on `id`'s plate in place of those laid
    /// before; answers their ids and those taken off, or `None` if `id` is not on the plate.
    pub(crate) fn lay_plugs(
        &mut self,
        id: ObjectId,
        plugs: Vec<Piece>,
    ) -> Option<(Vec<ObjectId>, Added)> {
        self.index_of(id)?;
        let removed = self.take_plugs_of(id);
        let index = self.index_of(id)?;
        let old = self.project.manifest.objects[index].clone();
        let mut added = Vec::with_capacity(plugs.len());
        for piece in plugs {
            let plug = ObjectId(self.next_id);
            self.next_id += 1;
            added.push((plug, piece.role));
            let mut state = new_object(plug, &old, &piece);
            state.plug_of = u32::try_from(index).ok();
            let meshes = ModelMeshes {
                source: Arc::new(piece.mesh),
                shell: None,
            };
            let end = self.project.manifest.objects.len();
            self.put_at(end, state, meshes);
        }
        Some((removed, added))
    }

    /// Takes the plugs laid for object `id` off the plate, answering their ids.
    pub(crate) fn take_plugs_of(&mut self, id: ObjectId) -> Vec<ObjectId> {
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

    /// Records which way object `id`'s faces now turn, as a repair that turned them found.
    pub(crate) fn set_orientation(&mut self, id: ObjectId, orientation: Orientation) {
        if let Some(index) = self.index_of(id) {
            self.project.manifest.objects[index].summary.orientation = orientation;
        }
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
        Role::Plug(index) => format!("{name} plug {index}"),
    }
}

#[cfg(test)]
mod tests;
