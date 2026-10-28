use core_format::{
    FormatError, LayerEntry, OpenFile, PrinterTuning, ReadSeek, Reads, SecondStage, SlicedFile,
    SlicedFileReader, WaitMode, Waits, layer_in_range, panel_in_range, stated_distance_mm,
    stated_pwm, stated_speed_mm_min, stated_wait_s,
};
use core_raster::Run;

use crate::header::HEADER_SIZE;
use crate::rle;
use crate::rle5::{decode_half, join_halves};
use crate::version::{ByteOrder, GooVersion};

mod v5;

/// The tag behind the version string, which is what says this is a `.goo` at all.
const MAGIC_TAG: [u8; 8] = [0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00];

/// Fields of the header we read, by their offset from the start of the file; the whole
/// list is in `docs/formats/goo.md`.
const SOFTWARE: u64 = 12;
const MACHINE: u64 = 92;
const RESIN: u64 = 156;
const GEOMETRY: u64 = 195_310;
const LAYER_HEIGHT: u64 = 195_332;
const TOTALS: u64 = 195_446;
const TRANSITION: u64 = 195_475;

/// Bytes of one layer's definition, up to the data size field behind it.
const DEFINITION_BYTES: u64 = 66;

/// Reads the Elegoo `.goo` container. Layout is in `docs/formats/goo.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct GooReader;

impl SlicedFileReader for GooReader {
    type Open<S: ReadSeek> = OpenGoo<S>;

    fn extension(&self) -> &'static str {
        "goo"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);

        let version = reads.text(4)?;
        let revision = if version == GooVersion::V5.tag() {
            GooVersion::V5
        } else {
            GooVersion::V3
        };
        let order = revision.order();
        if reads.array::<8>()? != MAGIC_TAG {
            // The tag is eight bytes where every other container has a word, so the first
            // four of it are what a mismatch can report.
            reads.seek_to(4)?;
            return Err(FormatError::NotThisFormat {
                format: "goo",
                found: reads.u32_be()?,
            });
        }

        reads.seek_to(SOFTWARE)?;
        let slicer = format!("{} {}", reads.text(32)?, reads.text(24)?);
        reads.seek_to(MACHINE)?;
        let machine = reads.text(32)?;
        reads.seek_to(RESIN)?;
        let resin = reads.text(32)?;

        reads.seek_to(GEOMETRY)?;
        let layer_count = order.u32(&mut reads)?;
        let width_px = u32::from(order.u16(&mut reads)?);
        let height_px = u32::from(order.u16(&mut reads)?);
        panel_in_range(width_px, height_px)?;
        reads.skip(2)?;
        let display_mm = (order.f32(&mut reads)?, order.f32(&mut reads)?);

        reads.seek_to(LAYER_HEIGHT)?;
        let exposure = read_exposure(&mut reads, order)?;
        let mut stated = read_motion(&mut reads, order)?;
        stated.retract_distance_mm = stated
            .retract_distance_mm
            .map(|mm| revision.resin_retract_mm(mm, exposure.layer_height_mm));
        stated.waits = exposure.waits;
        stated.light_off_delay_s = exposure.light_off_delay_s;

        reads.seek_to(TOTALS)?;
        let print_time_s = order.u32(&mut reads)?;
        let volume_mm3 = order.f32(&mut reads)?;
        reads.seek_to(TRANSITION)?;
        stated.transition_layers = Some(order.u16(&mut reads)?);

        let (layers, right_halves) = match revision {
            GooVersion::V3 => (walk_layers(&mut reads, layer_count)?, Vec::new()),
            GooVersion::V5 => v5::walk_tables(&mut reads, layer_count)?,
        };
        Ok(OpenGoo {
            reads,
            right_halves,
            half_px: width_px / 2,
            facts: SlicedFile {
                format: "goo",
                version: version.strip_prefix('V').and_then(parse_version),
                machine: Some(machine),
                slicer: Some(slicer.trim().to_owned()),
                resin: Some(resin),
                width_px,
                height_px,
                display_mm: Some(display_mm),
                layer_height_mm: exposure.layer_height_mm,
                exposure_s: exposure.exposure_s,
                bottom_exposure_s: exposure.bottom_exposure_s,
                bottom_layers: exposure.bottom_layers,
                print_time_s: Some(print_time_s),
                volume_mm3: Some(volume_mm3),
                grey_steps: match revision {
                    GooVersion::V3 => 256,
                    GooVersion::V5 => 8,
                },
                stated,
                layers,
            },
        })
    }
}

