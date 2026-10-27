use core_format::{
    FormatError, OpenFile, PrinterTuning, ReadSeek, Reads, SlicedFile, SlicedFileReader,
    layer_in_range, panel_in_range, stated_pwm,
};
use core_raster::Run;

use super::header::MAGIC;
use crate::fdg::{self, MotionAt};

/// Fields of the header, by their offset; the whole list is in `docs/formats/phz.md`.
const LAYER_TABLE: u64 = 0x24;
const PRINT_TIME: u64 = 0x30;
const PWM: u64 = 0x3C;
const BED: u64 = 0x4C;
const MACHINE_NAME: u64 = 0x90;
const VOLUME: u64 = 0x80;
const MOTION: MotionAt = MotionAt {
    light_off: 0x60,
    lift: 0x6C,
};

/// Reads the `.phz` container.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhzReader;

impl SlicedFileReader for PhzReader {
    type Open<S: ReadSeek> = OpenPhz<S>;

    fn extension(&self) -> &'static str {
        "phz"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);
        let magic = reads.u32_le()?;
        if magic != MAGIC {
            return Err(FormatError::NotThisFormat {
                format: "phz",
                found: magic,
            });
        }
        let version = reads.u32_le()?;
        let layer_height_mm = reads.f32_le()?;
        let exposure_s = reads.f32_le()?;
        let bottom_exposure_s = reads.f32_le()?;
        let bottom_layers = reads.u32_le()?;
        let width_px = reads.u32_le()?;
        let height_px = reads.u32_le()?;
        let pixel_count = panel_in_range(width_px, height_px)?;

        reads.seek_to(LAYER_TABLE)?;
        let table = u64::from(reads.u32_le()?);
        let layer_count = reads.u32_le()?;
        reads.seek_to(PRINT_TIME)?;
        let print_time_s = reads.u32_le()?;
        reads.seek_to(PWM)?;
        let light_pwm = stated_pwm(reads.u16_le()?);
        let bottom_light_pwm = stated_pwm(reads.u16_le()?);

        reads.seek_to(BED)?;
        let bed = (reads.f32_le()?, reads.f32_le()?);
        reads.skip(4)?;
        let key = reads.u32_le()?;
        reads.seek_to(VOLUME)?;
        let volume_ml = reads.f32_le()?;

        let machine = fdg::read_machine_name(&mut reads, MACHINE_NAME)?;
        let layers = fdg::read_table(&mut reads, table, layer_count)?;
        let stated = PrinterTuning {
            light_pwm,
            bottom_light_pwm,
            ..fdg::read_motion(&mut reads, layers.first(), MOTION)?
        };

        Ok(OpenPhz {
            reads,
            facts: SlicedFile {
                format: "phz",
                version: Some(version),
                machine,
                slicer: None,
                resin: None,
                width_px,
                height_px,
                display_mm: (bed.0 > 0.0 && bed.1 > 0.0).then_some(bed),
                layer_height_mm,
                exposure_s,
                bottom_exposure_s,
                bottom_layers,
                print_time_s: Some(print_time_s),
                volume_mm3: Some(volume_ml * 1000.0),
                grey_steps: 2,
                stated,
                layers,
            },
            key,
            pixel_count,
        })
    }
}

/// Covers or uncovers a layer's bytes with this container's keyed exclusive or; a key of
/// zero leaves them in the clear.
pub(crate) fn crypt(key: u32, index: u32, data: &mut [u8]) {
    if key == 0 {
        return;
    }
    let step = (key % 0x4324).wrapping_mul(0x34A3_2231);
    let word = step.wrapping_mul((index ^ 0x3FAD_2212).wrapping_mul(0x1E15_30CD));
    fdg::cover(data, word, step);
}

/// A `.phz` file with its table read and its masks still in it.
pub struct OpenPhz<S> {
    reads: Reads<S>,
    facts: SlicedFile,
    /// Key the layer bytes are covered with, or zero when they are in the clear.
    key: u32,
    pixel_count: u32,
}

impl<S: ReadSeek> OpenFile for OpenPhz<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let mut data = self.reads.bytes(entry.size as usize)?;
        crypt(self.key, index, &mut data);
        fdg::decode(&data, self.pixel_count)
    }
}
