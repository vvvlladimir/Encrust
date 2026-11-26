//! Models staged as the window holds them, one `Document` object per piece, edited by the
//! flags folded to edits (ADR 0472, `docs/design/cli-on-document.md`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use core_document::{Applied, ApplyError, Document, apply, standing_meshes};
use core_edit::{Edit, ObjectId};
use core_engine::plate_models;
use core_engine::project::{AutoSizing, Group, Manifest, ObjectState, Project, SupportState};
use core_engine::{Model, SupportPlan};
use core_geometry::{Aabb, BoundingVolume, Mesh, Scalar, Transform, signed_volume, transform_mesh};
use core_volume::Cancel;
use printer_profiles::{PrinterProfile, SupportProfile};

use super::finished::Finished;
use super::{Found, Laid, Part, Shaping, bounds, standing_of};
use crate::exit::{Cancelled, Stop};
use crate::pipeline::OrientSummary;
use crate::plate_edits::{imported_model, write_project};

/// The printer and resin a project names by catalogue id.
pub(super) type Named = (Option<String>, Option<String>);

/// The plate a run edits, and the entry of the command line each object came from.
pub(crate) struct Staging {
    document: Document,
    owners: Vec<(ObjectId, usize)>,
    /// The objects `--only` names, empty for every one.
    only: Vec<ObjectId>,
    /// The plate the flags work on, for a project's; a run of models has one.
    plate: u32,
    /// What the search chose for each piece it stood, where an entry was cut into several.
    oriented: HashMap<ObjectId, OrientSummary>,
}

impl Staging {
    /// An object for each piece of each entry, ids from 1 in the order given, as the window
    /// gives them.
    pub(super) fn of(
        pieces: Vec<Vec<Mesh>>,
        found: &[Found],
        (entries, named): (&[(PathBuf, Shaping)], Named),
    ) -> Self {
        // The first entry's profile ends in hand, every other's a group of the plate.
        let held: Vec<&SupportPlan> = (entries.iter().rev())
            .filter_map(|(_, shaping)| shaping.supports.as_ref())
            .collect();
        let mut objects = Vec::new();
        let mut owners = Vec::new();
        for (entry, (meshes, (import, _))) in pieces.into_iter().zip(found).enumerate() {
            for mesh in meshes {
                objects.push(imported_model(&entries[entry].0, mesh, import));
                owners.push((ObjectId(owners.len() as u64 + 1), entry));
            }
        }
        let mut manifest = Manifest::new(named.0, named.1);
        for plan in held {
            hold(&mut manifest.supports, plan);
        }
        let document = Document::of_models(manifest, objects);
        let only = (entries.first()).map_or_else(Vec::new, |(_, shaping)| shaping.only.clone());
        Self {
            document,
            owners,
            only,
            plate: 0,
            oriented: HashMap::new(),
        }
    }

    /// A project's plate to be edited by the flags, every object of it the one entry's; the
    /// flags work on `plate` and on the objects `only` names, every one when it is empty.
    pub(crate) fn of_project(project: Project, plate: u32, only: Vec<ObjectId>) -> Self {
        let document = Document::from_project(project);
        let owners = document.ids().into_iter().map(|id| (id, 0)).collect();
        Self {
            document,
            owners,
            only,
            plate,
            oriented: HashMap::new(),
        }
    }

    /// Notes what the search chose for `object`, one piece of several an entry was cut into.
    pub(super) fn note_oriented(&mut self, object: ObjectId, summary: OrientSummary) {
        self.oriented.insert(object, summary);
    }

    /// The plate as a project, as the edits left it.
    pub(crate) fn into_project(self) -> Project {
        self.document.into_project()
    }

    /// The objects of entry `entry` the flags shape: on the plate worked on, named by `--only`
    /// where it names any, never a plug.
    pub(crate) fn targets(&self, entry: usize) -> Vec<ObjectId> {
        (self.ids_of(entry).into_iter())
            .filter(|id| self.only.is_empty() || self.only.contains(id))
            .filter(|&id| {
                self.document
                    .object(id)
                    .is_some_and(|(state, _)| state.plate == self.plate && state.plug_of.is_none())
            })
            .collect()
    }

    /// The objects every entry's flags shape, in plate order.
    pub(crate) fn every_target(&self) -> Vec<ObjectId> {
        let entries = self
            .owners
            .iter()
            .map(|(_, entry)| *entry)
            .max()
            .map_or(0, |last| last + 1);
        (0..entries).flat_map(|entry| self.targets(entry)).collect()
    }

    /// The ids `--only` names that are no object of the plate worked on.
    pub(crate) fn unknown_only(&self) -> Vec<ObjectId> {
        let on_plate = self.every_target();
        (self.only.iter())
            .filter(|id| !on_plate.contains(id))
            .copied()
            .collect()
    }

