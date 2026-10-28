use core_format::{
    ChituPeel, FormatError, LayerEntry, OpenFile, PrinterTuning, ReadSeek, Reads, SlicedFile,
    SlicedFileReader, layer_in_range, panel_in_range, stated_pwm, stated_rests,
    stated_speed_mm_min, stated_wait_s,
};
use core_raster::Run;

use super::Crypt;
use super::header::MAGIC;
use super::rle;
use crate::layer::{LAYER_DEF_BYTES, LAYER_DEF_EX_BYTES};

/// Fields of the header, by their offset; the whole list is in `docs/formats/fdg.md`.
const RESOLUTION: u64 = 0x18;
const LAYER_TABLE: u64 = 0x34;
const BED: u64 = 0x50;
const MACHINE_NAME: u64 = 0x74;
const MOTION: MotionAt = MotionAt {
    light_off: 0x7C,
    lift: 0x88,
    second_stage: 0x9C,
};
const TRANSITION: u64 = 0xC8;
/// The copy of `.ctb`'s v4 motion: the bottom retract's two speeds lead it, the second
/// stage's bottom retract distance is its tenth word.
const V4_MOTION: u64 = 0xD8;

/// Where the three rests sit in the record in front of a layer's data, counted back from
/// the data; the header holds only two of them.
const RESTS_BEFORE_DATA: u64 = (LAYER_DEF_EX_BYTES - 0x44) as u64;

/// Where a header of this layout puts its two light-off delays, its five lift and retract
/// numbers and the second stage's six; the transition and the v4 block sit at one offset in
/// both.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MotionAt {
    pub light_off: u64,
    pub lift: u64,
    pub second_stage: u64,
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

        let head = Head {
            format: "fdg",
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
        open_layout(reads, head, MACHINE_NAME, MOTION, rle::crypt)
    }
}

/// What a header of this layout states, read off by its container's own reader.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Head {
    pub format: &'static str,
    pub version: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub pixel_count: u32,
    pub layer_height_mm: f32,
    pub exposure_s: f32,
    pub bottom_exposure_s: f32,
    pub bottom_layers: u32,
    pub print_time_s: u32,
    /// The bed's width and depth, mm.
    pub bed: (f32, f32),
    pub key: u32,
    pub volume_ml: f32,
    pub light_pwm: Option<u8>,
    pub bottom_light_pwm: Option<u8>,
    pub table: u64,
    pub layer_count: u32,
}

/// Reads what the rest of a file of this layout holds behind `head`: the machine name at
/// `machine_name`, the table and the peel at `motion`, its layers ciphered by `crypt`.
pub(crate) fn open_layout<S: ReadSeek>(
    mut reads: Reads<S>,
    head: Head,
    machine_name: u64,
    motion: MotionAt,
    crypt: Crypt,
) -> Result<OpenFdg<S>, FormatError> {
    let machine = read_machine_name(&mut reads, machine_name)?;
    let layers = read_table(&mut reads, head.table, head.layer_count)?;
    let stated = PrinterTuning {
        light_pwm: head.light_pwm,
        bottom_light_pwm: head.bottom_light_pwm,
        ..read_motion(&mut reads, layers.first(), motion)?
    };
    let bed = head.bed;
    Ok(OpenFdg {
        reads,
        facts: SlicedFile {
            format: head.format,
            version: Some(head.version),
            machine,
            slicer: None,
            resin: None,
            width_px: head.width_px,
            height_px: head.height_px,
            display_mm: (bed.0 > 0.0 && bed.1 > 0.0).then_some(bed),
            layer_height_mm: head.layer_height_mm,
            exposure_s: head.exposure_s,
            bottom_exposure_s: head.bottom_exposure_s,
            bottom_layers: head.bottom_layers,
            print_time_s: Some(head.print_time_s),
            volume_mm3: Some(head.volume_ml * 1000.0),
            grey_steps: 2,
            stated,
            layers,
        },
        key: head.key,
        pixel_count: head.pixel_count,
        crypt,
    })
}

/// The machine name whose address and length stand at `field`.
fn read_machine_name<S: ReadSeek>(
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

fn read_motion<S: ReadSeek>(
    reads: &mut Reads<S>,
    first: Option<&LayerEntry>,
    at: MotionAt,
) -> Result<PrinterTuning, FormatError> {
    reads.seek_to(at.light_off)?;
    let bottom_light_off = reads.f32_le()?;
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

    let mut second_stage = [0.0; 6];
    reads.seek_to(at.second_stage)?;
    for value in &mut second_stage {
        *value = reads.f32_le()?;
    }

    reads.seek_to(TRANSITION)?;
    let transition_layers = Some(reads.u16_le()?);
    reads.seek_to(V4_MOTION)?;
    let bottom_retract_speed = reads.f32_le()?;
    let stage_bottom_retract_speed = reads.f32_le()?;
    reads.seek_to(V4_MOTION + 0x24)?;
    let stage_bottom_retract = reads.f32_le()?;
    let waits = match first {
        Some(entry) if entry.offset >= RESTS_BEFORE_DATA => {
            reads.seek_to(entry.offset - RESTS_BEFORE_DATA)?;
            let before_lift_s = reads.f32_le()?;
            let after_lift_s = reads.f32_le()?;
            stated_rests(before_lift_s, after_lift_s, reads.f32_le()?)
        }
        _ => None,
    };

    let peel = ChituPeel {
        bottom_lift_total_mm: bottom_lift,
        lift_total_mm: lift,
        second_stage,
        bottom_retract: [stage_bottom_retract, stage_bottom_retract_speed],
    };
    Ok(PrinterTuning {
        bottom_lift_speed_mm_min: stated_speed_mm_min(bottom_lift_speed),
        lift_speed_mm_min: stated_speed_mm_min(lift_speed),
        retract_speed_mm_min: stated_speed_mm_min(retract_speed),
        bottom_retract_speed_mm_min: stated_speed_mm_min(bottom_retract_speed),
        light_off_delay_s: stated_wait_s(light_off),
        bottom_light_off_delay_s: stated_wait_s(bottom_light_off),
        transition_layers,
        waits,
        ..peel.stated()
    })
}

fn read_table<S: ReadSeek>(
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

/// An `.fdg` or `.phz` file with its table read and its masks still in it.
pub struct OpenFdg<S> {
    reads: Reads<S>,
    facts: SlicedFile,
    /// Key the layer bytes are covered with, or zero when they are in the clear.
    key: u32,
    pixel_count: u32,
    crypt: Crypt,
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
        (self.crypt)(self.key, index, &mut data);
        rle::decode(&data, self.pixel_count)
    }
}
