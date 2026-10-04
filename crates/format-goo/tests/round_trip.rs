//! Writes a `.goo` file and takes it apart again, byte by byte, through the public API.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_format::{
    ExposurePlan, ExposureRange, LayerPlan, LayerSink, PrintJob, SlicedFileWriter, Thumbnail,
};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_goo::{GooSink, GooWriter, checksum, decode};
use printer_profiles::{MaterialProfile, PrinterProfile};
use proptest::prelude::*;

const WIDTH_PX: u32 = 16;
const HEIGHT_PX: u32 = 8;

/// Header size the format fixes, repeated here so the test fails if the writer drifts.
const HEADER_SIZE: usize = 0x0002_FB95;

/// Bytes of a layer before its data size field, then the magic byte and the checksum.
const LAYER_DEFINITION_SIZE: usize = 2 + 15 * 4 + 2 + 2;

const ENDING_SIZE: usize = 11;

fn printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(
        r#"
name = "Test panel"
manufacturer = "Test"
mirror_x = true

[display]
width_px = 16
height_px = 8
width_mm = 1.6
height_mm = 0.8

[build_volume]
x = 1.6
y = 0.8
z = 10.0
"#,
        std::path::Path::new("inline.toml"),
    )
    .expect("the inline profile is valid")
}

fn job(layer_count: u32) -> PrintJob {
    PrintJob {
        printer: printer(),
        material: MaterialProfile::default(),
        raster: RasterSettings {
            width_px: WIDTH_PX,
            height_px: HEIGHT_PX,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            mirror_x: true,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        },
        plan: LayerPlan::of_count(
            MaterialProfile::default().layer_height_mm,
            layer_count as usize,
        ),
        volume_mm3: 1234.0,
        exposure: ExposurePlan::default(),
        thumbnail: None,
        created_unix_s: 0,
    }
}

fn layer_from(pixels: &[u8]) -> LayerRuns {
    let mut mask = LayerMask::new(WIDTH_PX, HEIGHT_PX);
    mask.pixels_mut().copy_from_slice(pixels);
    LayerRuns::from_mask(&mask)
}

fn blank() -> LayerRuns {
    LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()
}

fn write(layers: &[LayerRuns]) -> Vec<u8> {
    write_job(&job(layers.len() as u32), layers)
}

fn write_job(job: &PrintJob, layers: &[LayerRuns]) -> Vec<u8> {
    write_closing_with(job, layers, job.volume_mm3)
}

fn write_closing_with(job: &PrintJob, layers: &[LayerRuns], volume_mm3: f32) -> Vec<u8> {
    let mut file = std::io::Cursor::new(Vec::new());
    let mut sink = GooWriter
        .begin(job, &mut file)
        .expect("the job matches the panel");
    for layer in layers {
        sink.push(GooSink::encode(layer))
            .expect("a layer is written");
    }
    sink.finish(volume_mm3)
        .expect("every promised layer arrived");
    file.into_inner()
}

/// Where the totals sit, from docs/formats/goo.md: the volume follows the print time.
const VOLUME_FIELD: usize = 195_450;

#[test]
fn the_volume_handed_to_finish_is_the_one_in_the_header() {
    // A caller that only learns the volume by slicing opens the file with zero; see
    // docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md
    let mut opened = job(1);
    opened.volume_mm3 = 0.0;
    let file = write_closing_with(&opened, &[blank()], 4321.0);

    let written = f32::from_be_bytes([
        file[VOLUME_FIELD],
        file[VOLUME_FIELD + 1],
        file[VOLUME_FIELD + 2],
        file[VOLUME_FIELD + 3],
    ]);
    assert!(
        (written - 4321.0).abs() < 1e-3,
        "the header states {written} mm^3, not the 4321 the stack came to"
    );
    assert_eq!(read_layers(&file).len(), 1, "the layers still read back");
}

