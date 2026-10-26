//! The window's state as a project file, and back again.
//!
//! What goes in is the plate as it stands, geometry and all; what comes out is that plate
//! and nothing worked out anew (ADR 0191). The printer and resin go by id only, and come
//! back only where this catalogue has them (ADR 0261).

use std::collections::BTreeMap;
use std::sync::Arc;

use core_geometry::{Bvh, Vec3, center_of_mass};
use rayon::prelude::*;

use crate::i18n::tr;
use crate::scene::{ImportSummary, Imported, ObjectId, OwnSettings, Scene, SceneObject};
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::supports::with_own;
use crate::tool_settings::ToolSettings;
use core_engine::project::{
    BuiltCavity, Cavity, Manifest, ModelMeshes, ObjectHollowState, ObjectState, ObjectSupportState,
    OwnSupportState, Project, Summary, VERSION, hollow_of, supports_of,
};
use printer_profiles::{Raft, SupportProfile};

/// What a project file is written from: the plate, and the numbers each tool is set to.
pub struct Captured<'a> {
    pub scene: &'a Scene,
    pub slicing: &'a Slicing,
    pub tools: &'a Tools,
}

/// The same, to be written into by a load.
pub struct CapturedMut<'a> {
    pub scene: &'a mut Scene,
    pub slicing: &'a mut Slicing,
    pub tools: &'a mut Tools,
}

/// The whole plate as a project, ready to be written.
pub fn capture(plate: Captured<'_>) -> Project {
    let Captured {
        scene,
        slicing,
        tools,
    } = plate;
    let settings = ToolSettings::of(tools, slicing);
    let manifest = Manifest {
        version: VERSION,
        printer: slicing
            .printer_id
            .clone()
            .filter(|_| slicing.printer.is_some()),
        resin: slicing.resin_id.clone(),
        slicing: settings.slicing,
        supports: settings.supports,
        hollow: settings.hollow,
        drain: settings.drain,
        cut: settings.cut,
        array: settings.array,
        plates: scene.plates().to_vec(),
        active_plate: scene.active_plate(),
        objects: scene
            .objects()
            .iter()
            .map(|object| object_state(scene, object))
            .collect(),
    };
    Project {
        manifest,
        models: scene.objects().iter().map(model_meshes).collect(),
        preview: None,
    }
}

/// The printer and resin a project named that this catalogue does not have, which the
/// window kept its own in place of.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct NotHere {
    pub printer: Option<String>,
    pub resin: Option<String>,
}

impl NotHere {
    /// What the status line says of them, or `None` when both were found.
    pub fn note(&self) -> Option<String> {
        match (&self.printer, &self.resin) {
            (None, None) => None,
            (Some(printer), None) => {
                Some(tr!("project-missing-printer", printer = printer.clone()))
            }
            (None, Some(resin)) => Some(tr!("project-missing-resin", resin = resin.clone())),
            (Some(printer), Some(resin)) => Some(tr!(
                "project-missing-both",
                printer = printer.clone(),
                resin = resin.clone()
            )),
        }
    }
}

/// The geometry one object is written with: what it was imported as, and the shell
/// standing on it.
fn model_meshes(object: &SceneObject) -> ModelMeshes {
    ModelMeshes {
        source: Arc::clone(&object.mesh),
        shell: object.hollow.shell().map(Arc::clone),
    }
}

fn object_state(scene: &Scene, object: &SceneObject) -> ObjectState {
    ObjectState {
        name: object.name.clone(),
        plate: object.plate,
        transform: object.transform,
        visible: object.visible,
        summary: Summary {
            vertices_merged: object.summary.vertices_merged,
            faces_removed: object.summary.faces_removed,
            orientation: object.summary.orientation,
            diagnostics: object.summary.diagnostics.clone(),
        },
        supports: ObjectSupportState {
            points: object.supports.points().to_vec(),
            painted: object.supports.painted().clone(),
            blocked: object.supports.blocked().clone(),
            frozen: object.supports.frozen().to_vec(),
            grown: object.supports.grown(object.transform),
            raft_lift_mm: object.supports.raft_lift_mm(),
        },
        hollow: ObjectHollowState {
            blockers: object.hollow.blockers().to_vec(),
            drains: object.hollow.drains().to_vec(),
            channels: object.hollow.channels().to_vec(),
            built: built_cavity(object),
        },
        own_hollow: object.own.hollow.clone(),
        own_drain: object.own.drain.clone(),
        own_raft: object.own.raft,
        plug_of: object
            .plug_of
            .and_then(|model| scene.objects().iter().position(|other| other.id == model))
            .and_then(|index| u32::try_from(index).ok()),
        relief_painted: object.relief_painted.clone(),
        own_supports: object
            .own
            .supports
            .iter()
            .map(|(group, profile)| OwnSupportState {
                group: *group,
                profile: profile.clone(),
            })
            .collect(),
    }
}

/// The shell standing on `object` as the file records it, or `None` for a solid model.
fn built_cavity(object: &SceneObject) -> Option<BuiltCavity> {
    let asked = object.hollow.asked()?;
    let (voxel_mm, coarsened) = object.hollow.lattice()?;
    let (scale, stretched) = BuiltCavity::keys_of(object.hollow.built_shape()?);
    Some(BuiltCavity {
        wall: Cavity::of(asked),
        cavity_faces: object.hollow.cavity_faces()?,
        cavity_mm3: object.hollow.cavity_mm3(),
        voxel_mm,
        coarsened,
        scale,
        stretched,
    })
}

