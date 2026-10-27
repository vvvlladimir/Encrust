//! Writes an Anycubic file and walks it the way a reader would: the file mark must address
//! every table it claims, each table must name itself, and every layer must decode to the
//! pixels it went in as.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_format::{
    ExposurePlan, ExposureRange, FormatError, LayerPlan, LayerSink, OpenFile, PrintJob,
    SlicedFileReader, SlicedFileWriter, Thumbnail,
};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_anycubic::{
    AnycubicFlavour, AnycubicReader, AnycubicSink, AnycubicVersion, AnycubicWriter, PwsReader,
    decode,
};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 16;
const HEIGHT_PX: u32 = 8;

const LAYER_DEF_BYTES: usize = 32;

/// Where the file mark states each table is, by its byte position in the mark.
const HEADER_ADDRESS: usize = 0x14;
const PREVIEW_ADDRESS: usize = 0x1C;
const LAYER_TABLE_ADDRESS: usize = 0x24;
const COLOUR_ADDRESS: usize = 0x20;
const EXTRA_ADDRESS: usize = 0x28;
const MACHINE_ADDRESS: usize = 0x2C;
const LAYER_DATA_ADDRESS: usize = 0x30;
const MODEL_ADDRESS: usize = 0x34;
const SUB_LAYERS_ADDRESS: usize = 0x38;

fn printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(
        r#"
name = "Test panel"
manufacturer = "Test"

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
            mirror_x: false,
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

fn write(job: &PrintJob, layers: &[LayerRuns]) -> Vec<u8> {
    write_closing_with(job, layers, job.volume_mm3)
}

fn write_closing_with(job: &PrintJob, layers: &[LayerRuns], volume_mm3: f32) -> Vec<u8> {
    write_version(AnycubicWriter::default(), job, layers, volume_mm3)
}

fn write_version(
    writer: AnycubicWriter,
    job: &PrintJob,
    layers: &[LayerRuns],
    volume_mm3: f32,
) -> Vec<u8> {
    let mut file = std::io::Cursor::new(Vec::new());
    {
        let mut sink = writer
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(AnycubicSink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

fn u32_at(file: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        file[offset],
        file[offset + 1],
        file[offset + 2],
        file[offset + 3],
    ])
}

fn f32_at(file: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        file[offset],
        file[offset + 1],
        file[offset + 2],
        file[offset + 3],
    ])
}

/// A table's own name, which a reader checks before trusting the address that led to it.
fn table_name(file: &[u8], at: usize) -> String {
    String::from_utf8_lossy(&file[at..at + 12])
        .trim_end_matches('\0')
        .to_owned()
}

#[test]
fn the_file_mark_addresses_a_table_that_names_itself() {
    let file = write(&job(1), &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()]);

    assert_eq!(table_name(&file, 0), "ANYCUBIC");
    assert_eq!(u32_at(&file, 0x0C), 1, "the default writer is version 1");
    assert_eq!(
        u32_at(&file, 0x10),
        11,
        "eleven tables, whatever the revision"
    );

    let named = [
        (HEADER_ADDRESS, "HEADER"),
        (PREVIEW_ADDRESS, "PREVIEW"),
        (LAYER_TABLE_ADDRESS, "LAYERDEF"),
        (EXTRA_ADDRESS, "EXTRA"),
        (MACHINE_ADDRESS, "MACHINE"),
        (MODEL_ADDRESS, "Model"),
        (SUB_LAYERS_ADDRESS, "SUBIMGS"),
    ];
    for (slot, name) in named {
        assert_eq!(table_name(&file, u32_at(&file, slot) as usize), name);
    }
    let order = [
        HEADER_ADDRESS,
        PREVIEW_ADDRESS,
        COLOUR_ADDRESS,
        LAYER_TABLE_ADDRESS,
        EXTRA_ADDRESS,
        MACHINE_ADDRESS,
        0x18,
        MODEL_ADDRESS,
        SUB_LAYERS_ADDRESS,
        LAYER_DATA_ADDRESS,
    ]
    .map(|slot| u32_at(&file, slot));
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "the blocks are written in this order: {order:?}"
    );
    assert_eq!(u32_at(&file, 0x3C), 0, "no second preview");
}

