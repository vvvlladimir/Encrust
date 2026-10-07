use std::sync::Arc;

use core_format::{ExposurePlan, ExposureRange};
use core_geometry::Bvh;
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_raster::Shading;
use core_slicer::WINDOW_LAYERS;
use printer_profiles::{MaterialProfile, SupportProfile};

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

/// One plate of `project` as a run takes it: the geometry the file holds, meshed and cut
/// but never decided again (ADR 0191).
pub fn open_plate(project: &Project, opening: &Opening) -> Result<Plate, EngineError> {
    let manifest = &project.manifest;
    let printer = manifest.printer.as_ref().ok_or(EngineError::NoPrinter)?;
    let resin = manifest.resin.as_ref().ok_or(EngineError::NoResin)?;
    let (material, bands) = tuned(&resin.profile, printer.id.as_deref(), &manifest.slicing);

    let table = support_table(manifest);
    let models = manifest
        .objects
        .iter()
        .zip(&project.models)
        .filter(|(object, _)| object.plate == opening.plate && object.visible)
        .map(|(object, meshes)| model(object, meshes, &table))
        .collect();

    let slicing = &manifest.slicing;
    Ok(Plate {
        models,
        printer: printer.profile.clone(),
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
        format: SlicedFormat::from(slicing.format).at_revision_of(printer.profile.output),
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

/// One object as it prints: the shell the file holds, the bodies its holes cut, and the
/// trees it keeps, meshed to the profile of their group.
fn model(object: &ObjectState, meshes: &ModelMeshes, table: &[SupportProfile]) -> Model {
    let bvh = Arc::new(Bvh::build(&meshes.source));
    let hollow = hollow_of(&object.hollow, meshes);
    let supports = supports_of(
        &object.supports,
        &meshes.source,
        &bvh,
        object.transform,
        table,
    );

    Model {
        mesh: Arc::clone(meshes.printed()),
        transform: object.transform,
        cuts: hollow.cut_bodies().cloned(),
        supports: supports.meshes().unwrap_or_default().to_vec(),
    }
}
