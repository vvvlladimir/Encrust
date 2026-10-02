//! Writes an `.svgx` and reads it back through the public reader: the layers are polygons
//! in millimetres, so what comes back is the mask they fill rather than the bytes that
//! went in.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::path::Path;

use core_format::{
    ExposurePlan, LayerPlan, LayerSink, OpenFile, PrintJob, SlicedFileReader, SlicedFileWriter,
};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_svgx::{SvgxReader, SvgxSink, SvgxWriter};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 24;
const HEIGHT_PX: u32 = 12;
const PITCH_MM: f32 = 0.05;

fn printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(
        r#"
name = "Foto Test 8.9"
manufacturer = "FlashForge"
machine_name = "Foto 8.9"

[display]
width_px = 24
height_px = 12
width_mm = 1.2
height_mm = 0.6

[build_volume]
x = 1.2
y = 0.6
z = 20.0
"#,
        Path::new("inline.toml"),
    )
    .expect("the inline profile is valid")
}

fn job(layer_count: u32) -> PrintJob {
    PrintJob {
        printer: printer(),
        material: MaterialProfile {
            name: "Test resin".to_owned(),
            layer_height_mm: 0.05,
            exposure_s: 2.6,
            bottom_exposure_s: 32.0,
            bottom_layers: 2,
            transition_layers: 0,
            ..MaterialProfile::default()
        },
        raster: RasterSettings {
            width_px: WIDTH_PX,
            height_px: HEIGHT_PX,
            pitch: PixelPitch {
                x: PITCH_MM,
                y: PITCH_MM,
            },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Binary,
            grey: Grey::default(),
            blur_px: 0,
        },
        plan: LayerPlan::of_count(0.05, layer_count as usize),
        volume_mm3: 0.0,
        exposure: ExposurePlan::default(),
        thumbnail: None,
    }
}

/// A block with a square hole in it, which is the shape a vector container either cuts or
/// paints over.
fn layer() -> LayerRuns {
    let mut mask = LayerMask::new(WIDTH_PX, HEIGHT_PX);
    let pixels = mask.pixels_mut();
    for y in 2..10u32 {
        for x in 3..20u32 {
            pixels[(y * WIDTH_PX + x) as usize] = 255;
        }
    }
    for y in 5..7u32 {
        for x in 9..13u32 {
            pixels[(y * WIDTH_PX + x) as usize] = 0;
        }
    }
    LayerRuns::from_mask(&mask)
}

fn write(job: &PrintJob, layers: &[LayerRuns], volume_mm3: f32) -> Vec<u8> {
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = SvgxWriter
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(SvgxSink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

fn pixels_of(runs: &[core_raster::Run]) -> Vec<u8> {
    runs.iter()
        .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
        .collect()
}

#[test]
fn the_file_is_headed_by_the_identifier_and_the_three_addresses() {
    let file = write(&job(2), &[layer(), layer()], 1234.0);

    assert!(file.starts_with(b"DLP-II 1.1\n"));
    let address =
        |at: usize| u32::from_le_bytes([file[at], file[at + 1], file[at + 2], file[at + 3]]);
    assert_eq!(address(16), 28, "the first preview follows the header");
    assert!(address(20) > address(16), "the second follows the first");

    let document = address(24) as usize;
    let text = String::from_utf8_lossy(&file[document..]);
    assert!(text.starts_with("<?xml"), "the document is text");
    assert!(text.contains("<g id=\"layer-1\""));
    assert!(text.ends_with("</svg>\n"));
}

#[test]
fn the_volume_is_filled_in_once_the_stack_is_measured() {
    let file = write(&job(1), &[layer()], 2500.0);
    let text = String::from_utf8_lossy(&file);
    assert!(
        text.contains("volume=\"000000002.500\""),
        "millilitres in the field the head reserved"
    );
}

#[test]
fn every_layer_reads_back_as_the_mask_it_was_traced_from() {
    let runs = layer();
    let file = write(&job(3), &[runs.clone(), runs.clone(), runs.clone()], 900.0);
    let mut source = Cursor::new(file);
    let mut opened = SvgxReader
        .open(&mut source)
        .expect("what we wrote is one of these");

    let facts = opened.facts();
    assert_eq!(facts.layer_count(), 3);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.machine.as_deref(), Some("Foto 8.9"));
    assert_eq!(facts.resin.as_deref(), Some("Test resin"));
    assert_eq!(facts.bottom_layers, 2);
    assert!((facts.exposure_s - 2.6).abs() < 1e-6);
    assert!((facts.bottom_exposure_s - 32.0).abs() < 1e-6);
    assert_eq!(facts.volume_mm3, Some(900.0));
    assert_eq!(facts.display_mm, Some((1.2, 0.6)));

    let expected = pixels_of(runs.runs());
    for index in 0..3 {
        let read = pixels_of(&opened.layer(index).expect("the layer reads back"));
        assert_eq!(read, expected, "layer {index}, hole and all");
    }
}

#[test]
fn a_blank_layer_carries_no_path_and_reads_back_dark() {
    let blank = LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish();
    let file = write(&job(1), &[blank], 0.0);
    assert!(
        !String::from_utf8_lossy(&file).contains("<path"),
        "a layer with nothing on it has nothing to fill"
    );

    let mut source = Cursor::new(file);
    let mut opened = SvgxReader.open(&mut source).expect("it still opens");
    let runs = opened.layer(0).expect("a blank layer reads back");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].value, 0);
}

#[test]
fn every_shipped_machine_that_reads_this_container_can_be_written() {
    let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut found = 0;

    for entry in catalogue.printers() {
        if entry.profile.output != printer_profiles::OutputFormat::Svgx {
            continue;
        }
        found += 1;
        let mut job = job(1);
        job.printer = entry.profile.clone();
        job.raster.width_px = entry.profile.display.width_px;
        job.raster.height_px = entry.profile.display.height_px;

        let mut file = Cursor::new(Vec::new());
        SvgxWriter
            .begin(&job, &mut file)
            .unwrap_or_else(|error| panic!("{} cannot be written: {error}", entry.id));
    }
    assert!(found >= 8, "only {found} machines name this container");
}