#[test]
fn a_table_states_its_own_length() {
    let blank: Vec<_> = (0..4)
        .map(|_| LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish())
        .collect();
    let file = write(&job(4), &blank);

    let header = u32_at(&file, HEADER_ADDRESS) as usize;
    let preview = u32_at(&file, PREVIEW_ADDRESS) as usize;
    let table = u32_at(&file, LAYER_TABLE_ADDRESS) as usize;

    assert_eq!(u32_at(&file, header + 12), 96, "the header's fields alone");
    assert_eq!(
        u32_at(&file, preview + 12) as usize + 16,
        u32_at(&file, COLOUR_ADDRESS) as usize - preview,
        "the preview counts its shape and pixels, not its name and length"
    );
    assert_eq!(
        u32_at(&file, table + 12) as usize,
        4 + 4 * LAYER_DEF_BYTES,
        "the layer table counts the layer count and its rows"
    );
    assert_eq!(u32_at(&file, table + 16), 4, "four layers");
    let machine = u32_at(&file, MACHINE_ADDRESS) as usize;
    assert_eq!(u32_at(&file, machine + 12), 224);
    assert_eq!(
        u32_at(&file, 0x18) as usize + 32,
        machine + 224 + 32,
        "the slicer block follows the machine block"
    );
    assert_eq!(u32_at(&file, machine + 224 + 32), 128, "and states 128");
}

#[test]
fn each_row_points_at_its_own_layer_data_back_to_back() {
    let layers: Vec<_> = (0..3)
        .map(|index| layer_from(&vec![index * 40 + 40; (WIDTH_PX * HEIGHT_PX) as usize]))
        .collect();
    let file = write(&job(3), &layers);

    let table = u32_at(&file, LAYER_TABLE_ADDRESS) as usize + 20;
    let mut expected = u32_at(&file, LAYER_DATA_ADDRESS);

    for index in 0..3 {
        let row = table + index * LAYER_DEF_BYTES;
        let address = u32_at(&file, row);
        let size = u32_at(&file, row + 4);

        assert_eq!(
            address, expected,
            "layer {index} follows the one before it with no gap"
        );
        assert!(address as usize + size as usize <= file.len());
        let thickness = f32_at(&file, row + 20);
        assert!(
            (thickness - MaterialProfile::default().layer_height_mm).abs() < 1e-6,
            "a row carries the layer's own thickness, not its height above the plate"
        );
        expected = address + size;
    }
    assert_eq!(
        expected as usize,
        file.len(),
        "the last layer ends the file"
    );
}

#[test]
fn a_layer_decodes_to_the_pixels_it_went_in_as() {
    let pixels: Vec<u8> = (0..WIDTH_PX * HEIGHT_PX).map(|i| (i * 2) as u8).collect();
    let file = write(&job(1), &[layer_from(&pixels)]);

    let row = u32_at(&file, LAYER_TABLE_ADDRESS) as usize + 20;
    let address = u32_at(&file, row) as usize;
    let size = u32_at(&file, row + 4) as usize;

    let runs = decode(&file[address..address + size]).expect("our own encoder is well formed");
    let decoded: Vec<u8> = runs
        .iter()
        .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
        .collect();

    assert_eq!(decoded.len(), pixels.len(), "every pixel comes back");
    for (at, (&out, &back)) in pixels.iter().zip(decoded.iter()).enumerate() {
        // Four bits of grey: the top nibble survives and is repeated into both halves, so
        // a value comes back within one step of seventeen of where it went in.
        let nibble = out >> 4;
        assert_eq!(
            back,
            nibble << 4 | nibble,
            "pixel {at} went in as {out} and came back as {back}"
        );
    }

    let lit = pixels.iter().filter(|value| *value >= &16).count() as u32;
    assert_eq!(
        u32_at(&file, row + 24),
        lit,
        "the row counts the pixels the file cures, not the ones the mask asked for"
    );
}

#[test]
fn the_volume_handed_to_finish_is_the_one_in_the_header() {
    // A caller that only learns the volume by slicing opens the file with zero; see
    // docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md. The header states it
    // in millilitres.
    let mut opened = job(1);
    opened.volume_mm3 = 0.0;
    let file = write_closing_with(
        &opened,
        &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()],
        4321.0,
    );

    let header = u32_at(&file, HEADER_ADDRESS) as usize + 16;
    assert!(
        (f32_at(&file, header + 36) - 4.321).abs() < 1e-6,
        "the header states the volume in millilitres"
    );
}

