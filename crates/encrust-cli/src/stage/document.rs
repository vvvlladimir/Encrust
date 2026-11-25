//! Models staged as the window holds them, one `Document` object per piece, edited by the
//! flags folded to edits (ADR 0472, `docs/design/cli-on-document.md`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use core_document::Report;
use core_document::{Applied, ApplyError, Document, apply, standing_meshes};
use core_edit::{Edit, ObjectId};
use core_engine::plate_models;
use core_engine::project::{AutoSizing, Group, Manifest, SupportState};
use core_engine::{Model, SupportPlan};
use core_geometry::{
    Aabb, BoundingVolume, Mesh, Scalar, Transform, list_shells, signed_volume, transform_mesh,
};
use core_volume::Cancel;
use printer_profiles::{PrinterProfile, SupportProfile};

use super::finished::{self, Finished};
use super::{
    Found, Laid, Part, Shaping, bounds, imported, modelled, placed, repaired, standing_of,
};
use crate::args::{JobArgs, ProfileArgs};
use crate::exit::{Cancelled, Stop};
use crate::pipeline::{Watch, read_again};
use crate::plate_edits::{EditArgs, each_edit, imported_model, write_project};
use crate::profiles::Chosen;

/// The printer and resin a project names by catalogue id.
type Named = (Option<String>, Option<String>);

/// The plate a run edits, and the entry of the command line each object came from.
pub(crate) struct Staging {
    document: Document,
    owners: Vec<(ObjectId, usize)>,
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
        Self { document, owners }
    }

    /// The objects of entry `entry`, in plate order.
    pub(crate) fn ids_of(&self, entry: usize) -> Vec<ObjectId> {
        (self.owners.iter())
            .filter(|(_, owner)| *owner == entry)
            .map(|(id, _)| *id)
            .collect()
    }

    /// The one object entry `entry` stands as before modelling makes more of it.
    pub(crate) fn object_of(&self, entry: usize) -> Result<ObjectId> {
        (self.owners.iter())
            .find(|(_, owner)| *owner == entry)
            .map(|(id, _)| *id)
            .with_context(|| format!("model {} has no object on the plate", entry + 1))
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

    /// Each entry's objects as meshes standing on the plate, for the steps still run on meshes.
    pub(super) fn pieces(&self, entries: usize) -> Vec<Vec<Mesh>> {
        let mut pieces = vec![Vec::new(); entries];
        for (id, entry) in &self.owners {
            pieces[*entry].extend(self.mesh(*id));
        }
        pieces
    }

    /// The plate as it stands, with a part for each entry and what was found on it.
    pub(crate) fn laid(
        self,
        (found, finished): (Vec<Found>, Vec<Finished>),
        (entries, out): (&[(PathBuf, Shaping)], Option<&Path>),
        printer: Option<&PrinterProfile>,
        talk: bool,
    ) -> Result<Laid> {
        let owners: Vec<usize> = self.owners.iter().map(|(_, entry)| *entry).collect();
        let project = self.document.into_project();
        if let Some(path) = out {
            write_project(path, &project)?;
        }
        let models = plate_models(&project, project.manifest.active_plate);
        let mut stands_in: Vec<Option<Aabb>> = vec![None; entries.len()];
        for (model, &entry) in models.iter().zip(&owners) {
            let merged = [stands_in[entry], bounds(model)].into_iter().flatten();
            stands_in[entry] = merged.reduce(|one, other| one.merged(&other));
        }
        let finished = finished
            .into_iter()
            .chain(std::iter::repeat_with(Finished::default));
        let parts = (found.into_iter().zip(entries).zip(stands_in).zip(finished))
            .map(
                |((((import, oriented), (path, _)), stands_in), done)| Part {
                    input: path.clone(),
                    import: Some(import),
                    oriented,
                    standing: standing_of(stands_in, printer, done.supports.is_some(), talk),
                    hollow: done.hollow,
                    drains: done.drains,
                    supports: done.supports,
                    meets: Vec::new(),
                    prepared: None,
                    recipe: None,
                },
            )
            .collect();
        Ok(Laid {
            parts,
            models,
            owners,
        })
    }
}

/// Each entry imported, then mended, repaired, placed, pressed and modelled by the edits its
/// flags fold to; `by_recipe` leaves the orientation search and the repair to the recipe.
pub(super) fn shaped(
    entries: &[(PathBuf, Shaping)],
    (job, chosen): (&JobArgs, &Chosen),
    watch: &Watch,
    by_recipe: bool,
) -> Result<(Staging, Vec<Found>)> {
    // A recipe's orient and repair steps run what --orient and --repair ask; models combined
    // are joined where their files put them and the one they make is centred.
    let combining = job.model.combine.is_some();
    let edited: Vec<(PathBuf, Shaping)> = (entries.iter())
        .map(|(path, shaping)| {
            let mut shaping = shaping.clone();
            let transform = &mut shaping.import.transform;
            if by_recipe {
                transform.orient = false;
                shaping.import.repair = None;
            }
            if combining {
                transform.center = false;
            }
            (path.clone(), shaping)
        })
        .collect();
    let centred = entries
        .iter()
        .any(|(_, shaping)| shaping.import.transform.center);
    let printer = chosen.printer.as_ref();
    let (mut staging, found) = placed_by_edits(&edited, (&job.profile, chosen), false, watch)?;
    if watch.talk {
        for (import, _) in &found {
            print!("{import}");
        }
    }
    let centred = (combining && centred).then_some(printer);
    modelled::modelled(&mut staging, &job.model, centred, watch)?;
    Ok((staging, found))
}

