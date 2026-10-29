use core_format::{FormatError, ReadSeek, Reads, SlicedFileReader, panel_in_range, stated_pwm};

use super::header::MAGIC;
use crate::fdg::{self, Head, MotionAt, OpenFdg};

/// Fields of the header, by their offset; the whole list is in `docs/formats/phz.md`.
const LAYER_TABLE: u64 = 0x24;
const PRINT_TIME: u64 = 0x30;
const PWM: u64 = 0x3C;
const BED: u64 = 0x4C;
const MACHINE_NAME: u64 = 0x90;
const VOLUME: u64 = 0x80;
const MOTION: MotionAt = MotionAt {
    light_off: 0x5C,
    lift: 0x6C,
    second_stage: 0x98,
};

/// Reads the `.phz` container.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhzReader;

impl SlicedFileReader for PhzReader {
    type Open<S: ReadSeek> = OpenFdg<S>;

    fn extension(&self) -> &'static str {
        "phz"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);
        fdg::expect_magic(&mut reads, MAGIC, "phz")?;
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

        let head = Head {
            format: "phz",
            version,
            width_px,
            height_px,
            pixel_count,
            layer_height_mm,
            exposure_s,
            bottom_exposure_s,
            bottom_layers,
            print_time_s,
            bed,
            key,
            volume_ml,
            light_pwm,
            bottom_light_pwm,
            table,
            layer_count,
        };
        fdg::open_layout(reads, head, MACHINE_NAME, MOTION, crypt)
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
    core_format::chitu_keystream_xor(data, word, step);
}
