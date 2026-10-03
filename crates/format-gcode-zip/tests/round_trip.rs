//! Writes the archive and reads it back through the public reader, because a zip is not
//! read in the order it was written and the program is the only record of the stack.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::{Cursor, Read};
use std::path::Path;

use core_format::{
    ExposurePlan, ExposureRange, LayerPlan, LayerSink, OpenFile, PrintJob, SlicedFileReader,
    SlicedFileWriter,
};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_gcode_zip::{GcodeZipReader, GcodeZipSink, GcodeZipWriter};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 16;
const HEIGHT_PX: u32 = 8;

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
        Path::new("inline.toml"),
    )
    .expect("the inline profile is valid")
}

fn job(layer_count: u32) -> PrintJob {
    PrintJob {
        printer: printer(),
        material: MaterialProfile {
            layer_height_mm: 0.05,
            exposure_s: 2.5,
            bottom_exposure_s: 30.0,
            bottom_layers: 2,
            transition_layers: 0,
            ..MaterialProfile::default()
        },
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
        plan: LayerPlan::of_count(0.05, layer_count as usize),
        volume_mm3: 0.0,
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

fn write(job: &PrintJob, layers: &[LayerRuns], volume_mm3: f32) -> Vec<u8> {
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = GcodeZipWriter
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(GcodeZipSink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

fn blank(count: u32) -> Vec<LayerRuns> {
    (0..count)
        .map(|_| LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish())
        .collect()
}

fn entry_names(bytes: &[u8]) -> Vec<String> {
    zip::ZipArchive::new(Cursor::new(bytes))
        .expect("what we wrote is a zip")
        .file_names()
        .map(str::to_owned)
        .collect()
}

fn program(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("what we wrote is a zip");
    let mut text = String::new();
    zip.by_name("run.gcode")
        .expect("a reader looks the program up by name")
        .read_to_string(&mut text)
        .expect("the program is text");
    text
}

#[test]
fn the_archive_holds_every_entry_a_reader_looks_for() {
    let file = write(&job(3), &blank(3), 1234.0);

    assert_eq!(
        entry_names(&file),
        vec![
            "preview.png",
            "preview_cropping.png",
            "1.png",
            "2.png",
            "3.png",
            "run.gcode",
        ],
        "layers are numbered from one, and the program trails the stack it describes"
    );
}

#[test]
fn the_program_states_the_resin_the_stack_came_to() {
    let file = write(&job(5), &blank(5), 4321.0);
    let text = program(&file);

    // A caller that only learns the volume by slicing opens the file with zero; see
    // docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md.
    assert!(text.contains("\n;volume:4.321\n"), "millilitres: {text}");
    assert!(text.contains("\n;totalLayer:5\n"));
}

#[test]
fn every_layer_reads_back_with_its_own_z_and_exposure() {
    let file = write(&job(4), &blank(4), 1000.0);
    let mut source = Cursor::new(file);
    let opened = GcodeZipReader
        .open(&mut source)
        .expect("what we wrote is one of these");
    let facts = opened.facts();

    assert_eq!(facts.layer_count(), 4);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert!((facts.layer_height_mm - 0.05).abs() < 1e-6);
    assert!((facts.exposure_s - 2.5).abs() < 1e-6);
    assert_eq!(facts.bottom_layers, 2);
    assert!((facts.bottom_exposure_s - 30.0).abs() < 1e-6);
    assert_eq!(facts.machine.as_deref(), Some("Test panel"));
    assert!(
        facts
            .slicer
            .as_deref()
            .is_some_and(|s| s.contains("Encrust")),
        "the program names what wrote it"
    );

    for (index, entry) in facts.layers.iter().enumerate() {
        let expected_z = 0.05 * (index + 1) as f32;
        assert!(
            (entry.z_mm - expected_z).abs() < 1e-6,
            "layer {index} stands at {expected_z} mm"
        );
        let expected_s = if index < 2 { 30.0 } else { 2.5 };
        assert!(
            (entry.exposure_s - expected_s).abs() < 1e-6,
            "layer {index} is exposed for {expected_s} s"
        );
    }
}

#[test]
fn a_layer_decodes_to_the_pixels_it_went_in_as() {
    let pixels: Vec<u8> = (0..WIDTH_PX * HEIGHT_PX).map(|i| (i * 2) as u8).collect();
    let runs = layer_from(&pixels);
    let file = write(&job(1), std::slice::from_ref(&runs), 100.0);

    let mut source = Cursor::new(file);
    let mut opened = GcodeZipReader
        .open(&mut source)
        .expect("what we wrote is one of these");
    assert_eq!(
        opened.layer(0).expect("the layer reads back"),
        runs.runs(),
        "an eight-bit container quantises nothing"
    );
}

#[test]
fn a_banded_exposure_and_a_mixed_stack_survive_the_round_trip() {
    let mut varied = job(4);
    varied.printer.firmware.variable_layer_height = true;
    varied.material.bottom_layers = 0;
    varied.plan = LayerPlan::from_bounds(vec![0.0, 0.05, 0.15, 0.2, 0.3], 0.3);
    varied.exposure = ExposurePlan::new(vec![ExposureRange::new(0.1, 0.25, 9.0)]);

    let file = write(&varied, &blank(4), 500.0);
    let mut source = Cursor::new(file);
    let opened = GcodeZipReader
        .open(&mut source)
        .expect("what we wrote is one of these");
    let layers = &opened.facts().layers;

    let heights: Vec<f32> = layers.iter().map(|layer| layer.z_mm).collect();
    assert_eq!(heights, vec![0.05, 0.15, 0.2, 0.3]);
    for (index, layer) in layers.iter().enumerate() {
        let expected_s = varied.exposure_of_layer_s(index as u32);
        assert!(
            (layer.exposure_s - expected_s).abs() < 1e-3,
            "layer {index} is exposed for {expected_s} s, not {}",
            layer.exposure_s
        );
    }
    assert!(
        (layers[2].exposure_s - 9.0).abs() < 1e-6,
        "the band reaches the 0.05 mm layer topping out at 0.2 mm as it stands: {layers:?}"
    );
    assert!(
        (layers[0].exposure_s - 2.5).abs() < 1e-6,
        "the layer below the band keeps the resin's own exposure"
    );
}
