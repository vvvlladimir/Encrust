use core_format::{
    FormatError, LayerEntry, OpenFile, PrinterTuning, ReadSeek, Reads, SlicedFile,
    SlicedFileReader, layer_in_range, panel_in_range, stated_distance_mm, stated_speed_mm_min,
    stated_wait_s,
};
use core_raster::Run;

use super::codec::{self, RECORD_BYTES};
use super::header::{HEADER_BYTES, SUBTAG, TAG};

/// Reads the `.photons` container.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhotonsReader;

/// The header's numbers, in the order it holds them.
struct Header {
    pixel_mm: f64,
    layer_height_mm: f64,
    exposure_s: f64,
    light_off_s: f64,
    bottom_exposure_s: f64,
    bottom_layers: u32,
    lift_mm: f64,
    lift_mm_s: f64,
    retract_mm_s: f64,
    volume_ml: f64,
    preview_px: (u32, u32),
}

impl SlicedFileReader for PhotonsReader {
    type Open<S: ReadSeek> = OpenPhotons<S>;

    fn extension(&self) -> &'static str {
        "photons"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);
        let tag = reads.u32_be()?;
        if tag != TAG || reads.u16_be()? != SUBTAG {
            return Err(FormatError::NotThisFormat {
                format: "photons",
                found: tag,
            });
        }
        let header = read_header(&mut reads)?;

        let (preview_width, preview_height) = header.preview_px;
        let preview_bytes =
            (u64::from(preview_width) * u64::from(preview_height)).saturating_mul(2);
        reads.seek_to(HEADER_BYTES.saturating_add(preview_bytes))?;
        let layer_count = reads.u32_be()?;
        let (layers, (width_px, height_px)) = read_layers(&mut reads, layer_count, &header)?;
        let pixel_count = panel_in_range(width_px, height_px)?;

        let pitch_mm = header.pixel_mm as f32;
        Ok(OpenPhotons {
            reads,
            facts: SlicedFile {
                format: "photons",
                version: Some(TAG),
                machine: None,
                slicer: None,
                resin: None,
                width_px,
                height_px,
                display_mm: (pitch_mm > 0.0)
                    .then_some((pitch_mm * width_px as f32, pitch_mm * height_px as f32)),
                layer_height_mm: header.layer_height_mm as f32,
                exposure_s: header.exposure_s as f32,
                bottom_exposure_s: header.bottom_exposure_s as f32,
                bottom_layers: header.bottom_layers,
                print_time_s: None,
                volume_mm3: Some(header.volume_ml as f32 * 1000.0),
                grey_steps: 2,
                stated: stated(&header),
                layers,
            },
            pixel_count,
        })
    }
}

/// The peel the header states: one lift, retracted the same distance.
fn stated(header: &Header) -> PrinterTuning {
    PrinterTuning {
        lift_distance_mm: stated_distance_mm(header.lift_mm as f32),
        retract_distance_mm: stated_distance_mm(header.lift_mm as f32),
        lift_speed_mm_min: stated_speed_mm_min(header.lift_mm_s as f32 * 60.0),
        retract_speed_mm_min: stated_speed_mm_min(header.retract_mm_s as f32 * 60.0),
        light_off_delay_s: stated_wait_s(header.light_off_s as f32),
        ..PrinterTuning::default()
    }
}

fn read_header<S: ReadSeek>(reads: &mut Reads<S>) -> Result<Header, FormatError> {
    let pixel_mm = reads.f64_be()?;
    let layer_height_mm = reads.f64_be()?;
    let exposure_s = reads.f64_be()?;
    let light_off_s = reads.f64_be()?;
    let bottom_exposure_s = reads.f64_be()?;
    let bottom_layers = reads.u32_be()?;
    let lift_mm = reads.f64_be()?;
    let lift_mm_s = reads.f64_be()?;
    let retract_mm_s = reads.f64_be()?;
    let volume_ml = reads.f64_be()?;
    let preview_width = reads.u32_be()?;
    reads.skip(4)?;
    let preview_height = reads.u32_be()?;
    Ok(Header {
        pixel_mm,
        layer_height_mm,
        exposure_s,
        light_off_s,
        bottom_exposure_s,
        bottom_layers,
        lift_mm,
        lift_mm_s,
        retract_mm_s,
        volume_ml,
        preview_px: (preview_width, preview_height),
    })
}

/// Walks the records, which sit back to back with nothing pointing at them, and returns
/// where each layer's data is and the panel the first one states.
fn read_layers<S: ReadSeek>(
    reads: &mut Reads<S>,
    layer_count: u32,
    header: &Header,
) -> Result<(Vec<LayerEntry>, (u32, u32)), FormatError> {
    let rows = u64::from(layer_count);
    let mut entries = Vec::with_capacity(reads.claim("layer records", rows, RECORD_BYTES)?);
    let mut panel = (0, 0);
    for index in 0..layer_count {
        reads.skip(12)?;
        let width = reads.u32_be()?;
        let height = reads.u32_be()?;
        let size = codec::data_bytes(reads.u32_be()?);
        reads.skip(4)?;
        if index == 0 {
            panel = (width, height);
        }
        let offset = reads.position()?;
        let exposure_s = if index < header.bottom_layers {
            header.bottom_exposure_s
        } else {
            header.exposure_s
        };
        entries.push(LayerEntry {
            z_mm: (header.layer_height_mm * f64::from(index + 1)) as f32,
            exposure_s: exposure_s as f32,
            offset,
            size,
        });
        reads.seek_to(offset + u64::from(size))?;
    }
    Ok((entries, panel))
}

/// A `.photons` file with its records found and its masks still in it.
pub struct OpenPhotons<S> {
    reads: Reads<S>,
    facts: SlicedFile,
    pixel_count: u32,
}

impl<S: ReadSeek> OpenFile for OpenPhotons<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let data = self.reads.bytes(entry.size as usize)?;
        codec::decode(&data, self.pixel_count)
    }
}
