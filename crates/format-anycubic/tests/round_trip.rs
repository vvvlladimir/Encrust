//! Writes an Anycubic file and walks it the way a reader would: the file mark must address
//! every table it claims, each table must name itself, and every layer must decode to the
//! pixels it went in as.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_format::{
    ExposurePlan, ExposureRange, LayerPlan, LayerSink, PrintJob, SlicedFileWriter, Thumbnail,
};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_anycubic::{AnycubicFlavour, AnycubicSink, AnycubicVersion, AnycubicWriter, decode};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 16;
const HEIGHT_PX: u32 = 8;

const LAYER_DEF_BYTES: usize = 32;

/// Where the file mark states each table is, by its byte position in the mark.
const HEADER_ADDRESS: usize = 0x14;
const PREVIEW_ADDRESS: usize = 0x1C;
const LAYER_TABLE_ADDRESS: usize = 0x24;
const LAYER_DATA_ADDRESS: usize = 0x2C;

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
    assert_eq!(u32_at(&file, 0x0C), 1, "version 1 is what we write");
    assert_eq!(u32_at(&file, 0x10), 4, "four tables carry a version 1 file");

    let header = u32_at(&file, HEADER_ADDRESS) as usize;
    let preview = u32_at(&file, PREVIEW_ADDRESS) as usize;
    let table = u32_at(&file, LAYER_TABLE_ADDRESS) as usize;
    let data = u32_at(&file, LAYER_DATA_ADDRESS) as usize;

    assert_eq!(table_name(&file, header), "HEADER");
    assert_eq!(table_name(&file, preview), "PREVIEW");
    assert_eq!(table_name(&file, table), "LAYERDEF");
    assert!(
        header < preview && preview < table && table < data,
        "the tables are written in the order the mark lists them"
    );
    assert!(data <= file.len(), "the layer data is inside the file");

    // The blocks a later revision adds are addressed with a zero, not left out.
    for at in [0x18, 0x20, 0x28] {
        assert_eq!(u32_at(&file, at), 0);
    }
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

    assert_eq!(u32_at(&file, header + 12), 80, "the header's fields alone");
    assert_eq!(
        u32_at(&file, preview + 12) as usize,
        preview_end(&file) - preview,
        "the preview counts its own name and length in, where no other table does"
    );
    assert_eq!(
        u32_at(&file, table + 12) as usize,
        4 + 4 * LAYER_DEF_BYTES,
        "the layer table counts the layer count and its rows"
    );
    assert_eq!(u32_at(&file, table + 16), 4, "four layers");
}

/// Where the preview table ends, which is where the layer table begins.
fn preview_end(file: &[u8]) -> usize {
    u32_at(file, LAYER_TABLE_ADDRESS) as usize
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
        String::from_utf8_lossy(&file[preview + 20..preview + 24]).trim_end_matches('\0'),
        "x",
        "a mark of its own sits between the two resolutions"
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
        1,
        "the header tells the machine the rows may differ from it"
    );
}

/// Where the file mark states the blocks only a later revision carries.
const SOFTWARE_ADDRESS: usize = 0x18;
const COLOUR_ADDRESS: usize = 0x20;
const EXTRA_ADDRESS: usize = 0x28;
const MACHINE_ADDRESS: usize = 0x2C;

#[test]
fn revision_516_adds_the_grey_table_and_the_machine_block() {
    let layers = [LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()];
    let file = write_version(
        AnycubicWriter::new(AnycubicFlavour::Pwmx, AnycubicVersion::V516),
        &job(1),
        &layers,
        0.0,
    );

    assert_eq!(u32_at(&file, 0x0C), 516);
    assert_eq!(u32_at(&file, 0x10), 8, "eight blocks carry a 516 file");

    let header = u32_at(&file, HEADER_ADDRESS) as usize;
    assert_eq!(u32_at(&file, header + 12), 84, "516 adds one header field");

    let colour = u32_at(&file, COLOUR_ADDRESS) as usize;
    let table = u32_at(&file, LAYER_TABLE_ADDRESS) as usize;
    assert!(
        colour > 0 && colour < table,
        "the grey table sits between the preview and the layer table"
    );
    assert_eq!(u32_at(&file, colour + 4), 16, "sixteen grey levels");

    let extra = u32_at(&file, EXTRA_ADDRESS) as usize;
    let machine = u32_at(&file, MACHINE_ADDRESS) as usize;
    assert_eq!(table_name(&file, extra), "EXTRA");
    assert_eq!(table_name(&file, machine), "MACHINE");
    assert_eq!(
        u32_at(&file, machine + 12),
        156,
        "the machine block counts its own name and length in"
    );
    assert_eq!(
        u32_at(&file, SOFTWARE_ADDRESS),
        0,
        "the slicer block arrives at 517, not here"
    );

    // The layer data address moved one slot along, because the mark gained the machine.
    let data = u32_at(&file, 0x30) as usize;
    assert!(data > machine && data <= file.len());
}

#[test]
fn revision_517_adds_the_slicer_and_model_blocks() {
    let layers = [LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()];
    let file = write_version(
        AnycubicWriter::new(AnycubicFlavour::Pm5, AnycubicVersion::V517),
        &job(1),
        &layers,
        0.0,
    );

    assert_eq!(u32_at(&file, 0x0C), 517);
    assert_eq!(u32_at(&file, 0x10), 9, "nine blocks carry a 517 file");

    let header = u32_at(&file, HEADER_ADDRESS) as usize;
    assert_eq!(u32_at(&file, header + 12), 92, "517 adds three more fields");

    let software = u32_at(&file, SOFTWARE_ADDRESS) as usize;
    let model = u32_at(&file, 0x34) as usize;
    assert!(software > 0, "the slicer block is addressed");
    assert_eq!(
        u32_at(&file, software + 32),
        164,
        "the slicer block states its length in the middle of itself"
    );
    assert_eq!(table_name(&file, model), "MODEL");

    let data = u32_at(&file, 0x30) as usize;
    assert!(data > model, "the layers come last");
    assert!(data <= file.len());
}

#[test]
fn an_extension_names_the_newest_revision_its_machines_read() {
    assert_eq!(AnycubicFlavour::Pwx.newest_version(), AnycubicVersion::V1);
    assert_eq!(
        AnycubicFlavour::Pwmx.newest_version(),
        AnycubicVersion::V516
    );
    assert_eq!(AnycubicFlavour::Pm5.newest_version(), AnycubicVersion::V517);
}
