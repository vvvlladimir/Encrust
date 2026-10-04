//! Writes an `.sl1` and opens it the way a reader would: by name, because a zip is not
//! read in the order it was written, and every layer must decode to the pixels it went in
//! as.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Read;

use core_format::{ExposurePlan, LayerPlan, LayerSink, PrintJob, SlicedFileWriter, Thumbnail};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_sl1::{Sl1Flavour, Sl1Sink, Sl1Writer};
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
    let mut file = std::io::Cursor::new(Vec::new());
    {
        let mut sink = Sl1Writer::new(Sl1Flavour::Sl1, "plate")
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(Sl1Sink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

fn archive(bytes: &[u8]) -> zip::ZipArchive<std::io::Cursor<&[u8]>> {
    zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("what we wrote is a zip")
}

fn entry(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut zip = archive(bytes);
    let mut found = Vec::new();
    zip.by_name(name)
        .unwrap_or_else(|_| panic!("a reader looks up {name} by name"))
        .read_to_end(&mut found)
        .expect("the entry reads back");
    found
}

fn names(bytes: &[u8]) -> Vec<String> {
    archive(bytes)
        .file_names()
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn value_of(ini: &str, key: &str) -> Option<String> {
    ini.lines()
        .find_map(|line| line.strip_prefix(&format!("{key} = ")))
        .map(str::to_owned)
}

#[test]
fn the_archive_holds_every_entry_a_reader_looks_for() {
    let blank: Vec<_> = (0..3)
        .map(|_| LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish())
        .collect();
    let file = write(&job(3), &blank);

    assert_eq!(
        names(&file),
        vec![
            "config.ini",
            "plate00000.png",
            "plate00001.png",
            "plate00002.png",
            "prusaslicer.ini",
            "thumbnail/thumbnail400x400.png",
            "thumbnail/thumbnail800x480.png",
        ],
        "a layer is the job's own name and five digits, which is the shape a reader matches"
    );
}

#[test]
fn the_settings_files_are_written_last_and_still_found_by_name() {
    let file = write(&job(1), &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()]);

    // They go in behind the layers because one of their values is only known at the end.
    let order: Vec<String> = archive(&file).file_names().map(str::to_owned).collect();
    assert_eq!(
        order.last().map(String::as_str),
        Some("prusaslicer.ini"),
        "the settings trail the stack in the order the entries were added"
    );
    assert!(!entry(&file, "config.ini").is_empty());
    assert!(!entry(&file, "prusaslicer.ini").is_empty());
}

#[test]
fn the_stack_height_and_the_resin_reach_the_settings() {
    let mut opened = job(5);
    opened.volume_mm3 = 0.0;
    let blank: Vec<_> = (0..5)
        .map(|_| LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish())
        .collect();
    let file = write_closing_with(&opened, &blank, 4321.0);

    let config = String::from_utf8(entry(&file, "config.ini")).expect("the file is text");
    assert_eq!(value_of(&config, "numFast").as_deref(), Some("5"));
    assert_eq!(value_of(&config, "numSlow").as_deref(), Some("0"));
    assert_eq!(value_of(&config, "jobDir").as_deref(), Some("plate"));

    // A caller that only learns the volume by slicing opens the file with zero; see
    // docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md.
    assert_eq!(
        value_of(&config, "usedMaterial").as_deref(),
        Some("4.321"),
        "the settings state the resin the stack came to, in millilitres"
    );
}

#[test]
fn a_layer_decodes_to_the_pixels_it_went_in_as() {
    let pixels: Vec<u8> = (0..WIDTH_PX * HEIGHT_PX).map(|i| (i * 2) as u8).collect();
    let file = write(&job(1), &[layer_from(&pixels)]);

    let png = entry(&file, "plate00000.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(&png));
    let mut reader = decoder.read_info().expect("the entry is a PNG");
    assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
    assert_eq!(
        reader.info().color_type,
        png::ColorType::Grayscale,
        "the container carries every grey the mask had"
    );

    let mut decoded = vec![0; reader.output_buffer_size().expect("a bounded image")];
    let frame = reader
        .next_frame(&mut decoded)
        .expect("the pixels read back");
    assert_eq!(
        &decoded[..frame.buffer_size()],
        pixels.as_slice(),
        "an eight-bit container quantises nothing"
    );
}

#[test]
fn both_previews_carry_the_job_thumbnail() {
    let job = PrintJob {
        thumbnail: Some(Thumbnail::filled(8, 8, [255, 0, 0])),
        ..job(1)
    };
    let file = write(&job, &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()]);

    for (name, width, height) in [
        ("thumbnail/thumbnail400x400.png", 400u32, 400u32),
        ("thumbnail/thumbnail800x480.png", 800, 480),
    ] {
        let png = entry(&file, name);
        let decoder = png::Decoder::new(std::io::Cursor::new(&png));
        let mut reader = decoder.read_info().expect("the preview is a PNG");
        assert_eq!((reader.info().width, reader.info().height), (width, height));
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);

        let mut decoded = vec![0; reader.output_buffer_size().expect("a bounded image")];
        reader.next_frame(&mut decoded).expect("the preview reads");

        let middle = ((height / 2) * width + width / 2) as usize * 3;
        assert_eq!(
            &decoded[middle..middle + 3],
            &[255, 0, 0],
            "{name} is the part"
        );

        // A square thumbnail fills a square record and is padded either side of a wider
        // one, so only the wider record has a corner that is not the model.
        if width != height {
            assert_eq!(
                &decoded[..3],
                &[0, 0, 0],
                "{name} pads rather than stretching"
            );
        }
    }
}

#[test]
fn a_job_without_a_thumbnail_still_writes_both_previews() {
    let file = write(&job(1), &[LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish()]);
    for name in [
        "thumbnail/thumbnail400x400.png",
        "thumbnail/thumbnail800x480.png",
    ] {
        assert!(!entry(&file, name).is_empty());
    }
}