/// The header's exposure block, from the layer height on.
struct ExposureBlock {
    layer_height_mm: f32,
    exposure_s: f32,
    bottom_exposure_s: f32,
    bottom_layers: u32,
    waits: Option<Waits>,
    light_off_delay_s: Option<f32>,
}

fn read_exposure<S: ReadSeek>(
    reads: &mut Reads<S>,
    order: ByteOrder,
) -> Result<ExposureBlock, FormatError> {
    let layer_height_mm = order.f32(reads)?;
    let exposure_s = order.f32(reads)?;
    // The delay mode byte is 1 in every vendor file; which wait is unused is told by its zeros.
    reads.skip(1)?;
    let light_off_delay_s = stated_wait_s(order.f32(reads)?);
    // Six rests, the bottom layers' three first; a resin holds one set, the common one.
    reads.skip(12)?;
    let rests = [order.f32(reads)?, order.f32(reads)?, order.f32(reads)?].map(stated_wait_s);
    let waits = match rests {
        [
            Some(before_lift_s),
            Some(after_lift_s),
            Some(after_retract_s),
        ] => Some(Waits {
            mode: if before_lift_s + after_lift_s + after_retract_s > 0.0 {
                WaitMode::Rest
            } else {
                WaitMode::LightOff
            },
            before_lift_s,
            after_lift_s,
            after_retract_s,
        }),
        _ => None,
    };
    Ok(ExposureBlock {
        layer_height_mm,
        exposure_s,
        bottom_exposure_s: order.f32(reads)?,
        bottom_layers: order.u32(reads)?,
        waits,
        light_off_delay_s,
    })
}

/// The header's motion block, which follows the exposure: each peel, then the light.
fn read_motion<S: ReadSeek>(
    reads: &mut Reads<S>,
    order: ByteOrder,
) -> Result<PrinterTuning, FormatError> {
    let mut peel = [0.0; 8];
    for value in &mut peel {
        *value = order.f32(reads)?;
    }
    let [
        bottom_lift,
        bottom_lift_speed,
        lift,
        lift_speed,
        _,
        bottom_retract_speed,
        retract,
        retract_speed,
    ] = peel;
    let mut second = [0.0; 8];
    for value in &mut second {
        *value = order.f32(reads)?;
    }
    let [
        bottom_lift_distance_mm,
        bottom_lift_speed_mm_min,
        lift_distance_mm,
        lift_speed_mm_min,
        bottom_retract_distance_mm,
        bottom_retract_speed_mm_min,
        retract_distance_mm,
        retract_speed_mm_min,
    ] = second;
    let stage = SecondStage {
        lift_distance_mm,
        lift_speed_mm_min,
        retract_distance_mm,
        retract_speed_mm_min,
        bottom_lift_distance_mm,
        bottom_lift_speed_mm_min,
        bottom_retract_distance_mm,
        bottom_retract_speed_mm_min,
    };
    Ok(PrinterTuning {
        second_stage: stage.is_used().then_some(stage),
        bottom_lift_distance_mm: stated_distance_mm(bottom_lift),
        bottom_lift_speed_mm_min: stated_speed_mm_min(bottom_lift_speed),
        lift_distance_mm: stated_distance_mm(lift),
        lift_speed_mm_min: stated_speed_mm_min(lift_speed),
        bottom_retract_speed_mm_min: stated_speed_mm_min(bottom_retract_speed),
        retract_distance_mm: stated_distance_mm(retract),
        retract_speed_mm_min: stated_speed_mm_min(retract_speed),
        bottom_light_pwm: stated_pwm(order.u16(reads)?),
        light_pwm: stated_pwm(order.u16(reads)?),
        ..PrinterTuning::default()
    })
}

/// `V3.0` as 30, the number the header's own field would hold.
fn parse_version(text: &str) -> Option<u32> {
    let (major, minor) = text.split_once('.')?;
    Some(major.parse::<u32>().ok()? * 10 + minor.parse::<u32>().ok()?)
}

