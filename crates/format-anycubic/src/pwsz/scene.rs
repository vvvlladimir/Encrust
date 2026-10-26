//! The archive's two binary entries: `scene.slice`, the extent of the model and of each
//! layer, and `calc_layer_volumes.data`, the resin slab by slab.

use core_analysis::cure;
use core_format::{Fields, FormatError, PrintJob};
use core_raster::{LayerRuns, PixelPitch};

use super::built;

/// What a firmware checks the archive is.
const SCENE_MARK: &str = "ANYCUBIC-PWSZ";

/// Bytes of the scene entry's reserved words, between the model's extent and the rows.
const SCENE_RESERVED_BYTES: usize = 64 * 4;

/// What the volumes entry opens with.
const VOLUMES_MARK: u32 = 0x2025_1024;

/// Thickness the volumes entry groups layers into, millimetres.
const SLAB_MM: f32 = 0.2;

/// Raw volume one millilitre of resin is stated as: the firmware multiplies by 0.0011.
const RAW_PER_MM3: f32 = 1.0 / 1.1;

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

/// First and last lit column and row of a layer.
fn lit_box(layer: &LayerRuns) -> Option<[u32; 4]> {
    let width = u64::from(layer.width());
    let mut found: Option<[u32; 4]> = None;
    let mut at = 0_u64;
    for run in layer.runs() {
        let length = u64::from(run.length);
        if run.value > 0 && length > 0 {
            let (first, last) = (at, at + length - 1);
            let (row_0, row_1) = ((first / width) as u32, (last / width) as u32);
            let (column_0, column_1) = if row_0 == row_1 {
                ((first % width) as u32, (last % width) as u32)
            } else {
                (0, (width - 1) as u32)
            };
            let [x0, y0, x1, y1] = found.unwrap_or([column_0, row_0, column_1, row_1]);
            found = Some([
                x0.min(column_0),
                y0.min(row_0),
                x1.max(column_1),
                y1.max(row_1),
            ]);
        }
        at += length;
    }
    found
}

/// One layer as the scene entry records it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SceneRow {
    /// Top of the layer, millimetres above the plate.
    top_mm: f32,
    thickness_mm: f32,
    area_mm2: f32,
    /// Lit extent, millimetres from the panel's centre: left, top, right, bottom.
    box_mm: Option<[f32; 4]>,
    pieces: u32,
    largest_piece_mm2: f32,
}

impl SceneRow {
    pub(crate) fn of(job: &PrintJob, index: u32, outline: &Outline) -> Self {
        let pitch = job.raster.pitch;
        let pixel_mm2 = pitch.x * pitch.y;
        let (half_x, half_y) = (
            job.raster.width_px as f32 * pitch.x / 2.0,
            job.raster.height_px as f32 * pitch.y / 2.0,
        );
        // The last lit pixel's far edge is where the box ends, hence the `+ 1`.
        let box_mm = outline.box_px.map(|[x0, y0, x1, y1]| {
            [
                x0 as f32 * pitch.x - half_x,
                y0 as f32 * pitch.y - half_y,
                (x1 + 1) as f32 * pitch.x - half_x,
                (y1 + 1) as f32 * pitch.y - half_y,
            ]
        });
        Self {
            top_mm: job.layer_z_mm(index),
            thickness_mm: job.layer_height_mm(index),
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
        .reduce(|[a0, b0, c0, d0], [a1, b1, c1, d1]| {
            [a0.min(a1), b0.min(b1), c0.max(c1), d0.max(d1)]
        })
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

/// The volumes entry: the stack cut into slabs of about 0.2 mm, each with the resin it
/// takes.
pub(crate) fn layer_volumes(rows: &[SceneRow]) -> Result<Vec<u8>, FormatError> {
    let slabs = slabs(rows);
    built(|fields| {
        fields.u32_le(VOLUMES_MARK)?;
        fields.f32_le(SLAB_MM)?;
        fields.u32_le(1)?;
        fields.u32_le(slabs.len() as u32)?;
        for slab in &slabs {
            fields.u32_le(slab.layer_ref)?;
            fields.f32_le(slab.bottom_mm)?;
            fields.f32_le(slab.thickness_mm)?;
            fields.zeros(8 * 4)?;
            fields.f32_le(slab.volume_mm3 * RAW_PER_MM3)?;
        }
        Ok(())
    })
}

/// Consecutive layers summed until they are a slab thick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slab {
    /// The layer before the slab's first, or the first for the slab at the plate.
    layer_ref: u32,
    bottom_mm: f32,
    thickness_mm: f32,
    volume_mm3: f32,
}

fn slabs(rows: &[SceneRow]) -> Vec<Slab> {
    let mut slabs: Vec<Slab> = Vec::new();
    let mut open: Option<Slab> = None;
    for (index, row) in rows.iter().enumerate() {
        let slab = open.get_or_insert(Slab {
            layer_ref: (index as u32).saturating_sub(1),
            bottom_mm: row.top_mm - row.thickness_mm,
            thickness_mm: 0.0,
            volume_mm3: 0.0,
        });
        slab.thickness_mm += row.thickness_mm;
        slab.volume_mm3 += row.area_mm2 * row.thickness_mm;
        // A micron short of the slab still closes it, as the heights are whole microns.
        if slab.thickness_mm >= SLAB_MM - 0.000_5 {
            slabs.extend(open.take());
        }
    }
    slabs.extend(open);
    slabs
}