    /// The objects of entry `entry`, in plate order.
    pub(crate) fn ids_of(&self, entry: usize) -> Vec<ObjectId> {
        (self.owners.iter())
            .filter(|(_, owner)| *owner == entry)
            .map(|(id, _)| *id)
            .collect()
    }

    /// How high object `id`'s raft lifts it, mm; nought off a raft or off the plate.
    pub(crate) fn raft_lift_mm(&self, id: ObjectId) -> f32 {
        (self.document.object(id)).map_or(0.0, |(state, _)| state.supports.raft_lift_mm)
    }

    /// Where object `id` stands; the identity off the plate.
    pub(crate) fn transform(&self, id: ObjectId) -> Transform {
        self.document
            .object(id)
            .map(|(state, _)| state.transform)
            .unwrap_or_default()
    }

    /// Takes `profile` up as the plate's group in hand, as a front end puts a profile of its
    /// own on the plate before an edit names it.
    pub(crate) fn hold_profile(&mut self, profile: SupportProfile) {
        let group = Group {
            name: profile.name.clone(),
            profile,
        };
        self.document.hold_group(group);
    }

    /// Whether object `id` carries a cavity.
    pub(crate) fn hollowed(&self, id: ObjectId) -> bool {
        self.document
            .object(id)
            .is_some_and(|(_, meshes)| meshes.shell.is_some())
    }

    /// The models of the plate as a run cuts them, in the order of [`Self::ids`].
    pub(crate) fn models(&self) -> Vec<Model> {
        let project = self.document.clone().into_project();
        plate_models(&project, project.manifest.active_plate)
    }

    /// Object `id` as a run cuts it, its supports under it.
    pub(crate) fn model(&self, id: ObjectId) -> Option<Model> {
        self.document.model(id)
    }

    /// The box object `id` and its supports stand in, plate mm.
    pub(crate) fn standing_bounds(&self, id: ObjectId) -> Option<Aabb> {
        let supports = standing_meshes(&self.document, id);
        let boxes = supports.iter().filter_map(|mesh| mesh.aabb());
        let placed = self.mesh(id).and_then(|mesh| mesh.aabb());
        placed
            .into_iter()
            .chain(boxes)
            .reduce(|one, other| one.merged(&other))
    }

    /// What object `id` and its supports cover of the plate, as one mesh.
    pub(crate) fn standing_mesh(&self, id: ObjectId) -> Mesh {
        let mut mesh = self.mesh(id).unwrap_or_default();
        for supports in standing_meshes(&self.document, id) {
            mesh.append(&supports);
        }
        mesh
    }

    /// The resin in the supports under object `id`, millilitres.
    pub(crate) fn support_ml(&self, id: ObjectId) -> Scalar {
        let meshes = standing_meshes(&self.document, id);
        let mm3: Scalar = meshes.iter().map(|mesh| signed_volume(mesh).abs()).sum();
        mm3 / 1000.0
    }

    /// The triangles of the supports and raft under object `id`.
    pub(crate) fn support_faces(&self, id: ObjectId) -> usize {
        let meshes = standing_meshes(&self.document, id);
        meshes.iter().map(|mesh| mesh.faces.len()).sum()
    }

    /// The entry object `id` came from.
    pub(super) fn entry_of(&self, id: ObjectId) -> Option<usize> {
        (self.owners.iter())
            .find(|(object, _)| *object == id)
            .map(|(_, entry)| *entry)
    }

    /// Every object, in plate order.
    pub(crate) fn ids(&self) -> Vec<ObjectId> {
        self.owners.iter().map(|(id, _)| *id).collect()
    }

    /// Carries `edit` out on the plate, stopped by `stop`; what it adds takes the entry of what
    /// it removed, else of `object`, and stands where the document put it.
    pub(crate) fn apply(
        &mut self,
        edit: &Edit,
        object: ObjectId,
        stop: &Stop,
    ) -> Result<Applied, ApplyError> {
        let asked = || stop.requested();
        self.apply_with(edit, object, Cancel::when(&asked))
    }

    /// `apply`, stopped and told how far it has got by `cancel`.
    pub(crate) fn apply_with(
        &mut self,
        edit: &Edit,
        object: ObjectId,
        cancel: Cancel<'_>,
    ) -> Result<Applied, ApplyError> {
        let applied = apply(&mut self.document, edit, cancel)?;
        let first = applied.removed.first().copied().unwrap_or(object);
        let owner = self.entry_of(first).unwrap_or(0);
        self.owners = (self.document.ids().into_iter())
            .map(|id| (id, self.entry_of(id).unwrap_or(owner)))
            .collect();
        Ok(applied)
    }

