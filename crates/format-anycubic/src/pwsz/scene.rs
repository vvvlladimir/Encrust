//! The archive's two binary entries: `scene.slice`, the extent of the model and of each
//! layer, and `calc_layer_volumes.data`, the area each layer cures.

use core_analysis::cure;
use core_format::{Fields, FormatError, PrintJob};
use core_raster::{LayerRuns, PixelPitch};

use super::built;
use crate::extent::{box_mm, lit_box, union};

/// What a firmware checks the archive is.
const SCENE_MARK: &str = "ANYCUBIC-PWSZ";

/// Bytes of the scene entry's reserved words, between the model's extent and the rows.
const SCENE_RESERVED_BYTES: usize = 64 * 4;

/// What the volumes entry opens with.
const VOLUMES_MARK: u32 = 0x2025_1024;

/// A thickness the volumes entry states behind its mark, millimetres; the vendor's slicer
/// writes it as a constant.
const VOLUMES_STEP_MM: f32 = 0.2;

/// Lit extent and pieces of one layer, in pixels: what the layer's row is made from once
/// the panel's pitch is known.
pub(crate) struct Outline {
    /// First and last lit column and row, or `None` for a dark layer.
    box_px: Option<[u32; 4]>,
    pieces: u32,
    area_px: f32,
    largest_piece_px: f32,
}

impl Outline {
    pub(crate) fn of(layer: &LayerRuns) -> Self {
        // A unit pitch makes every area a pixel count, scaled to the panel in `SceneRow::of`.
        let cured = cure(layer, PixelPitch { x: 1.0, y: 1.0 });
        let areas = cured.pieces().iter().map(|piece| piece.area_mm2);
        Self {
            box_px: lit_box(layer),
            pieces: cured.pieces().len() as u32,
            area_px: areas.clone().sum(),
            largest_piece_px: areas.fold(0.0, f32::max),
        }
    }
}

/// One layer as the scene entry records it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SceneRow {
    /// Top of the layer, millimetres above the plate.
    top_mm: f32,
    area_mm2: f32,
    /// Lit extent, millimetres from the panel's centre: left, top, right, bottom.
    box_mm: Option<[f32; 4]>,
    pieces: u32,
    largest_piece_mm2: f32,
}

impl SceneRow {
    pub(crate) fn of(job: &PrintJob, index: u32, outline: &Outline) -> Self {
        let pixel_mm2 = job.raster.pitch.x * job.raster.pitch.y;
        let box_mm = outline.box_px.map(|box_px| box_mm(job, box_px));
        Self {
            top_mm: job.layer_z_mm(index),
            area_mm2: outline.area_px * pixel_mm2,
            box_mm,
            pieces: outline.pieces,
            largest_piece_mm2: outline.largest_piece_px * pixel_mm2,
        }
    }
}

/// The scene entry: the model's extent, then a row per layer between two marks.
pub(crate) fn scene_slice(job: &PrintJob, rows: &[SceneRow]) -> Result<Vec<u8>, FormatError> {
    let model = rows
        .iter()
        .filter_map(|row| row.box_mm)
        .fold(None, |model, row| Some(union(model, row)))
        .unwrap_or_default();
    built(|fields| {
        fields.text(SCENE_MARK, 16)?;
        fields.text(&format!("Encrust {}", env!("CARGO_PKG_VERSION")), 64)?;
        // Binary type, version, slice type and model unit, as both writers read here state them.
        for word in [3, 1, 0, 0] {
            fields.u32_le(word)?;
        }
        fields.f32_le(1.0)?;
        fields.u32_le(job.layer_count())?;
        for value in [model[0], model[1], 0.0, model[2], model[3], job.height_mm()] {
            fields.f32_le(value)?;
        }
        fields.u32_le(0)?;
        fields.zeros(SCENE_RESERVED_BYTES)?;
        fields.bytes(b"<---")?;
        fields.u32_le(rows.len() as u32)?;
        for row in rows {
            write_scene_row(fields, row)?;
        }
        fields.bytes(b"--->")
    })
}

fn write_scene_row(fields: &mut Fields<'_>, row: &SceneRow) -> std::io::Result<()> {
    fields.f32_le(row.top_mm)?;
    fields.f32_le(row.area_mm2)?;
    for value in row.box_mm.unwrap_or_default() {
        fields.f32_le(value)?;
    }
    fields.u32_le(row.pieces)?;
    fields.f32_le(row.largest_piece_mm2)?;
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
