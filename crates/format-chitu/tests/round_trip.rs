//! Writes a Chitu file and walks it the way a reader would: every offset in the header
//! must land on the block it names, and every layer must decode to the pixels it went in
//! as. Both containers, because they share the front of the file.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_format::{
    ExposurePlan, ExposureRange, LayerPlan, LayerSink, PrintJob, SlicedFileWriter, Thumbnail,
};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_chitu::{
    CbddlpSink, CbddlpWriter, CtbSink, CtbVersion, CtbWriter, GREY_PASSES, decode_passes,
};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 16;
const HEIGHT_PX: u32 = 8;

const HEADER_BYTES: usize = 112;
const LAYER_DEF_BYTES: usize = 36;
const LAYER_DEF_EX_BYTES: u32 = 84;

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

fn write(version: CtbVersion, layers: &[LayerRuns]) -> Vec<u8> {
    write_job(version, &job(layers.len() as u32), layers)
}

fn write_job(version: CtbVersion, job: &PrintJob, layers: &[LayerRuns]) -> Vec<u8> {
    write_closing_with(version, job, layers, job.volume_mm3)
}

fn write_closing_with(
    version: CtbVersion,
    job: &PrintJob,
    layers: &[LayerRuns],
    volume_mm3: f32,
) -> Vec<u8> {
    let mut file = std::io::Cursor::new(Vec::new());
    {
        let mut sink = CtbWriter::new(version)
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(CtbSink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

#[test]
fn the_volume_handed_to_finish_is_the_one_in_the_header() {
    // A caller that only learns the volume by slicing opens the file with zero; see
    // docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md. The file states it
    // in millilitres.
    let mut opened = job(1);
    opened.volume_mm3 = 0.0;
    let file = write_closing_with(
        CtbVersion::V4,
        &opened,
        &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()],
        4321.0,
    );

    let at = file
        .windows(4)
        .position(|word| word == 4.321f32.to_le_bytes())
        .expect("the volume the stack came to is in the header");
    assert!(at < 1024, "the volume sits in the header, not at {at}");
}

fn u32_at(file: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        file[offset],
        file[offset + 1],
        file[offset + 2],
        file[offset + 3],
    ])
}

/// The header fields that are offsets, by their byte position in it.
const LARGE_PREVIEW: usize = 0x3C;
const LAYER_TABLE: usize = 0x40;
const LAYER_COUNT: usize = 0x44;
const SMALL_PREVIEW: usize = 0x48;
const PRINT_PARAMETERS: usize = 0x54;
const PRINT_PARAMETERS_SIZE: usize = 0x58;
const SLICER_OFFSET: usize = 0x68;
const SLICER_SIZE: usize = 0x6C;

#[test]
fn every_offset_in_the_header_lands_inside_the_file_in_order() {
    let file = write(
        CtbVersion::V4,
        &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish(); 1],
    );

    let large = u32_at(&file, LARGE_PREVIEW) as usize;
    let small = u32_at(&file, SMALL_PREVIEW) as usize;
    let parameters = u32_at(&file, PRINT_PARAMETERS) as usize;
    let slicer = u32_at(&file, SLICER_OFFSET) as usize;
    let table = u32_at(&file, LAYER_TABLE) as usize;

    assert_eq!(large, HEADER_BYTES, "the large preview follows the header");
    assert!(
        large < small && small < parameters && parameters < slicer && slicer < table,
        "the blocks are written in the order the header lists them"
    );
    assert!(table < file.len(), "the layer table is inside the file");
    assert_eq!(u32_at(&file, PRINT_PARAMETERS_SIZE), 60);
    assert_eq!(u32_at(&file, SLICER_SIZE), 76);

    // A preview record says where its own pixels start, right behind its 32-byte record.
    assert_eq!(u32_at(&file, large + 8) as usize, large + 32);
    assert_eq!(u32_at(&file, small + 8) as usize, small + 32);
}