    /// Object `id`'s mesh as it prints, standing on the plate.
    pub(crate) fn mesh(&self, id: ObjectId) -> Option<Mesh> {
        let (object, meshes) = self.document.object(id)?;
        Some(transform_mesh(meshes.printed(), object.transform))
    }

    /// The plate as it stands, with a part for each object but a plug, which counts with its
    /// model, and what was found on it; import's findings go with an entry's first part.
    pub(crate) fn laid(
        mut self,
        (found, finished): (Vec<Found>, Vec<(ObjectId, Finished)>),
        (entries, out): (&[(PathBuf, Shaping)], Option<&Path>),
        printer: Option<&PrinterProfile>,
        talk: bool,
    ) -> Result<Laid> {
        let entry_of: HashMap<ObjectId, usize> = self.owners.iter().copied().collect();
        let project = self.document.into_project();
        if let Some(path) = out {
            write_project(path, &project)?;
        }
        let plate = project.manifest.active_plate;
        let models = plate_models(&project, plate);
        let on: Vec<&ObjectState> = (project.manifest.objects.iter())
            .filter(|object| object.printable_on(plate))
            .collect();
        let holders = holders_of(&project.manifest.objects, &on);
        let mut found: Vec<Option<Found>> = found.into_iter().map(Some).collect();
        let mut finished: HashMap<ObjectId, Finished> = finished.into_iter().collect();
        let (mut parts, mut part_entries, mut owners) = (Vec::new(), Vec::new(), Vec::new());
        let mut part_at = HashMap::new();
        for (index, object) in on.iter().enumerate() {
            if holders[index] != index {
                continue;
            }
            let entry = entry_of.get(&object.id).copied().unwrap_or(0);
            let pieces = on.iter().enumerate().filter(|(at, other)| {
                holders[*at] == *at && entry_of.get(&other.id) == Some(&entry)
            });
            let path = &entries[entry.min(entries.len() - 1)].0;
            let input = match pieces.count() {
                1 => path.clone(),
                _ => PathBuf::from(format!("{} #{}", path.display(), object.id.0)),
            };
            let (import, oriented) = found[entry]
                .take()
                .map_or((None, None), |(i, o)| (Some(i), o));
            let oriented = self.oriented.remove(&object.id).or(oriented);
            let done = finished.remove(&object.id).unwrap_or_default();
            part_at.insert(index, parts.len());
            parts.push(Part {
                id: Some(object.id),
                input,
                import,
                oriented,
                standing: None,
                hollow: done.hollow,
                drains: done.drains,
                supports: done.supports,
                meets: Vec::new(),
                prepared: None,
                recipe: None,
            });
            part_entries.push(entry);
        }
        let mut stands_in: Vec<Option<Aabb>> = vec![None; parts.len()];
        for (index, model) in models.iter().enumerate() {
            let part = part_at[&holders[index]];
            owners.push(part);
            let merged = [stands_in[part], bounds(model)].into_iter().flatten();
            stands_in[part] = merged.reduce(|one, other| one.merged(&other));
        }
        for (part, stands_in) in parts.iter_mut().zip(stands_in) {
            part.standing = standing_of(stands_in, printer, part.supports.is_some(), talk);
        }
        Ok(Laid {
            parts,
            models,
            owners,
            part_entries,
        })
    }
}

/// For each of `on`, the place in `on` of the object whose part it counts with: its own, a
/// plug's the model it was laid for.
fn holders_of(all: &[ObjectState], on: &[&ObjectState]) -> Vec<usize> {
    let place_of = |object: &ObjectState| on.iter().position(|other| other.id == object.id);
    (on.iter().enumerate())
        .map(|(index, object)| {
            let holder = object.plug_of.and_then(|of| all.get(of as usize));
            holder.and_then(place_of).unwrap_or(index)
        })
        .collect()
}

/// An edit's error as the command line says it: a stop is the run's own.
pub(crate) fn said(error: ApplyError) -> anyhow::Error {
    match error {
        ApplyError::Cancelled => Cancelled.into(),
        other => other.into(),
    }
}

/// The plate's supports with `plan` in hand, as the panel holds the profile chosen: the
/// automatic profile sizing, or its profile as the active group of its name, on its raft.
fn hold(supports: &mut SupportState, plan: &SupportPlan) {
    let placing = plan.placing();
    supports.raft = Some(placing.raft);
    if let Some(auto) = plan.sizing() {
        supports.sizing = Some(AutoSizing {
            on: true,
            profile: auto.clone(),
        });
        return;
    }
    let named = (supports.groups.iter()).position(|group| group.profile.name == placing.name);
    let group = Group {
        name: placing.name.clone(),
        profile: placing,
    };
    let at = match named {
        Some(at) => {
            supports.groups[at] = group;
            at
        }
        None => {
            supports.groups.push(group);
            supports.groups.len() - 1
        }
    };
    supports.active = at as u16;
}
