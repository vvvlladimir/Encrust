//! The archive's two binary entries: `scene.slice`, the extent of the model and of each
//! layer, and `calc_layer_volumes.data`, the area each layer cures.

use core_format::{Fields, FormatError, PrintJob};

use super::built;

/// What a firmware checks the archive is.
const SCENE_MARK: &str = "ANYCUBIC-PWSZ";

/// Bytes of the scene entry's reserved words, between the model's extent and the rows.
const SCENE_RESERVED_BYTES: usize = 64 * 4;

/// What the volumes entry opens with.
const VOLUMES_MARK: u32 = 0x2025_1024;

/// A thickness the volumes entry states behind its mark, millimetres; the vendor's slicer
/// writes it as a constant.
const VOLUMES_STEP_MM: f32 = 0.2;

/// One layer as the scene entry records it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SceneRow {
    /// Top of the layer, millimetres above the plate.
    top_mm: f32,
    area_mm2: f32,
}

impl SceneRow {
    /// Layer `index` of `job`, curing `cured_px` pixels.
    pub(crate) fn of(job: &PrintJob, index: u32, cured_px: f32) -> Self {
        Self {
            top_mm: job.layer_z_mm(index),
            area_mm2: cured_px * job.raster.pitch.x * job.raster.pitch.y,
        }
    }
}

/// The scene entry: the model's extent, then a row per layer between two marks. As the
/// vendor writes it, every extent is zero and a layer is one piece as large as itself.
pub(crate) fn scene_slice(job: &PrintJob, rows: &[SceneRow]) -> Result<Vec<u8>, FormatError> {
    built(|fields| {
        fields.text(SCENE_MARK, 16)?;
        fields.text(&format!("Encrust {}", env!("CARGO_PKG_VERSION")), 64)?;
        // Binary type, version, slice type and model unit, as both writers read here state them.
        for word in [3, 1, 0, 0] {
            fields.u32_le(word)?;
        }
        fields.f32_le(1.0)?;
        fields.u32_le(job.layer_count())?;
        fields.zeros(7 * 4)?;
        fields.zeros(SCENE_RESERVED_BYTES)?;
        fields.bytes(b"<---")?;
        fields.u32_le(rows.len() as u32)?;
        for row in rows {
            write_scene_row(fields, *row)?;
        }
        fields.bytes(b"--->")
    })
}

fn write_scene_row(fields: &mut Fields<'_>, row: SceneRow) -> std::io::Result<()> {
    fields.f32_le(row.top_mm)?;
    fields.f32_le(row.area_mm2)?;
    fields.zeros(4 * 4)?;
    fields.u32_le(1)?;
    fields.f32_le(row.area_mm2)?;
    fields.zeros(8 * 4)
}

/// The volumes entry: a row per layer stating its top, the sliced layer height and the area
/// it cures, as the vendor's slicer writes it.
pub(crate) fn layer_volumes(job: &PrintJob, rows: &[SceneRow]) -> Result<Vec<u8>, FormatError> {
    built(|fields| {
        fields.u32_le(VOLUMES_MARK)?;
        fields.f32_le(VOLUMES_STEP_MM)?;
        fields.u32_le(1)?;
        fields.u32_le(rows.len() as u32)?;
        for (index, row) in rows.iter().enumerate() {
            fields.u32_le(index as u32 + 1)?;
            fields.f32_le(row.top_mm)?;
            fields.f32_le(job.nominal_height_mm())?;
            fields.zeros(8 * 4)?;
            fields.f32_le(row.area_mm2)?;
        }
        Ok(())
    })
}
