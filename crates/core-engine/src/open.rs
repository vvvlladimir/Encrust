use std::path::Path;
use std::sync::Arc;

use core_format::{ExposurePlan, ExposureRange};
use core_geometry::{Bvh, Mesh};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_raster::Shading;
use core_slicer::WINDOW_LAYERS;
use core_volume::{Drilled, ModelHollow};
use printer_profiles::{Catalogue, MaterialProfile, PrinterProfile, Raft, SupportProfile};

use crate::project::{
    Manifest, ModelMeshes, ObjectState, Project, SlicingState, hollow_of, supports_of,
};
use crate::{Cutting, EngineError, Model, Plate};

/// What opening a project into a plate needs that the file does not say.
#[derive(Debug, Clone, Copy)]
pub struct Opening {
    /// Which of the project's plates is opened.
    pub plate: u32,
    /// Layers rasterised at once.
    pub raster_window: usize,
    /// When the file is made, seconds since the Unix epoch.
    pub created_unix_s: u64,
}

/// The printer and resin a project is opened for, which the file names but does not
/// carry (ADR 0261).
#[derive(Debug, Clone)]
pub struct Machine {
    pub printer: PrinterProfile,
    /// The catalogue id the printer is known by, which the resin's tuning is keyed on.
    pub printer_id: Option<String>,
    pub resin: MaterialProfile,
}

impl Machine {
    /// The printer and resin `manifest` names, as `catalogue` has them.
    pub fn named(manifest: &Manifest, catalogue: &Catalogue) -> Result<Self, EngineError> {
        let printer_id = manifest.printer.as_deref().ok_or(EngineError::NoPrinter)?;
        let resin_id = manifest.resin.as_deref().ok_or(EngineError::NoResin)?;
        Ok(Self {
            printer: catalogue.printer(printer_id)?.profile.clone(),
            printer_id: Some(printer_id.to_owned()),
            resin: catalogue.resin(resin_id)?.profile.clone(),
        })
    }

    /// The printer and resin handed over as the TOML they are kept as, which is how a
    /// caller with no catalogue of the user's names them.
    pub fn from_toml(printer: &str, resin: &str) -> Result<Self, EngineError> {
        Ok(Self {
            printer: PrinterProfile::from_toml_str(printer, Path::new("printer.toml"))?,
            printer_id: None,
            resin: MaterialProfile::from_toml_str(resin, Path::new("resin.toml"))?,
        })
    }
}

/// One plate of `project` as a run takes it on `machine`: the geometry the file holds,
/// meshed and cut but never decided again (ADR 0191).
pub fn open_plate(
    project: &Project,
    machine: &Machine,
    opening: &Opening,
) -> Result<Plate, EngineError> {
    let manifest = &project.manifest;
    let (material, bands) = tuned(
        &machine.resin,
        machine.printer_id.as_deref(),
        &manifest.slicing,
    );

    let table = support_table(manifest);
    let models = manifest
        .objects
        .iter()
        .zip(&project.models)
        .filter(|(object, _)| object.plate == opening.plate && object.visible)
        .map(|(object, meshes)| model(object, meshes, &table, manifest.supports.raft))
        .collect();

    let slicing = &manifest.slicing;
    Ok(Plate {
        models,
        printer: machine.printer.clone(),
        panel: PanelOverrides {
            shading: if slicing.anti_alias {
                Shading::Coverage
            } else {
                Shading::Binary
            },
            grey_levels: slicing.grey_levels,
            grey_floor: None,
            blur_px: slicing.blur_px,
        },
        cutting: Cutting {
            layer_height_mm: material.layer_height_mm,
            adaptive: slicing.adaptive,
            samples: slicing.samples,
            compensation: material.compensation,
            slice_window: WINDOW_LAYERS,
        },
        material,
        exposure: ExposurePlan::new(bands),
        remove_islands: slicing.remove_islands,
        hanging_passes: slicing.hanging_passes,
        format: SlicedFormat::from(slicing.format).at_revision_of(machine.printer.output),
        raster_window: opening.raster_window,
        created_unix_s: opening.created_unix_s,
    })
}

/// The resin as this printer starts from it, carried to the project's layer height, and
/// the bands the file keeps at the resin's own height carried along with it.
fn tuned(
    resin: &MaterialProfile,
    printer_id: Option<&str>,
    slicing: &SlicingState,
) -> (MaterialProfile, Vec<ExposureRange>) {
    let measured = match printer_id {
        Some(id) => resin.starting_point(id),
        None => resin.clone(),
    };
    let height_mm = slicing.layer_height_mm;
    let bands = slicing
        .exposure
        .iter()
        .map(|band| ExposureRange {
            exposure_s: measured.exposure_at(band.exposure_s, height_mm),
            ..*band
        })
        .collect();
    (measured.rescaled_to(height_mm), bands)
}

/// The profile of every support group, the first of which everything falls back to.
fn support_table(manifest: &Manifest) -> Vec<SupportProfile> {
    let table: Vec<SupportProfile> = manifest
        .supports
        .groups
        .iter()
        .map(|group| group.profile.clone())
        .collect();
    if table.is_empty() {
        vec![SupportProfile::default()]
    } else {
        table
    }
}

/// The object's cuts taken out of what it prints from exactly, as the window takes them
/// out, or `None` without cuts or where that fails and the winding rule takes them out.
fn drilled_of(hollow: &ModelHollow, meshes: &ModelMeshes) -> Option<Arc<Mesh>> {
    let cuts = hollow.cut_bodies()?;
    let drilled = Drilled::of(meshes.printed(), hollow.cavity_faces(), cuts).ok()?;
    Some(drilled.mesh)
}

/// One object as it prints: the shell the file holds, the bodies its holes cut, and the
/// trees it keeps, meshed to the profile of their group.
fn model(
    object: &ObjectState,
    meshes: &ModelMeshes,
    table: &[SupportProfile],
    plate_raft: Option<Raft>,
) -> Model {
    let hollow = hollow_of(&object.hollow, meshes);
    let drilled = drilled_of(&hollow, meshes);
    // Against what prints, as the window stood them, so a cavity's supports are swept
    // against the cavity and a drilled hole's against its bore (ADR 0322, 0332).
    let surface = drilled.as_ref().unwrap_or(meshes.printed());
    let bvh = Bvh::build(surface);
    let supports = supports_of(
        &object.supports,
        &meshes.source,
        surface,
        &bvh,
        object.transform,
        &object.support_table(table, plate_raft),
    );

    Model {
        mesh: Arc::clone(meshes.printed()),
        transform: object.transform,
        cuts: hollow.cut_bodies().cloned(),
        drilled,
        supports: supports.meshes().unwrap_or_default().to_vec(),
    }
}