/// Puts a loaded project in place of whatever the window was holding. Its printer and
/// resin are taken where this catalogue has them, and its slicing only with its printer;
/// answers the ones it named that are not here.
pub fn apply(project: Project, plate: CapturedMut<'_>) -> NotHere {
    let CapturedMut {
        scene,
        slicing,
        tools,
    } = plate;
    let Project {
        manifest, models, ..
    } = project;

    let printer_named = manifest.printer.is_some();
    let not_here = take_machine(manifest.printer, manifest.resin, slicing);
    let kept = ToolSettings::of(tools, slicing).slicing;
    ToolSettings {
        slicing: if printer_named && not_here.printer.is_none() {
            manifest.slicing
        } else {
            kept
        },
        supports: manifest.supports,
        hollow: manifest.hollow,
        drain: manifest.drain,
        cut: manifest.cut,
        // The manifest has no entry for the Relief, Text, Combine or Repair tools, so they
        // keep what they are set to.
        relief: tools.relief.state,
        array: manifest.array,
        text: tools.text.state.clone(),
        combine: tools.boolean.op,
        repair: tools.repair,
    }
    .apply(tools, slicing);

    *scene = Scene::default();
    for (index, name) in manifest.plates.iter().enumerate() {
        if index > 0 {
            scene.add_plate();
        }
        scene.rename_plate(index as u32, name.clone());
    }
    let (table, raft) = (tools.supports.table(), tools.supports.raft);
    restore_objects(scene, manifest.objects, models, &table, raft);
    scene.select(None);
    scene.show_plate(manifest.active_plate);
    not_here
}

/// Takes the printer and resin of these ids where the catalogue has them, and answers the
/// ones it does not: the window keeps its own in their place.
fn take_machine(printer: Option<String>, resin: Option<String>, slicing: &mut Slicing) -> NotHere {
    let mut not_here = NotHere::default();
    if let Some(id) = printer {
        match slicing.catalogue.printer(&id) {
            Ok(entry) => {
                let profile = entry.profile.clone();
                slicing.set_printer(profile, Some(id));
            }
            Err(_) => not_here.printer = Some(id),
        }
    }
    if let Some(id) = resin {
        match slicing.catalogue.resin(&id) {
            Ok(entry) => {
                let resin = entry.profile.clone();
                slicing.resin_id = Some(id);
                slicing.set_material(resin);
            }
            Err(_) => not_here.resin = Some(id),
        }
    }
    not_here
}

/// Puts every object the file holds back on `scene`, with the shell, the cuts and the
/// trees it was saved with: nothing here is hollowed or grown again.
fn restore_objects(
    scene: &mut Scene,
    objects: Vec<ObjectState>,
    models: Vec<ModelMeshes>,
    table: &[SupportProfile],
    plate_raft: Raft,
) {
    // The hierarchy is the expensive half of opening a model and is per mesh, so the
    // plate's models are built at once rather than one after another.
    let measured: Vec<(Arc<Bvh>, Vec3)> = models
        .par_iter()
        .map(|model| {
            (
                Arc::new(Bvh::build(&model.source)),
                center_of_mass(&model.source).unwrap_or(Vec3::ZERO),
            )
        })
        .collect();

    let mut plugs = Vec::new();
    for ((state, meshes), (bvh, center_of_mass)) in objects.into_iter().zip(&models).zip(measured) {
        let id = scene.insert(Imported {
            name: state.name,
            bvh: Arc::clone(&bvh),
            mesh: Arc::clone(&meshes.source),
            transform: state.transform,
            center_of_mass,
            summary: ImportSummary {
                vertices_merged: state.summary.vertices_merged,
                faces_removed: state.summary.faces_removed,
                orientation: state.summary.orientation,
                diagnostics: state.summary.diagnostics,
            },
            // A project carries meshes, not the files they came from: a texture that was
            // not pressed in before saving is not in there to press in now.
            mapped: None,
        });
        plugs.push((id, state.plug_of));
        let Some(object) = scene.get_mut(id) else {
            continue;
        };
        object.visible = state.visible;
        object.plate = state.plate;
        // Opened as it was saved: the plugs standing are the ones it has.
        object.plugs_laid = None;
        let own_supports: BTreeMap<u16, SupportProfile> = state
            .own_supports
            .into_iter()
            .map(|own| (own.group, own.profile))
            .collect();
        let own = OwnSettings {
            hollow: state.own_hollow,
            drain: state.own_drain,
            supports: own_supports,
            raft: state.own_raft,
        };
        object.hollow = hollow_of(&state.hollow, meshes);
        // Stood against the shell now, or the first frame would take it for a new surface
        // and grow the saved trees again (ADR 0332).
        object.keep_support_surface();
        let (surface, surface_bvh) = object.support_surface();
        let (surface, surface_bvh) = (Arc::clone(surface), Arc::clone(surface_bvh));
        object.supports = supports_of(
            &state.supports,
            &meshes.source,
            &surface,
            &surface_bvh,
            state.transform,
            &with_own(table.to_vec(), &own, plate_raft),
        );
        object.relief_painted = state.relief_painted;
        object.own = own;
    }
    // A plug names its model by its place in the file, which is known once all are in.
    let ids: Vec<ObjectId> = plugs.iter().map(|(id, _)| *id).collect();
    for (id, of) in plugs {
        let model = of.and_then(|index| ids.get(index as usize).copied());
        if let Some(object) = scene.get_mut(id) {
            object.plug_of = model;
        }
    }
}

#[cfg(test)]
mod tests;