/// Walks the layers from the front, because the container has no table: each layer's
/// definition states its own size and so says where the next one begins.
fn walk_layers<S: ReadSeek>(
    reads: &mut Reads<S>,
    layer_count: u32,
) -> Result<Vec<LayerEntry>, FormatError> {
    // The shortest a layer can be is its definition, the size field and the two bytes the
    // size counts beside the runs, which is what says a count is impossible.
    let claimed = reads.claim("layers", u64::from(layer_count), DEFINITION_BYTES + 6)?;
    let mut layers = Vec::with_capacity(claimed);
    let mut at = u64::from(HEADER_SIZE);

    for _ in 0..layer_count {
        reads.seek_to(at + 6)?;
        let z_mm = reads.f32_be()?;
        let exposure_s = reads.f32_be()?;

        reads.seek_to(at + DEFINITION_BYTES)?;
        let data_size = reads.u32_be()?;

        // The size counts the magic byte in front of the runs and the checksum behind
        // them, so the runs themselves are two bytes shorter.
        let runs = data_size.checked_sub(2).ok_or(FormatError::MalformedRun {
            offset: (at + DEFINITION_BYTES) as usize,
        })?;
        layers.push(LayerEntry {
            z_mm,
            exposure_s,
            offset: at + DEFINITION_BYTES + 5,
            size: runs,
        });
        at += DEFINITION_BYTES + 4 + u64::from(data_size) + 2;
    }
    Ok(layers)
}

/// A `.goo` with its layer offsets walked and its masks still in it.
pub struct OpenGoo<S> {
    reads: Reads<S>,
    facts: SlicedFile,
    /// Version 5.1 only: where each layer's right half lies, the left being in `facts`.
    right_halves: Vec<LayerEntry>,
    half_px: u32,
}

impl<S: ReadSeek> OpenFile for OpenGoo<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let data = self.reads.bytes(entry.size as usize)?;
        let Some(right) = self.right_halves.get(index as usize).copied() else {
            return rle::decode(&data);
        };
        self.reads.seek_to(right.offset)?;
        let right = decode_half(&self.reads.bytes(right.size as usize)?)?;
        join_halves(
            &decode_half(&data)?,
            &right,
            self.half_px,
            self.facts.height_px,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn a_file_that_is_not_a_goo_is_refused_by_its_tag() {
        let mut source = Cursor::new(vec![0x11; 64]);
        let err = GooReader
            .open(&mut source)
            .err()
            .expect("the tag does not match");
        assert!(matches!(
            err,
            FormatError::NotThisFormat {
                format: "goo",
                found: 0x1111_1111
            }
        ));
    }

    /// An exposure block from the layer height on: delay mode 1, then the seven waits.
    fn exposure_block(light_off_s: f32, rests_s: [f32; 3]) -> Cursor<Vec<u8>> {
        let mut bytes = [0.05_f32, 2.5].map(f32::to_be_bytes).concat();
        bytes.push(1);
        for value in [light_off_s].into_iter().chain(rests_s).chain(rests_s) {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(30.0_f32.to_be_bytes());
        bytes.extend(2_u32.to_be_bytes());
        Cursor::new(bytes)
    }

    #[test]
    fn a_vendor_file_holding_the_light_off_reads_as_light_off() {
        let block = read_exposure(
            &mut Reads::new(exposure_block(0.5, [0.0; 3])),
            ByteOrder::Big,
        )
        .expect("a whole block");
        let waits = block.waits.expect("three rests stated");
        assert_eq!(waits.mode, WaitMode::LightOff, "no rest is stated");
        assert_eq!(block.light_off_delay_s, Some(0.5));
    }

    #[test]
    fn a_vendor_file_resting_reads_as_resting() {
        let block = read_exposure(
            &mut Reads::new(exposure_block(0.0, [2.0, 3.0, 4.0])),
            ByteOrder::Big,
        )
        .expect("a whole block");
        let waits = block.waits.expect("three rests stated");
        assert_eq!(waits.mode, WaitMode::Rest);
        assert_eq!(
            [
                waits.before_lift_s,
                waits.after_lift_s,
                waits.after_retract_s
            ],
            [2.0, 3.0, 4.0]
        );
    }

    #[test]
    fn the_version_string_becomes_the_number_the_field_would_hold() {
        assert_eq!(parse_version("3.0"), Some(30));
        assert_eq!(parse_version("3"), None);
    }
}
