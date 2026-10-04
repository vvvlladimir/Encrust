use std::sync::Arc;

use core_format::{ExposurePlan, ExposureRange};
use core_geometry::{Bvh, Mesh};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_raster::Shading;
use core_slicer::WINDOW_LAYERS;
use core_supports::ModelSupports;
use core_volume::{ModelHollow, Shell, hollow_at_scale};
use printer_profiles::{MaterialProfile, SupportProfile};

use crate::project::{Manifest, ObjectState, Project, SlicingState};
use crate::{Cutting, EngineError, Model, Plate};

/// What opening a project into a plate needs that the file does not say.
#[derive(Debug, Clone, Copy)]
pub struct Opening {
    /// Which of the project's plates is opened.
    pub plate: u32,
    /// What building one cavity may take, in bytes; a finer lattice is coarsened to fit.
    pub hollow_budget_bytes: usize,
    /// Layers rasterised at once.
    pub raster_window: usize,
    /// When the file is made, seconds since the Unix epoch.
    pub created_unix_s: u64,
}

/// One plate of `project` as a run takes it, with every cavity and support tree the file
/// leaves out built again from what it keeps (ADR 0178).
pub fn open_plate(project: &Project, opening: &Opening) -> Result<Plate, EngineError> {
    let manifest = &project.manifest;
    let printer = manifest.printer.as_ref().ok_or(EngineError::NoPrinter)?;
    let resin = manifest.resin.as_ref().ok_or(EngineError::NoResin)?;
    let (material, bands) = tuned(&resin.profile, printer.id.as_deref(), &manifest.slicing);

    let table = support_table(manifest);
    let models = manifest
        .objects
        .iter()
        .zip(&project.meshes)
        .filter(|(object, _)| object.plate == opening.plate && object.visible)
        .map(|(object, mesh)| model(object, mesh, &table, opening.hollow_budget_bytes))
        .collect::<Result<Vec<_>, _>>()?;

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

/// One object as it prints: hollowed again when the file says it was, with its holes cut
/// and its support trees grown from the points it keeps.
fn model(
    object: &ObjectState,
    mesh: &Arc<Mesh>,
    table: &[SupportProfile],
    budget_bytes: usize,
) -> Result<Model, EngineError> {
    let bvh = Arc::new(Bvh::build(mesh));
    let state = &object.hollow;
    let mut cavity = ModelHollow::restored(
        Arc::clone(mesh),
        Arc::clone(&bvh),
        state.blockers.clone(),
        state.drains.clone(),
        state.channels.clone(),
    );
    if let Some(wall) = &state.cavity {
        let settings = cavity.asking(&core_volume::HollowSettings {
            budget_bytes,
            ..wall.settings()
        });
        let hollowed =
            hollow_at_scale(mesh, &bvh, &settings, object.transform.scale).map_err(|source| {
                EngineError::Hollow {
                    object: object.name.clone(),
                    source,
                }
            })?;
        cavity.take(Shell {
            mesh: Arc::new(hollowed.mesh),
            cavity_mm3: hollowed.cavity_mm3,
            voxel_mm: hollowed.voxel_mm,
            coarsened: hollowed.coarsened,
            scale: object.transform.scale,
            settings,
        });
    }

    let placed = &object.supports;
    let mut supports = ModelSupports::restore(
        placed.points.clone(),
        placed.painted.clone(),
        placed.blocked.clone(),
        placed.frozen.clone(),
    );
    supports.refresh(mesh, &bvh, object.transform, table);

    Ok(Model {
        mesh: Arc::clone(cavity.shell().unwrap_or(mesh)),
        transform: object.transform,
        cuts: cavity.cut_bodies().cloned(),
        supports: supports.meshes().unwrap_or_default().to_vec(),
    })
}