/// Each entry imported, then mended, repaired, placed and pressed by its flags' edits, as `slice`
/// and `inspect` take it; `with_shells` lists its shells as import leaves it.
pub(crate) fn placed_by_edits(
    entries: &[(PathBuf, Shaping)],
    (profile, chosen): (&ProfileArgs, &Chosen),
    with_shells: bool,
    watch: &Watch,
) -> Result<(Staging, Vec<Found>)> {
    let (meshes, mut found) = imported(entries, chosen, watch)?;
    for (mesh, (import, _)) in meshes.iter().zip(&mut found) {
        import.shells = with_shells.then(|| list_shells(mesh));
    }
    let pieces = meshes.into_iter().map(|mesh| vec![mesh]).collect();
    let mut staging = Staging::of(pieces, &found, (entries, catalogue_ids(profile)));
    let printer = chosen.printer.as_ref();
    repaired::repaired(&mut staging, &mut found, entries, watch)?;
    placed::placed(&mut staging, &mut found, entries, (profile, printer), watch)?;
    repaired::pressed(&mut staging, entries, watch)?;
    read_again_where_reshaped(&staging, &mut found, entries, printer)?;
    Ok((staging, found))
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

/// The printer and resin the run names by catalogue id, which a project written keeps; none
/// for one of a file.
fn catalogue_ids(selection: &ProfileArgs) -> Named {
    let printer = selection
        .printer
        .clone()
        .filter(|_| selection.profile.is_none());
    let resin = selection
        .resin
        .clone()
        .filter(|_| selection.material.is_none());
    (printer, resin)
}

/// What import read off each entry's object, read again where an edit before modelling
/// changed it.
fn read_again_where_reshaped(
    staging: &Staging,
    found: &mut [Found],
    entries: &[(PathBuf, Shaping)],
    printer: Option<&PrinterProfile>,
) -> Result<()> {
    for (entry, ((import, _), (_, shaping))) in found.iter_mut().zip(entries).enumerate() {
        let mesh = staging.mesh(staging.object_of(entry)?);
        if let Some(mesh) = mesh.filter(|_| repaired::reshaped(&shaping.import)) {
            read_again(import, &mesh, printer)?;
        }
    }
    Ok(())
}

/// An edit's error as the command line says it: a stop is the run's own.
pub(crate) fn said(error: ApplyError) -> anyhow::Error {
    match error {
        ApplyError::Cancelled => Cancelled.into(),
        other => other.into(),
    }
}

/// Every entry's object hollowed, drilled, plugged and supported by the edits its flags fold
/// to, then the plate arranged where asked, last, with the supports; `held`: printer, drift mm.
pub(super) fn laid(
    mut staging: Staging,
    found: Vec<Found>,
    (entries, edits): (&[(PathBuf, Shaping)], &EditArgs),
    (held, arrange): ((Option<&PrinterProfile>, Scalar), bool),
    watch: &Watch,
) -> Result<Laid> {
    for (_, shaping) in entries {
        shaping.hollow.check_flags()?;
    }
    let finished = finished::finished(&mut staging, entries, held, watch)?;
    placed::copied(&mut staging, entries, watch)?;
    if arrange {
        arranged(&mut staging, entries, held.0, watch)?;
    }
    each_edit(&edits.listed()?, |edit| {
        let first = staging.ids().first().copied().unwrap_or(ObjectId::UNSET);
        let object = edit.object().unwrap_or(first);
        staging.apply(edit, object, watch.stop).map_err(said)?;
        Ok(())
    })?;
    let out = edits.project_out.as_deref();
    staging.laid((found, finished), (entries, out), held.0, watch.talk)
}

/// `--arrange`: every object spread over the usable plate with its supports, as the window's
/// Arrange packs it; one that finds no room is said and left where it stood.
pub(super) fn arranged(
    staging: &mut Staging,
    entries: &[(PathBuf, Shaping)],
    printer: Option<&PrinterProfile>,
    watch: &Watch,
) -> Result<()> {
    let printer = printer.context("--arrange needs a printer to know the plate")?;
    let (low_mm, plate_mm) = finished::usable_area(printer);
    let edit = Edit::Arrange {
        low_mm,
        plate_mm,
        clearance_mm: core_edit::defaults::arrange_clearance_mm(),
    };
    let first = staging.ids().first().copied().unwrap_or(ObjectId::UNSET);
    let applied = staging
        .apply(&edit, first, watch.stop)
        .map_err(said)
        .context("arranging the plate")?;
    if let Report::Arranged { left_out, .. } = applied.report
        && let Some(&one) = left_out.first()
    {
        let named = staging
            .entry_of(one)
            .map(|entry| entries[entry].0.display().to_string());
        tracing::warn!(
            "{} of the plate's models found no room and stand where they were, {} among them",
            left_out.len(),
            named.unwrap_or_default()
        );
    }
    Ok(())
}
