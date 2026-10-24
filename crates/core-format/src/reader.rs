use core_raster::Run;
use printer_profiles::{MaterialProfile, PrinterTuning};

use crate::{FormatError, ReadSeek};

/// What a sliced file says about itself, whoever wrote it.
/// Fields are what the container states; `None` means the container has no place for it.
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
    /// The rest of the resin the container records: motion, light, waits and transition.
    /// The exposure fields stay `None`, being the ones above; see `SlicedFile::tuning`.
    pub stated: PrinterTuning,
    /// One entry per layer, in print order.
    pub layers: Vec<LayerEntry>,
}

impl SlicedFile {
    pub fn layer_count(&self) -> u32 {
        self.layers.len() as u32
    }

    /// Every resin setting the file states, its exposure included.
    pub fn tuning(&self) -> PrinterTuning {
        PrinterTuning {
            layer_height_mm: Some(self.layer_height_mm),
            exposure_s: Some(self.exposure_s),
            bottom_exposure_s: Some(self.bottom_exposure_s),
            bottom_layers: Some(self.bottom_layers),
            ..self.stated.clone()
        }
    }

    /// The resin the file was printed with, named as it names it or else `fallback`, and
    /// on our defaults wherever the container has no place for a setting.
    pub fn resin(&self, fallback: &str) -> MaterialProfile {
        let name = match self.resin.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => name,
            _ => fallback,
        };
        MaterialProfile {
            name: name.to_owned(),
            ..MaterialProfile::default()
        }
        .with_tuning(&self.tuning())
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

/// What a container's layer table says about one layer; held for every layer, as the table is the
/// index. Deliberately small: ten thousand layers cost a few hundred kilobytes and no mask.
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

/// Most pixels a panel stated by a file may claim. The largest catalogue panel is 15120x6230, about
/// 94 megapixels; larger refuses a header-driven allocation.
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

    fn stated_file(resin: Option<&str>) -> SlicedFile {
        SlicedFile {
            format: "goo",
            version: Some(30),
            machine: None,
            slicer: None,
            resin: resin.map(str::to_owned),
            width_px: 16,
            height_px: 8,
            display_mm: None,
            layer_height_mm: 0.035,
            exposure_s: 2.37,
            bottom_exposure_s: 31.3,
            bottom_layers: 7,
            print_time_s: None,
            volume_mm3: None,
            grey_steps: 256,
            stated: PrinterTuning {
                lift_speed_mm_min: Some(61.0),
                light_pwm: Some(217),
                ..PrinterTuning::default()
            },
            layers: Vec::new(),
        }
    }

    #[test]
    fn a_resin_from_a_file_takes_what_it_states_and_keeps_our_defaults_for_the_rest() {
        let resin = stated_file(Some("  ENCRUST-TEST ")).resin("cube");
        let ours = MaterialProfile::default();

        assert_eq!(resin.name, "ENCRUST-TEST");
        assert_eq!(
            (
                resin.layer_height_mm,
                resin.exposure_s,
                resin.bottom_exposure_s
            ),
            (0.035, 2.37, 31.3)
        );
        assert_eq!(resin.bottom_layers, 7);
        assert_eq!((resin.lift_speed_mm_min, resin.light_pwm), (61.0, 217));
        assert_eq!(resin.retract_speed_mm_min, ours.retract_speed_mm_min);
        assert_eq!(resin.waits, ours.waits);
    }

    #[test]
    fn a_file_naming_no_resin_names_it_after_itself() {
        assert_eq!(stated_file(None).resin("cube").name, "cube");
        assert_eq!(stated_file(Some(" ")).resin("cube").name, "cube");
    }

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
