//! Writes a `.cws` and reads it back through the public reader: the settings come from
//! `slice.conf`, every layer from its own PNG, and the exposures from the program.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::{Cursor, Read};
use std::path::Path;

use core_format::{
    ExposurePlan, LayerPlan, LayerSink, OpenFile, PrintJob, SlicedFileReader, SlicedFileWriter,
};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_cws::{CwsReader, CwsSink, CwsWriter, claims};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 24;
const HEIGHT_PX: u32 = 12;

fn printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(
        r#"
name = "Bene Test 4"
manufacturer = "Nova3D"

[display]
width_px = 24
height_px = 12
width_mm = 2.4
height_mm = 1.2

[build_volume]
x = 2.4
y = 1.2
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
            name: "Test resin".to_owned(),
            layer_height_mm: 0.05,
            exposure_s: 2.5,
            bottom_exposure_s: 30.0,
            bottom_layers: 2,
            transition_layers: 0,
            light_off_delay_s: 1.0,
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

/// A block of full exposure with a grey border, which is the shape whose edge an eight-bit
/// image keeps and a one-bit one would not.
fn layer() -> LayerRuns {
    let mut mask = LayerMask::new(WIDTH_PX, HEIGHT_PX);
    let pixels = mask.pixels_mut();
    for y in 2..9u32 {
        for x in 3..20u32 {
            let edge = x == 3 || x == 19 || y == 2 || y == 8;
            pixels[(y * WIDTH_PX + x) as usize] = if edge { 120 } else { 255 };
        }
    }
    LayerRuns::from_mask(&mask)
}

fn write(job: &PrintJob, layers: &[LayerRuns], volume_mm3: f32) -> Vec<u8> {
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = CwsWriter
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(CwsSink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

fn text_entry(file: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(file.to_vec())).expect("it is an archive");
    let mut entry = zip.by_name(name).expect("the entry is there");
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("it is text");
    text
}

#[test]
fn the_archive_holds_the_settings_an_image_a_layer_and_one_program() {
    let file = write(&job(3), &[layer(), layer(), layer()], 1234.0);
    let zip = zip::ZipArchive::new(Cursor::new(file.clone())).expect("it is an archive");
    let names: Vec<String> = zip.file_names().map(str::to_owned).collect();

    assert!(names.contains(&"slice.conf".to_owned()));
    assert!(names.contains(&"encrust.gcode".to_owned()));
    for index in 0..3 {
        assert!(
            names.contains(&format!("encrust000{index}.png")),
            "{names:?}"
        );
    }
    assert_eq!(zip.len(), 5, "nothing else is in it");

    let conf = text_entry(&file, "slice.conf");
    assert!(conf.contains("xres                    = 24"), "{conf}");
    assert!(conf.contains("layers_num              = 3"), "{conf}");
}

#[test]
fn the_program_states_each_layers_own_exposure() {
    let file = write(&job(3), &[layer(), layer(), layer()], 10.0);
    let program = text_entry(&file, "encrust.gcode");

    assert!(program.contains(";<Slice> 0\n"));
    assert!(program.contains(";<Delay> 30000\n"), "the bottom exposure");
    assert!(program.contains(";<Delay> 2500\n"), "the normal exposure");
    assert!(program.trim_end().ends_with(";<Completed>"));
}

#[test]
fn every_layer_reads_back_the_pixels_it_went_in_as() {
    let runs = layer();
    let file = write(&job(3), &[runs.clone(), runs.clone(), runs.clone()], 900.0);
    let mut source = Cursor::new(file.clone());
    assert!(
        claims(&mut source),
        "the settings entry names the container"
    );

    let mut opened = CwsReader
        .open(&mut source)
        .expect("what we wrote is one of these");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), 3);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.resin.as_deref(), Some("Test resin"));
    assert!(
        facts
            .slicer
            .as_deref()
            .is_some_and(|slicer| slicer.starts_with("Encrust-")),
        "{:?}",
        facts.slicer
    );
    assert!((facts.layer_height_mm - 0.05).abs() < 1e-6);
    assert!((facts.exposure_s - 2.5).abs() < 1e-6);
    assert!((facts.bottom_exposure_s - 30.0).abs() < 1e-6);
    assert_eq!(facts.bottom_layers, 2);
    assert_eq!(facts.display_mm, Some((2.4, 1.2)));
    assert!((facts.layers[0].exposure_s - 30.0).abs() < 1e-3);
    assert!((facts.layers[2].exposure_s - 2.5).abs() < 1e-3);

    for index in 0..3 {
        assert_eq!(
            opened.layer(index).expect("the layer reads back"),
            runs.runs(),
            "layer {index} carries all eight bits of every grey"
        );
    }
}

#[test]
fn a_blank_stack_still_reads_back_as_layers() {
    let blank: Vec<_> = (0..2)
        .map(|_| LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish())
        .collect();
    let file = write(&job(2), &blank, 0.0);
    let mut source = Cursor::new(file);
    let mut opened = CwsReader.open(&mut source).expect("it still opens");

    assert_eq!(opened.facts().layer_count(), 2);
    let runs = opened.layer(1).expect("a blank layer reads back");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].value, 0);
}

#[test]
fn every_shipped_machine_that_reads_this_container_can_be_written() {
    let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut found = 0;

    for entry in catalogue.printers() {
        if entry.profile.output != printer_profiles::OutputFormat::Cws {
            continue;
        }
        found += 1;
        let mut job = job(1);
        job.printer = entry.profile.clone();
        job.raster.width_px = entry.profile.display.width_px;
        job.raster.height_px = entry.profile.display.height_px;

        let mut file = Cursor::new(Vec::new());
        CwsWriter
            .begin(&job, &mut file)
            .unwrap_or_else(|error| panic!("{} cannot be written: {error}", entry.id));
    }
    assert!(found >= 5, "only {found} machines name this container");
}