/// The two fixed preview offsets, and the side of each square, from docs/formats/goo.md
const SMALL_PREVIEW: usize = 194;
const BIG_PREVIEW: usize = 27_108;

/// One preview pixel as RGB565, big-endian like the rest of the container.
fn preview_pixel(file: &[u8], preview: usize, index: usize) -> [u8; 3] {
    let word = u16::from_be_bytes([file[preview + index * 2], file[preview + index * 2 + 1]]);
    [
        ((word >> 11) as u8) << 3,
        (((word >> 5) & 0x3F) as u8) << 2,
        ((word & 0x1F) as u8) << 3,
    ]
}

/// The pixels of every layer, recovered from the container and its run-length data.
fn read_layers(file: &[u8]) -> Vec<Vec<u8>> {
    let layer_count =
        u32::from_be_bytes([file[195_310], file[195_311], file[195_312], file[195_313]]) as usize;

    let mut layers = Vec::with_capacity(layer_count);
    let mut cursor = HEADER_SIZE;
    for index in 0..layer_count {
        assert_eq!(
            &file[cursor + LAYER_DEFINITION_SIZE - 2..cursor + LAYER_DEFINITION_SIZE],
            [0x0D, 0x0A],
            "layer {index} definition must end with the delimiter"
        );
        cursor += LAYER_DEFINITION_SIZE;

        let size = u32::from_be_bytes([
            file[cursor],
            file[cursor + 1],
            file[cursor + 2],
            file[cursor + 3],
        ]) as usize;
        cursor += 4;

        assert_eq!(file[cursor], 0x55, "layer {index} must open with the magic");
        let data = &file[cursor + 1..cursor + size - 1];
        assert_eq!(
            file[cursor + size - 1],
            checksum(data),
            "layer {index} checksum must be the negated byte sum"
        );
        cursor += size;

        assert_eq!(&file[cursor..cursor + 2], [0x0D, 0x0A]);
        cursor += 2;

        layers.push(
            decode(data)
                .expect("our own run-length data decodes")
                .iter()
                .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
                .collect(),
        );
    }

    assert_eq!(
        cursor + ENDING_SIZE,
        file.len(),
        "nothing may follow the ending string"
    );
    assert_eq!(
        &file[cursor..],
        [
            0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00
        ]
    );
    layers
}

#[test]
fn a_written_file_opens_with_the_version_and_the_magic_tag() {
    let file = write(&[blank()]);
    assert_eq!(&file[..4], b"V3.0");
    assert_eq!(
        &file[4..12],
        [0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00]
    );
}

#[test]
fn layer_content_starts_where_the_header_says_it_does() {
    let file = write(&[blank()]);
    let offset = HEADER_SIZE - 7;
    let recorded = u32::from_be_bytes([
        file[offset],
        file[offset + 1],
        file[offset + 2],
        file[offset + 3],
    ]) as usize;
    assert_eq!(recorded, HEADER_SIZE);
}

#[test]
fn every_pixel_of_every_layer_comes_back_unchanged() {
    let pixel_count = (WIDTH_PX * HEIGHT_PX) as usize;
    let layers: Vec<LayerRuns> = (0..3u32)
        .map(|layer| {
            let pixels: Vec<u8> = (0..pixel_count)
                .map(|i| ((i as u32 * 13 + layer * 37) % 256) as u8)
                .collect();
            layer_from(&pixels)
        })
        .collect();

    let recovered = read_layers(&write(&layers));
    assert_eq!(recovered.len(), 3);
    for (index, (got, want)) in recovered.iter().zip(&layers).enumerate() {
        assert_eq!(
            got,
            want.to_mask().pixels(),
            "layer {index} changed on the way out"
        );
    }
}

#[test]
fn a_blank_stack_still_defines_every_pixel() {
    // An all-black layer is the case where an encoder is most likely to emit nothing at
    // all, which the printer decodes into whatever was left in its buffer.
    let recovered = read_layers(&write(&[blank()]));
    assert_eq!(recovered[0].len(), (WIDTH_PX * HEIGHT_PX) as usize);
    assert!(recovered[0].iter().all(|&pixel| pixel == 0));
}

