use core_raster::Run;

use crate::{FormatError, ReadSeek};

/// What a sliced file says about itself, whoever wrote it.
///
/// Every field is what the container states rather than what we would have written, so a
/// file from another slicer reports that slicer's numbers. A field the container has no
/// place for is `None`, which is itself worth showing: it is why two files of the same
/// stack differ.
#[derive(Debug, Clone, PartialEq)]
pub struct SlicedFile {
    /// Extension of the container, without the dot.
    pub format: &'static str,
    /// Revision the container states, where it states one.
    pub version: Option<u32>,
    /// Machine the file names, and the software that wrote it.
    pub machine: Option<String>,
    pub slicer: Option<String>,
    pub resin: Option<String>,
    pub width_px: u32,
    pub height_px: u32,
    /// Panel size in millimetres, where the container records it.
    pub display_mm: Option<(f32, f32)>,
    /// The layer height the header states, millimetres.
    pub layer_height_mm: f32,
    pub exposure_s: f32,
    pub bottom_exposure_s: f32,
    pub bottom_layers: u32,
    pub print_time_s: Option<u32>,
    /// Resin the file says the stack takes, cubic millimetres.
    pub volume_mm3: Option<f32>,
    /// Greys the layer data can carry: 256 for a `.sl1`, 16 for an Anycubic file.
    pub grey_steps: u16,
    /// One entry per layer, in print order.
    pub layers: Vec<LayerEntry>,
}

impl SlicedFile {
    pub fn layer_count(&self) -> u32 {
        self.layers.len() as u32
    }

    /// The height of the stack the entries add up to, or the header's own product when the
    /// container records no per-layer Z.
    pub fn height_mm(&self) -> f32 {
        match self.layers.last() {
            Some(last) if last.z_mm > 0.0 => last.z_mm,
            _ => self.layer_height_mm * self.layer_count() as f32,
        }
    }
}

/// What a container's layer table says about one layer.
///
/// This is held for every layer, because it is how a reader reaches one: the table is the
/// index. It is deliberately the smallest thing that can be — a stack of ten thousand
/// layers costs a few hundred kilobytes and no mask is held at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerEntry {
    /// Top of the layer above the plate, millimetres, or zero where the container records
    /// only a thickness.
    pub z_mm: f32,
    pub exposure_s: f32,
    /// Where this layer's data begins and how long it is. For a container of separate
    /// images the offset has no meaning and is zero.
    pub offset: u64,
    pub size: u32,
}

/// Opens a sliced file and reads back what it says, without holding a mask.
///
/// The mirror of `SlicedFileWriter`. Four containers implement it, which is what makes it a
/// trait rather than four functions; see `docs/decisions/0149`.
pub trait SlicedFileReader {
    /// What this reader's open file is, over the source it took.
    type Open<S: ReadSeek>: OpenFile;

    /// Extension of the container this reads, without the dot.
    fn extension(&self) -> &'static str;

    /// Reads everything but the layer masks.
    ///
    /// The open file takes the source, so `&mut File` reads the tables and hands the file
    /// back, and `Box<dyn ReadSeek>` keeps it open for as long as layers are wanted.
    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError>;
}

/// A sliced file with its tables read and its layers still in it.
pub trait OpenFile {
    /// What the file says about itself.
    fn facts(&self) -> &SlicedFile;

    /// The runs of one layer, in the eight-bit grey the container decodes to.
    ///
    /// Layers are read one at a time and on demand, so opening a file costs its tables and
    /// nothing more.
    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError>;
}

/// Checks `index` against a file of `layer_count` layers, so every reader reports the same
/// error for the same mistake.
pub fn layer_in_range(index: u32, layer_count: u32) -> Result<(), FormatError> {
    if index >= layer_count {
        return Err(FormatError::NoSuchLayer { index, layer_count });
    }
    Ok(())
}

/// Most pixels a panel stated by a file may claim.
///
/// The largest panel in the catalogue is 15120x6230, about 94 megapixels, so this leaves
/// room to spare and still refuses a header whose only purpose is the mask it would make
/// us allocate.
pub const MAX_PANEL_PX: u32 = 256 * 1024 * 1024;

/// The pixel count of a panel a container states, checked before anything is sized from
/// it, so that no reader multiplies two header fields into an overflow.
pub fn panel_in_range(width_px: u32, height_px: u32) -> Result<u32, FormatError> {
    match width_px.checked_mul(height_px) {
        Some(pixels) if pixels <= MAX_PANEL_PX => Ok(pixels),
        _ => Err(FormatError::PanelTooLarge {
            width_px,
            height_px,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_whose_product_overflows_is_refused_rather_than_wrapped() {
        let err = panel_in_range(0x4412_FD01, 0x007A_5300).unwrap_err();
        assert!(matches!(err, FormatError::PanelTooLarge { .. }));
    }

    #[test]
    fn the_largest_panel_sold_is_still_read() {
        // ELEGOO Jupiter 2, the largest panel in the catalogue.
        assert_eq!(
            panel_in_range(15_120, 6_230).expect("a panel in the catalogue is a panel"),
            94_197_600
        );
    }
}