#[test]
fn the_preview_carries_the_job_thumbnail_as_five_six_five_colour() {
    let job = PrintJob {
        thumbnail: Some(Thumbnail::filled(8, 8, [255, 0, 0])),
        ..job(1)
    };
    let file = write(&job, &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()]);

    let preview = u32_at(&file, PREVIEW_ADDRESS) as usize;
    assert_eq!(u32_at(&file, preview + 16), 224);
    assert_eq!(
        u32_at(&file, preview + 20),
        42,
        "a word the vendor's slicer always states sits between the two sides"
    );
    assert_eq!(u32_at(&file, preview + 24), 168);

    // A square thumbnail in a 224 by 168 record is a 168 by 168 block with padding either
    // side, so the middle is the model and the corner is not. The words are little-endian.
    let pixels = preview + 28;
    let word =
        |index: usize| u16::from_le_bytes([file[pixels + 2 * index], file[pixels + 2 * index + 1]]);
    assert_eq!(word(84 * 224 + 112), 0xF800, "red, as five-six-five");
    assert_eq!(word(0), 0x0000, "the padding stays background");
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
    let file = write(
        &banded,
        &vec![layer_from(&vec![255; (WIDTH_PX * HEIGHT_PX) as usize]); 4],
    );

    let table = u32_at(&file, LAYER_TABLE_ADDRESS) as usize + 20;
    let exposures: Vec<f32> = (0..4)
        .map(|index| f32_at(&file, table + index * LAYER_DEF_BYTES + 16))
        .collect();

    assert_eq!(exposures, vec![30.0, 9.0, 9.0, 2.0]);
    let header = u32_at(&file, HEADER_ADDRESS) as usize + 16;
    assert_eq!(
        u32_at(&file, header + 64),
        0,
        "the per-layer flag stays clear, as the vendor's slicer leaves it; the rows carry it"
    );
}

#[test]
fn every_revision_is_the_same_tables_under_its_own_number() {
    let layers = [LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()];
    let files: Vec<_> = [
        (AnycubicVersion::V1, 1),
        (AnycubicVersion::V515, 1),
        (AnycubicVersion::V516, 1),
        (AnycubicVersion::V517, 7),
        (AnycubicVersion::V518, 15),
    ]
    .into_iter()
    .map(|(version, properties)| {
        let writer = AnycubicWriter::new(AnycubicFlavour::Pwmx, version);
        let file = write_version(writer, &job(1), &layers, 0.0);
        let machine = u32_at(&file, MACHINE_ADDRESS) as usize;
        assert_eq!(u32_at(&file, 0x0C), version.number());
        assert_eq!(u32_at(&file, machine + 132), properties, "{version:?}");
        assert_eq!(
            u32_at(&file, machine + 148),
            version.number(),
            "{version:?}"
        );
        file
    })
    .collect();
    let without_numbers = |file: &Vec<u8>| {
        let machine = u32_at(file, MACHINE_ADDRESS) as usize;
        let mut bytes = file.clone();
        for at in [0x0C, machine + 132, machine + 148] {
            bytes[at..at + 4].fill(0);
        }
        bytes
    };
    for file in &files[1..] {
        assert_eq!(without_numbers(file), without_numbers(&files[0]));
    }
}

#[test]
fn the_header_states_the_panels_grey_and_ends_as_the_vendor_slicer_ends_it() {
    let mut blurred = job(1);
    blurred.raster.grey.levels = std::num::NonZeroU8::new(4);
    blurred.raster.blur_px = 2;
    let file = write(
        &blurred,
        &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()],
    );
    let fields = u32_at(&file, HEADER_ADDRESS) as usize + 16;
    assert_eq!(
        u32_at(&file, fields + 40),
        4,
        "four greys is anti-alias level four"
    );
    assert_eq!(u32_at(&file, fields + 80), 0, "advanced mode off");
    assert_eq!(
        u32_at(&file, fields + 84),
        4 | 2 << 16,
        "the grey level repeats it, and the blur is two pixels"
    );
    assert_eq!(u32_at(&file, fields + 88), 10, "the standard resin type");
    assert_eq!(u32_at(&file, fields + 92), 0, "intelligent mode off");
    let grey = u32_at(&file, COLOUR_ADDRESS) as usize;
    assert_eq!(&file[grey..grey + 8], &[0; 8], "a grey table of no levels");
}

