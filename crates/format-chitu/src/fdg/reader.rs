use core_format::{
    FormatError, LayerEntry, OpenFile, PrinterTuning, ReadSeek, Reads, SlicedFile,
    SlicedFileReader, layer_in_range, panel_in_range, stated_distance_mm, stated_pwm, stated_rests,
    stated_speed_mm_min, stated_wait_s,
};
use core_raster::Run;

use super::header::MAGIC;
use super::rle;
use crate::layer::{LAYER_DEF_BYTES, LAYER_DEF_EX_BYTES};

/// Fields of the header, by their offset; the whole list is in `docs/formats/fdg.md`.
const RESOLUTION: u64 = 0x18;
const LAYER_TABLE: u64 = 0x34;
const BED: u64 = 0x50;
const MACHINE_NAME: u64 = 0x74;
const MOTION: MotionAt = MotionAt {
    light_off: 0x80,
    lift: 0x88,
};
const TRANSITION: u64 = 0xC8;
const BOTTOM_RETRACT_SPEED: u64 = 0xD8;

/// Where the three rests sit in the record in front of a layer's data, counted back from
/// the data; the header holds only two of them.
const RESTS_BEFORE_DATA: u64 = (LAYER_DEF_EX_BYTES - 0x44) as u64;

/// Where a header of this layout puts its light-off delay and its five lift and retract
/// numbers; the transition and the bottom retract speed sit at one offset in both.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MotionAt {
    pub light_off: u64,
    pub lift: u64,
}

/// Reads the `.fdg` container.
#[derive(Debug, Default, Clone, Copy)]
pub struct FdgReader;

impl SlicedFileReader for FdgReader {
    type Open<S: ReadSeek> = OpenFdg<S>;

    fn extension(&self) -> &'static str {
        "fdg"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);
        let magic = reads.u32_le()?;
        if magic != MAGIC {
            return Err(FormatError::NotThisFormat {
                format: "fdg",
                found: magic,
            });
        }
        let version = reads.u32_le()?;
        let layer_count = reads.u32_le()?;
        let bottom_layers = reads.u32_le()?;

        reads.seek_to(RESOLUTION)?;
        let width_px = reads.u32_le()?;
        let height_px = reads.u32_le()?;
        let pixel_count = panel_in_range(width_px, height_px)?;
        let layer_height_mm = reads.f32_le()?;
        let exposure_s = reads.f32_le()?;
        let bottom_exposure_s = reads.f32_le()?;

        reads.seek_to(LAYER_TABLE)?;
        let table = u64::from(reads.u32_le()?);
        let print_time_s = reads.u32_le()?;
        reads.skip(4)?;
        let bottom_light_pwm = stated_pwm(reads.u16_le()?);
        let light_pwm = stated_pwm(reads.u16_le()?);

        reads.seek_to(BED)?;
        let bed = (reads.f32_le()?, reads.f32_le()?);
        reads.skip(4)?;
        let key = reads.u32_le()?;
        reads.skip(8)?;
        let volume_ml = reads.f32_le()?;

        let machine = read_machine_name(&mut reads, MACHINE_NAME)?;
        let layers = read_table(&mut reads, table, layer_count)?;
        let stated = PrinterTuning {
            light_pwm,
            bottom_light_pwm,
            ..read_motion(&mut reads, layers.first(), MOTION)?
        };

        Ok(OpenFdg {
            reads,
            facts: SlicedFile {
                format: "fdg",
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

/// The machine name whose address and length stand at `field`.
pub(crate) fn read_machine_name<S: ReadSeek>(
    reads: &mut Reads<S>,
    field: u64,
) -> Result<Option<String>, FormatError> {
    reads.seek_to(field)?;
    let at = u64::from(reads.u32_le()?);
    let length = reads.u32_le()? as usize;
    if at == 0 || length == 0 {
        return Ok(None);
    }
    reads.seek_to(at)?;
    Ok(Some(reads.text(length)?))
}

pub(crate) fn read_motion<S: ReadSeek>(
    reads: &mut Reads<S>,
    first: Option<&LayerEntry>,
    at: MotionAt,
) -> Result<PrinterTuning, FormatError> {
    reads.seek_to(at.light_off)?;
    let light_off = reads.f32_le()?;
    reads.seek_to(at.lift)?;
    let mut motion = [0.0; 5];
    for value in &mut motion {
        *value = reads.f32_le()?;
    }
    let [
        bottom_lift,
        bottom_lift_speed,
        lift,
        lift_speed,
        retract_speed,
    ] = motion;

    reads.seek_to(TRANSITION)?;
    let transition_layers = Some(reads.u16_le()?);
    reads.seek_to(BOTTOM_RETRACT_SPEED)?;
    let bottom_retract_speed = reads.f32_le()?;
    let waits = match first {
        Some(entry) if entry.offset >= RESTS_BEFORE_DATA => {
            reads.seek_to(entry.offset - RESTS_BEFORE_DATA)?;
            let before_lift_s = reads.f32_le()?;
            let after_lift_s = reads.f32_le()?;
            stated_rests(before_lift_s, after_lift_s, reads.f32_le()?)
        }
        _ => None,
    };

    Ok(PrinterTuning {
        bottom_lift_distance_mm: stated_distance_mm(bottom_lift),
        bottom_lift_speed_mm_min: stated_speed_mm_min(bottom_lift_speed),
        lift_distance_mm: stated_distance_mm(lift),
        retract_distance_mm: stated_distance_mm(lift),
        lift_speed_mm_min: stated_speed_mm_min(lift_speed),
        retract_speed_mm_min: stated_speed_mm_min(retract_speed),
        bottom_retract_speed_mm_min: stated_speed_mm_min(bottom_retract_speed),
        light_off_delay_s: stated_wait_s(light_off),
        transition_layers,
        waits,
        ..PrinterTuning::default()
    })
}

pub(crate) fn read_table<S: ReadSeek>(
    reads: &mut Reads<S>,
    table: u64,
    layer_count: u32,
) -> Result<Vec<LayerEntry>, FormatError> {
    let rows = u64::from(layer_count);
    let mut entries = Vec::with_capacity(reads.claim("layer table rows", rows, LAYER_DEF_BYTES)?);
    for row in 0..rows {
        reads.seek_to(table + row * LAYER_DEF_BYTES)?;
        let z_mm = reads.f32_le()?;
        let exposure_s = reads.f32_le()?;
        reads.skip(4)?;
        let offset = u64::from(reads.u32_le()?);
        let size = reads.u32_le()?;
        entries.push(LayerEntry {
            z_mm,
            exposure_s,
            offset,
            size,
        });
    }
    Ok(entries)
}

/// An `.fdg` file with its table read and its masks still in it.
pub struct OpenFdg<S> {
    reads: Reads<S>,
    facts: SlicedFile,
    /// Key the layer bytes are covered with, or zero when they are in the clear.
    key: u32,
    pixel_count: u32,
}

impl<S: ReadSeek> OpenFile for OpenFdg<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let mut data = self.reads.bytes(entry.size as usize)?;
        rle::crypt(self.key, index, &mut data);
        rle::decode(&data, self.pixel_count)
    }
}