proptest! {
    #[test]
    fn any_mask_survives_the_container(
        pixels in proptest::collection::vec(any::<u8>(), (WIDTH_PX * HEIGHT_PX) as usize)
    ) {
        let recovered = read_layers(&write(&[layer_from(&pixels)]));
        prop_assert_eq!(&recovered[0], &pixels);
    }
}

#[test]
fn both_previews_carry_the_job_thumbnail() {
    let red = Thumbnail::filled(8, 8, [255, 0, 0]);
    let job = PrintJob {
        thumbnail: Some(red),
        ..job(1)
    };
    let file = write_job(&job, &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()]);

    // Both records are square and so is the thumbnail, so nothing is padded: every pixel
    // is the same red, 0xF800 once five bits of it survive.
    assert_eq!(preview_pixel(&file, SMALL_PREVIEW, 0), [248, 0, 0]);
    assert_eq!(
        preview_pixel(&file, SMALL_PREVIEW, 116 * 116 - 1),
        [248, 0, 0]
    );
    assert_eq!(
        preview_pixel(&file, BIG_PREVIEW, 290 * 290 / 2),
        [248, 0, 0]
    );
}

#[test]
fn a_job_without_a_thumbnail_still_writes_both_previews_black() {
    let file = write(&[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()]);

    assert!(
        file[SMALL_PREVIEW..SMALL_PREVIEW + 2 * 116 * 116]
            .iter()
            .all(|byte| *byte == 0),
        "the record is there at its fixed size whatever it holds"
    );
    assert_eq!(preview_pixel(&file, BIG_PREVIEW, 0), [0, 0, 0]);
}

/// Where the advance-mode flag sits, from docs/formats/goo.md.
const ADVANCE_MODE: usize = 195_445;

/// The exposure of each layer, read out of the layer definitions in print order.
fn layer_exposures(file: &[u8], layer_count: usize) -> Vec<f32> {
    let mut cursor = HEADER_SIZE;
    let mut exposures = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
        // The exposure follows the pause position and the layer position; see
        // docs/formats/goo.md.
        let at = cursor + 10;
        exposures.push(f32::from_be_bytes([
            file[at],
            file[at + 1],
            file[at + 2],
            file[at + 3],
        ]));
        cursor += LAYER_DEFINITION_SIZE;
        let size = u32::from_be_bytes([
            file[cursor],
            file[cursor + 1],
            file[cursor + 2],
            file[cursor + 3],
        ]) as usize;
        cursor += 4 + size + 2;
    }
    exposures
}

#[test]
fn a_band_of_height_writes_its_own_exposure_on_the_layers_it_covers() {
    let banded = PrintJob {
        material: MaterialProfile {
            layer_height_mm: 1.0,
            bottom_layers: 1,
            bottom_exposure_s: 30.0,
            exposure_s: 2.0,
            ..MaterialProfile::default()
        },
        // Layer tops run 1, 2, 3, 4 mm; the band takes the second and third.
        plan: LayerPlan::of_count(1.0, 4),
        exposure: ExposurePlan::new(vec![ExposureRange::new(2.0, 4.0, 9.0)]),
        ..job(4)
    };
    let file = write_job(&banded, &[blank(), blank(), blank(), blank()]);

    let exposures = layer_exposures(&file, 4);
    assert_eq!(exposures, vec![30.0, 9.0, 9.0, 2.0]);
    assert_eq!(
        file[ADVANCE_MODE], 1,
        "a stack whose exposure varies has to be read per layer"
    );
}

#[test]
fn a_stack_on_the_resins_own_exposure_is_not_read_per_layer() {
    let file = write(&[blank(), blank()]);
    assert_eq!(file[ADVANCE_MODE], 0);
}
