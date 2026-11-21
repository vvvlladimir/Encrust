//! The engine's drain search over an object as it stands on the plate (ADR 0401).

use core_edit::ObjectId;
use core_engine::{
    ANALYSIS_LAYER_MM, DrainSearch, Drained, EngineError, Scanning, suggest_drains_heard,
};
use core_geometry::{Mesh, Scalar, Transform, transform_mesh};
use core_volume::{Cancel, HoleSize, ModelHollow};

use crate::error::ApplyError;

/// Nearer the plate than this a model's lowest point rests on it, millimetres: no hole may
/// open where the first layers spread.
const ON_PLATE_MM: Scalar = 1e-2;

/// What a drain search is asked for.
#[derive(Debug, Clone, Copy)]
pub struct SearchAsked {
    pub size: HoleSize,
    /// The wall the cavity was cut to and its lattice, millimetres; `None` while solid.
    pub wall: Option<(Scalar, Scalar)>,
    /// Whether each drained pocket also gets a vent.
    pub vents: bool,
    pub layer_height_mm: Scalar,
}

/// The plate's height under `placed` where its lowest point rests on it, else `None`.
pub fn plate_under(placed: &Mesh) -> Option<Scalar> {
    placed
        .aabb()
        .map(|bounds| bounds.mins.z)
        .filter(|lowest| lowest.abs() <= ON_PLATE_MM)
}

/// The holes that drain every pocket `placed` traps, each proven by a drill and a rescan;
/// `plate_z` is the plate under a resting model, `heard` stops it by answering false.
pub fn drains_proven(
    (placed, cuts): (&Mesh, Option<&Mesh>),
    plate_z: Option<Scalar>,
    asked: &SearchAsked,
    heard: impl FnMut(Scanning) -> bool,
) -> Result<Option<Drained>, EngineError> {
    let search = DrainSearch {
        mesh: placed,
        cuts,
        size: asked.size,
        wall: asked.wall,
        vents: asked.vents,
        plate_z,
        layer_height_mm: asked.layer_height_mm,
    };
    suggest_drains_heard(&search, heard)
}

/// The search over object `id` as it stands: `printed`, its shell, and the cuts `hollow` has,
/// on the plate under `transform`.
pub(crate) fn search_object(
    (printed, hollow, transform): (&Mesh, &ModelHollow, Transform),
    (size, vents): (HoleSize, bool),
    (id, cancel): (ObjectId, Cancel<'_>),
) -> Result<Drained, ApplyError> {
    let placed = transform_mesh(printed, transform);
    let cuts = hollow
        .cut_bodies()
        .map(|cuts| transform_mesh(cuts, transform));
    let wall = hollow.asked().map(|asked| asked.thickness_mm);
    let asked = SearchAsked {
        size,
        wall: wall.zip(hollow.lattice().map(|(voxel_mm, _)| voxel_mm)),
        vents,
        layer_height_mm: ANALYSIS_LAYER_MM,
    };
    let found = drains_proven(
        (&placed, cuts.as_ref()),
        plate_under(&placed),
        &asked,
        |_| !cancel.asked(),
    )
    .map_err(|error| ApplyError::Failed {
        object: id,
        reason: error.to_string(),
    })?;
    found.ok_or(ApplyError::Cancelled)
}