#[test]
fn each_table_row_points_at_its_own_layer_data() {
    let layers: Vec<_> = (0..3)
        .map(|index| layer_from(&vec![index * 40; (WIDTH_PX * HEIGHT_PX) as usize]))
        .collect();
    let file = write(CtbVersion::V4, &layers);

    let table = u32_at(&file, LAYER_TABLE) as usize;
    assert_eq!(u32_at(&file, LAYER_COUNT), 3);

    for index in 0..3 {
        let row = table + index * LAYER_DEF_BYTES;
        let address = u32_at(&file, row + 12) as usize;
        let size = u32_at(&file, row + 16) as usize;

        assert_eq!(
            u32_at(&file, row + 24),
            LAYER_DEF_EX_BYTES,
            "layer {index} must declare the extended block in front of its data"
        );
        assert!(
            address + size <= file.len(),
            "layer {index} data is inside the file"
        );

        // The extended block in front of the data repeats the row it belongs to.
        let extended = address - LAYER_DEF_EX_BYTES as usize;
        assert_eq!(
            &file[extended..extended + LAYER_DEF_BYTES],
            &file[row..row + LAYER_DEF_BYTES],
            "layer {index} extended block must repeat its table row"
        );
    }
}

#[test]
fn a_layer_decodes_to_the_pixels_it_went_in_as() {
    let pixels: Vec<u8> = (0..WIDTH_PX * HEIGHT_PX).map(|i| (i * 2) as u8).collect();
    let file = write(CtbVersion::V4, &[layer_from(&pixels)]);

    let table = u32_at(&file, LAYER_TABLE) as usize;
    let address = u32_at(&file, table + 12) as usize;
    let size = u32_at(&file, table + 16) as usize;

    let runs = core_format::decode_rle7(&file[address..address + size])
        .expect("our own encoder is well formed");
    let decoded: Vec<u8> = runs
        .iter()
        .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
        .collect();

    assert_eq!(decoded.len(), pixels.len(), "every pixel comes back");
    for (at, (&out, &back)) in pixels.iter().zip(decoded.iter()).enumerate() {
        // The format carries seven bits of grey, so a value survives to within one step.
        assert!(
            out.abs_diff(back) <= 1,
            "pixel {at} went in as {out} and came back as {back}"
        );
    }
}

#[test]
fn version_five_adds_the_resin_block_behind_the_version_four_one() {
    let layer = [LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()];
    let four = write(CtbVersion::V4, &layer);
    let five = write(CtbVersion::V5, &layer);

    assert!(
        five.len() > four.len(),
        "version 5 carries a resin block version 4 does not"
    );
    assert_eq!(
        u32_at(&four, LARGE_PREVIEW),
        u32_at(&five, LARGE_PREVIEW),
        "everything in front of the version 4 block is laid out the same way"
    );
}

/// One preview record decoded back into pixels: the run-length RGB15 of
/// docs/formats/chitu.md.
fn preview_pixels(file: &[u8], record: usize) -> Vec<[u8; 3]> {
    let address = u32_at(file, record + 8) as usize;
    let length = u32_at(file, record + 12) as usize;
    let data = &file[address..address + length];

    let mut pixels = Vec::new();
    let mut cursor = 0;
    while cursor + 1 < data.len() {
        let word = u16::from_le_bytes([data[cursor], data[cursor + 1]]);
        cursor += 2;
        let colour = word & !0x0020;
        let pixel = [
            ((colour >> 11) as u8) << 3,
            (((colour >> 5) & 0x3F) as u8) << 2,
            ((colour & 0x1F) as u8) << 3,
        ];

        let mut run = 1;
        if word & 0x0020 != 0 {
            let count = u16::from_le_bytes([data[cursor], data[cursor + 1]]);
            cursor += 2;
            run = u32::from(count & 0x0FFF) + 1;
        }
        pixels.extend(std::iter::repeat_n(pixel, run as usize));
    }
    pixels
}

#[test]
fn both_previews_carry_the_job_thumbnail() {
    let job = PrintJob {
        thumbnail: Some(Thumbnail::filled(8, 8, [255, 0, 0])),
        ..job(1)
    };
    let file = write_job(
        CtbVersion::V4,
        &job,
        &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()],
    );

    let large = u32_at(&file, LARGE_PREVIEW) as usize;
    let pixels = preview_pixels(&file, large);
    assert_eq!(pixels.len(), 400 * 300, "the record holds every pixel");

    // A square thumbnail in a 400 by 300 record is a 300 by 300 block with fifty columns
    // of padding either side, so the middle is the model and the corner is not.
    assert_eq!(
        pixels[150 * 400 + 200],
        [248, 0, 0],
        "the middle is the part"
    );
    assert_eq!(pixels[0], [0, 0, 0], "the padding stays background");

    let small = u32_at(&file, SMALL_PREVIEW) as usize;
    let pixels = preview_pixels(&file, small);
    assert_eq!(pixels.len(), 200 * 125);
    assert_eq!(pixels[62 * 200 + 100], [248, 0, 0]);
}

