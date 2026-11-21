//! Models staged as the window holds them, one `Document` object per piece (ADR 0472).

use std::path::PathBuf;

use anyhow::Result;
use core_document::Document;
use core_engine::plate_models;
use core_engine::project::Manifest;
use core_geometry::{Aabb, BoundingVolume, Mesh, Scalar};
use printer_profiles::PrinterProfile;

use super::{Found, Laid, Part, Shaping, bounds, put_down_every, shapes_anything, standing_of};
use crate::pipeline::Watch;
use crate::plate_edits::imported_model;

/// Every piece of every entry laid on the plate: through a `Document` where no entry is shaped
/// past import, else hollowed and supported one by one. `held` is the printer and the drift, mm.
pub(super) fn laid<'a>(
    pieces: Vec<Vec<Mesh>>,
    found: Vec<Found>,
    entries: &'a [(PathBuf, Shaping)],
    held: (Option<&PrinterProfile>, Scalar),
    watch: &Watch,
) -> Result<Laid<'a>> {
    for (_, shaping) in entries {
        shaping.hollow.check_flags()?;
    }
    // TODO(step-R6d4): hollow, drains and supports as edits; put_down_every goes in R6d8.
    let shaped =
        |shaping: &Shaping| shapes_anything(shaping) || !shaping.hollow.drain_support_at.is_empty();
    if entries.iter().any(|(_, shaping)| shaped(shaping)) {
        return put_down_every(pieces, found, entries, held, watch);
    }
    Ok(documented(pieces, found, entries, held.0, watch.talk))
}

fn documented<'a>(
    pieces: Vec<Vec<Mesh>>,
    found: Vec<Found>,
    entries: &'a [(PathBuf, Shaping)],
    printer: Option<&PrinterProfile>,
    talk: bool,
) -> Laid<'a> {
    let mut owners = Vec::new();
    let mut objects = Vec::new();
    for (entry, (meshes, (import, _))) in pieces.into_iter().zip(&found).enumerate() {
        for mesh in meshes {
            objects.push(imported_model(&entries[entry].0, mesh, import));
            owners.push(entry);
        }
    }
    let project = Document::of_models(Manifest::new(None, None), objects).into_project();
    let models = plate_models(&project, project.manifest.active_plate);
    let mut stands_in: Vec<Option<Aabb>> = vec![None; entries.len()];
    let mut plugged = Vec::with_capacity(models.len());
    for (index, (model, &entry)) in models.iter().zip(&owners).enumerate() {
        let merged = [stands_in[entry], bounds(model)].into_iter().flatten();
        stands_in[entry] = merged.reduce(|one, other| one.merged(&other));
        plugged.push((index + 1..index + 1, entries[entry].0.as_path()));
    }
    let parts = (found.into_iter().zip(entries).zip(stands_in))
        .map(|(((import, oriented), (path, _)), stands_in)| Part {
            input: path.clone(),
            import: Some(import),
            oriented,
            hollow: None,
            drains: None,
            supports: None,
            standing: standing_of(stands_in, printer, false, talk),
            meets: Vec::new(),
            prepared: None,
            recipe: None,
        })
        .collect();
    Laid {
        parts,
        models,
        owners,
        plugged,
    }
}
