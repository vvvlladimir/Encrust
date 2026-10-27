use core_format::{
    FormatError, LayerEntry, OpenFile, PrinterTuning, ReadSeek, Reads, SlicedFile,
    SlicedFileReader, layer_in_range, panel_in_range, stated_distance_mm, stated_pwm,
    stated_speed_mm_min, stated_wait_s,
};
use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, Run, ScanlineRasterizer, Shading};
use core_slicer::Layer;

use super::blocks::{MACHINE_BLOCK_BYTES, MAGIC, VERSION};
use super::rings::{self, POINT_BYTES};
use crate::v3::blocks::{PAGE_BREAK, PREVIEW_SIZES_PX};
use crate::v3::reader::{read_text, read_utf16_number};

/// Reads the `.cxline`. Layout is in `docs/formats/cxline.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CxlineReader;

/// The header's fields a reader keeps, in the order it holds them.
struct Header {
    machine: Option<String>,
    layer_count: u32,
    width_px: u32,
    height_px: u32,
    exposure_s: f32,
    volume_ml: f32,
}

impl SlicedFileReader for CxlineReader {
    type Open<S: ReadSeek> = OpenCxline<S>;

    fn extension(&self) -> &'static str {
        "cxline"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);
        let end = reads.length()?;
        let header = read_header(&mut reads)?;
        panel_in_range(header.width_px, header.height_px)?;
        for (width, height) in PREVIEW_SIZES_PX {
            reads.skip(u64::from(width * height * 2) + PAGE_BREAK.len() as u64)?;
        }

        let width_mm = read_utf16_number(&mut reads)?;
        let height_mm = read_utf16_number(&mut reads)?;
        let layer_height_mm = read_utf16_number(&mut reads)?.unwrap_or_default();
        let mut settings = [0u16; 11];
        for value in &mut settings {
            *value = reads.u16_be()?;
        }
        let [_, light_off, bottom_exposure, bottom_layers, ..] = settings.map(f32::from);
        let bottom_layers = bottom_layers as u32;
        reads.skip(u64::from(header.layer_count) * 4 + PAGE_BREAK.len() as u64)?;
        reads.skip(8 + PAGE_BREAK.len() as u64)?;

        let layers = walk_layers(&mut reads, header.layer_count, end)?
            .into_iter()
            .zip(0u32..)
            .map(|((offset, size), index)| LayerEntry {
                z_mm: layer_height_mm * (index + 1) as f32,
                exposure_s: if index < bottom_layers {
                    bottom_exposure
                } else {
                    header.exposure_s
                },
                offset,
                size,
            })
            .collect();

        let display_mm = width_mm.zip(height_mm);
        let panel = panel_of(&header, display_mm)?;
        Ok(OpenCxline {
            reads,
            panel,
            facts: SlicedFile {
                format: "cxline",
                version: Some(u32::from(VERSION)),
                machine: header.machine,
                slicer: None,
                resin: None,
                width_px: header.width_px,
                height_px: header.height_px,
                display_mm,
                layer_height_mm,
                exposure_s: header.exposure_s,
                bottom_exposure_s: bottom_exposure,
                bottom_layers,
                print_time_s: None,
                volume_mm3: Some(header.volume_ml * 1000.0),
                grey_steps: 2,
                stated: stated(settings, light_off),
                layers,
            },
        })
    }
}

fn read_header<S: ReadSeek>(reads: &mut Reads<S>) -> Result<Header, FormatError> {
    let magic_bytes = reads.u32_be()?;
    let magic = if magic_bytes as usize == MAGIC.len() {
        reads.bytes(MAGIC.len())?
    } else {
        Vec::new()
    };
    if magic != MAGIC {
        return Err(FormatError::NotThisFormat {
            format: "cxline",
            found: magic_bytes,
        });
    }
    let version = reads.u16_be()?;
    if version != VERSION {
        return Err(FormatError::UnsupportedVersion {
            format: "cxline",
            version: u32::from(version),
        });
    }
    let machine = read_text(reads)?;
    let layer_count = u32::from(reads.u16_be()?);
    let width_px = u32::from(reads.u16_be()?);
    let height_px = u32::from(reads.u16_be()?);

    reads.skip(0x10)?;
    let exposure_s = f32::from(reads.u16_be()?) / 10.0;
    let volume_ml = reads.f32_be()?;
    reads.skip(MACHINE_BLOCK_BYTES as u64 - 0x16)?;
    Ok(Header {
        machine,
        layer_count,
        width_px,
        height_px,
        exposure_s,
        volume_ml,
    })
}