#[test]
fn the_second_layer_table_repeats_the_first() {
    let layers: Vec<_> = (0..3)
        .map(|index| layer_from(&vec![index * 40 + 40; (WIDTH_PX * HEIGHT_PX) as usize]))
        .collect();
    let file = write(&job(3), &layers);

    let machine = u32_at(&file, MACHINE_ADDRESS) as usize;
    let pixel_um = (f32_at(&file, machine + 0x9C), f32_at(&file, machine + 0xA0));
    assert!(
        (pixel_um.0 - 100.0).abs() < 1e-3 && (pixel_um.1 - 100.0).abs() < 1e-3,
        "a 0.1 mm pixel is 100 um along each axis, got {pixel_um:?}"
    );
    assert_eq!(u32_at(&file, machine + 0xC4), 1, "one panel");
    assert_eq!(
        u32_at(&file, machine + 0xCC),
        WIDTH_PX | HEIGHT_PX << 16,
        "the panel's resolution as two u16"
    );

    let sub_layers = u32_at(&file, SUB_LAYERS_ADDRESS) as usize;
    assert_eq!(u32_at(&file, sub_layers + 12), 24 + 44 * 3);
    assert_eq!(u32_at(&file, sub_layers + 16), 3);
    let table = u32_at(&file, LAYER_TABLE_ADDRESS) as usize + 20;
    for index in 0..3 {
        let row = table + index * LAYER_DEF_BYTES;
        let sub_row = sub_layers + 24 + index * 44;
        assert_eq!(
            (
                u32_at(&file, sub_row),
                u32_at(&file, sub_row + 4),
                u32_at(&file, sub_row + 8)
            ),
            (
                u32_at(&file, row),
                u32_at(&file, row + 4),
                u32_at(&file, row + 24)
            ),
            "layer {index} is the same data in both tables"
        );
    }
    assert_eq!(
        u32_at(&file, LAYER_DATA_ADDRESS) as usize,
        sub_layers + 24 + 44 * 3,
        "the layers follow the second table"
    );
    let open = AnycubicReader
        .open(std::io::Cursor::new(file))
        .expect("the file reads back");
    assert_eq!(open.facts().machine.as_deref(), Some("Test panel"));
}

#[test]
fn an_extension_is_written_at_the_revision_the_vendor_slicer_picks() {
    assert_eq!(AnycubicFlavour::Pwx.newest_version(), AnycubicVersion::V1);
    assert_eq!(AnycubicFlavour::Dlp.newest_version(), AnycubicVersion::V515);
    assert_eq!(
        AnycubicFlavour::Pwmx.newest_version(),
        AnycubicVersion::V516
    );
    assert_eq!(
        AnycubicFlavour::Pwmb.newest_version(),
        AnycubicVersion::V517
    );
    for flavour in [
        AnycubicFlavour::Pm5s,
        AnycubicFlavour::M5sp,
        AnycubicFlavour::Pm4n,
    ] {
        assert_eq!(
            flavour.newest_version(),
            AnycubicVersion::V518,
            "{flavour:?}"
        );
    }
}

/// A one-layer V1 file made a `.pws`: `levels` passes in the header and `passes` as the
/// layer's bytes, appended and addressed from the layer table.
fn as_pws(levels: u32, passes: &[u8]) -> Vec<u8> {
    let writer = AnycubicWriter::new(AnycubicFlavour::Pw0, AnycubicVersion::V1);
    let mut file = write_version(writer, &job(1), &[layer_from(&[0; 128])], 1.0);
    // The header's fields start behind its 16-byte name and length; 0x28 is the level.
    let level_at = u32_at(&file, HEADER_ADDRESS) as usize + 16 + 0x28;
    file[level_at..level_at + 4].copy_from_slice(&levels.to_le_bytes());
    let row = u32_at(&file, LAYER_TABLE_ADDRESS) as usize + 20;
    let offset = u32::try_from(file.len()).expect("a small file");
    file[row..row + 4].copy_from_slice(&offset.to_le_bytes());
    let size = u32::try_from(passes.len()).expect("a short layer");
    file[row + 4..row + 8].copy_from_slice(&size.to_le_bytes());
    file.extend_from_slice(passes);
    file
}

#[test]
fn a_pws_layer_is_its_passes_summed_into_grey() {
    // Two passes over 128 pixels: the first lights 64 from the left, the second 32.
    let file = as_pws(2, &[0x80 | 64, 64, 0x80 | 32, 96]);
    let mut open = PwsReader
        .open(std::io::Cursor::new(file))
        .expect("a .pws of two passes opens");
    assert_eq!(open.facts().format, "pws");
    assert_eq!(open.facts().grey_steps, 3, "none, one or both passes");
    let runs: Vec<(u32, u8)> = open
        .layer(0)
        .expect("both passes fill the panel")
        .iter()
        .map(|run| (run.length, run.value))
        .collect();
    assert_eq!(
        runs,
        [(32, 255), (32, 127), (64, 0)],
        "lit in c of 2 passes is c * 128 - 1"
    );
}

#[test]
fn a_pws_asking_for_no_passes_is_refused() {
    let refused = PwsReader.open(std::io::Cursor::new(as_pws(0, &[])));
    assert!(matches!(
        refused.err(),
        Some(FormatError::AntiAliasLevels { levels: 0, .. })
    ));
}