#[test]
fn a_job_without_a_thumbnail_still_writes_both_previews_black() {
    let file = write(
        CtbVersion::V4,
        &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()],
    );

    let large = u32_at(&file, LARGE_PREVIEW) as usize;
    let pixels = preview_pixels(&file, large);
    assert_eq!(pixels.len(), 400 * 300);
    assert!(pixels.iter().all(|pixel| *pixel == [0, 0, 0]));
}

#[test]
fn a_band_of_height_writes_its_own_exposure_into_the_layer_table() {
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
    let file = write_job(
        CtbVersion::V4,
        &banded,
        &vec![layer_from(&vec![255; (WIDTH_PX * HEIGHT_PX) as usize]); 4],
    );

    let table = u32_at(&file, LAYER_TABLE) as usize;
    let exposures: Vec<f32> = (0..4)
        .map(|index| {
            // The exposure is the second field of a row, behind the layer's Z.
            let at = table + index * LAYER_DEF_BYTES + 4;
            f32::from_le_bytes([file[at], file[at + 1], file[at + 2], file[at + 3]])
        })
        .collect();

    assert_eq!(exposures, vec![30.0, 9.0, 9.0, 2.0]);
}

fn write_cbddlp(job: &PrintJob, layers: &[LayerRuns]) -> Vec<u8> {
    let mut file = std::io::Cursor::new(Vec::new());
    {
        let mut sink = CbddlpWriter::default()
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(CbddlpSink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(job.volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

#[test]
fn the_older_container_stops_at_the_print_parameters() {
    let file = write_cbddlp(&job(1), &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()]);

    let large = u32_at(&file, LARGE_PREVIEW) as usize;
    let small = u32_at(&file, SMALL_PREVIEW) as usize;
    let parameters = u32_at(&file, PRINT_PARAMETERS) as usize;
    let table = u32_at(&file, LAYER_TABLE) as usize;

    assert_eq!(large, HEADER_BYTES, "the large preview follows the header");
    assert!(large < small && small < parameters && parameters < table);
    assert_eq!(u32_at(&file, PRINT_PARAMETERS_SIZE), 60);
    assert_eq!(
        (u32_at(&file, SLICER_OFFSET), u32_at(&file, SLICER_SIZE)),
        (0, 0),
        "there is no slicer info block to point at"
    );
    assert_eq!(
        parameters + 60,
        table,
        "the layer table follows the print parameters directly"
    );
}

#[test]
fn a_layer_of_the_older_container_decodes_to_the_greys_its_passes_carry() {
    let pixels: Vec<u8> = (0..WIDTH_PX * HEIGHT_PX).map(|i| (i * 2) as u8).collect();
    let file = write_cbddlp(&job(1), &[layer_from(&pixels)]);

    let table = u32_at(&file, LAYER_TABLE) as usize;
    let passes: Vec<Vec<u8>> = (0..GREY_PASSES as usize)
        .map(|pass| {
            // Rows run pass-major, and this layer is the only one, so the pass is the row.
            let row = table + pass * LAYER_DEF_BYTES;
            assert_eq!(
                u32_at(&file, row + 24),
                LAYER_DEF_BYTES as u32,
                "no extended block follows a row of the older container"
            );
            let address = u32_at(&file, row + 12) as usize;
            let size = u32_at(&file, row + 16) as usize;
            file[address..address + size].to_vec()
        })
        .collect();

    let runs =
        decode_passes(&passes, WIDTH_PX * HEIGHT_PX).expect("our own passes are well formed");
    let decoded: Vec<u8> = runs
        .iter()
        .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
        .collect();

    assert_eq!(decoded.len(), pixels.len(), "every pixel comes back");
    for (at, (&out, &back)) in pixels.iter().zip(decoded.iter()).enumerate() {
        // Eight passes give nine steps 32 apart, and a value lands on the step below it.
        assert!(
            back <= out && out - back < 32,
            "pixel {at} went in as {out} and came back as {back}"
        );
    }
}