/// Where each layer's area, rings and points stand and how long they are: nothing points at
/// a layer, so finding the last means walking every ring of every one before it.
fn walk_layers<S: ReadSeek>(
    reads: &mut Reads<S>,
    count: u32,
    end: u64,
) -> Result<Vec<(u64, u32)>, FormatError> {
    let mut layers = Vec::with_capacity(reads.claim("layers", u64::from(count), 8)?);
    for _ in 0..count {
        let offset = reads.position()?;
        reads.skip(4)?;
        for _ in 0..reads.u32_be()? {
            let points = reads.u32_be()?;
            reads.skip(u64::from(points) * POINT_BYTES)?;
        }
        let size = reads.position()? - offset;
        if offset + size > end {
            return Err(FormatError::AddressPastEnd { offset, end });
        }
        layers.push((offset, size as u32));
        reads.skip(PAGE_BREAK.len() as u64)?;
    }
    Ok(layers)
}

/// The motion and light the settings block states; its speeds are millimetres a second.
fn stated(settings: [u16; 11], light_off_s: f32) -> PrinterTuning {
    let [
        _,
        _,
        _,
        _,
        bottom_lift,
        bottom_lift_speed,
        lift,
        lift_speed,
        retract_speed,
        bottom_pwm,
        pwm,
    ] = settings.map(f32::from);
    PrinterTuning {
        bottom_lift_distance_mm: stated_distance_mm(bottom_lift),
        bottom_lift_speed_mm_min: stated_speed_mm_min(bottom_lift_speed * 60.0),
        lift_distance_mm: stated_distance_mm(lift),
        retract_distance_mm: stated_distance_mm(lift),
        lift_speed_mm_min: stated_speed_mm_min(lift_speed * 60.0),
        retract_speed_mm_min: stated_speed_mm_min(retract_speed * 60.0),
        light_off_delay_s: stated_wait_s(light_off_s),
        light_pwm: stated_pwm(pwm as u16),
        bottom_light_pwm: stated_pwm(bottom_pwm as u16),
        ..PrinterTuning::default()
    }
}

/// The panel the rings are filled on, which only the panel's millimetres put a pitch to.
fn panel_of(
    header: &Header,
    display_mm: Option<(f32, f32)>,
) -> Result<RasterSettings, FormatError> {
    let (width_mm, height_mm) = display_mm
        .filter(|&(width, height)| width > 0.0 && height > 0.0)
        .ok_or_else(|| FormatError::Missing {
            what: "the panel size in millimetres".to_owned(),
        })?;
    Ok(RasterSettings {
        width_px: header.width_px,
        height_px: header.height_px,
        pitch: PixelPitch {
            x: width_mm / header.width_px as f32,
            y: height_mm / header.height_px as f32,
        },
        mirror_x: false,
        mirror_y: false,
        // The rings are already what the panel is to light, so they are filled as they
        // stand rather than shaded by how much of a pixel they cover.
        shading: Shading::Binary,
        grey: Grey::default(),
        blur_px: 0,
    })
}

/// A `.cxline` with its layers found and their rings still in it.
pub struct OpenCxline<S> {
    reads: Reads<S>,
    panel: RasterSettings,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenCxline<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let data = self.reads.bytes(entry.size as usize)?;
        let rastered = ScanlineRasterizer
            .rasterize(&Layer::new(0.0, rings::contours_of(&data)?), &self.panel)
            .map_err(|source| FormatError::Encoding {
                what: "a layer",
                reason: source.to_string(),
            })?;
        Ok(rastered.runs.runs().to_vec())
    }
}
