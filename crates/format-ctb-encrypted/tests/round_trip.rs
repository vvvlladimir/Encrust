//! Reads a known file, and writes a file and reads it back the way
//! a machine would: settings deciphered, signature held, every layer decoding to the
//! pixels it went in as.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::path::Path;

use core_format::{
    ExposurePlan, FormatError, LayerPlan, LayerSink, OpenFile, PrintJob, SlicedFileReader,
    SlicedFileWriter, WaitMode, Waits,
};
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use format_ctb_encrypted::{CtbEncryptedReader, CtbEncryptedSink, CtbEncryptedWriter};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 16;
const HEIGHT_PX: u32 = 8;

/// The first three layers of a cube sliced for a Saturn 4 Ultra, with
/// the test resin's unique numbers: 2.37 s, 31.3 s, seven bottom layers, 0.035 mm.
fn known_file() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/saturn-4-ultra-v5.ctb");
    std::fs::read(path).expect("the fixture is checked in")
}

#[test]
fn a_known_file_gives_its_settings_resin_and_layers() {
    let mut open = CtbEncryptedReader
        .open(Cursor::new(known_file()))
        .expect("the known file opens");
    let facts = open.facts().clone();
    assert_eq!(facts.version, Some(5));
    assert_eq!((facts.width_px, facts.height_px), (11_520, 5_120));
    assert_eq!(facts.machine.as_deref(), Some("ELEGOO Saturn 4 Ultra"));
    assert_eq!(facts.resin.as_deref(), Some("ENCRUST-TEST"));
    assert!((facts.exposure_s - 2.37).abs() < 1e-4);
    assert!((facts.bottom_exposure_s - 31.3).abs() < 1e-4);
    assert_eq!((facts.bottom_layers, facts.layer_count()), (7, 3));
    assert_eq!(facts.stated.light_pwm, Some(211));
    assert_eq!(facts.stated.transition_layers, Some(6));
    assert_eq!(
        facts.stated.lift_speed_mm_min, None,
        "a tilting vat's 0.05 mm/min is a placeholder"
    );
    let waits = facts.stated.waits.expect("the file rests");
    assert_eq!(waits.mode, WaitMode::Rest);
    assert!((waits.before_lift_s - 1.3).abs() < 1e-4);
    assert!((waits.after_lift_s - 0.7).abs() < 1e-4);
    assert!((waits.after_retract_s - 2.1).abs() < 1e-4);

    let expected = u64::from(facts.width_px) * u64::from(facts.height_px);
    for index in 0..facts.layer_count() {
        let runs = open
            .layer(index)
            .expect("every layer deciphers and decodes");
        let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
        assert_eq!(covered, expected, "layer {index} covers the whole panel");
        assert!(runs.iter().any(|run| run.value > 0), "layer {index} cures");
    }
}

#[test]
fn a_file_whose_settings_were_altered_is_refused() {
    let mut bytes = known_file();
    // The settings start behind the 48-byte header; one flipped byte changes the checksum.
    bytes[48] ^= 1;
    let refused = CtbEncryptedReader.open(Cursor::new(bytes)).err();
    assert!(
        matches!(refused, Some(FormatError::SignatureMismatch)),
        "{refused:?}"
    );
}

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
        Path::new("inline.toml"),
    )
    .expect("the inline profile is valid")
}

fn job(layer_count: u32) -> PrintJob {
    let material = MaterialProfile {
        name: "Bench grey".to_owned(),
        bottom_layers: 1,
        transition_layers: 2,
        light_pwm: 211,
        lift_distance_mm: 5.5,
        lift_speed_mm_min: 60.0,
        retract_speed_mm_min: 180.0,
        waits: Waits {
            mode: WaitMode::Rest,
            before_lift_s: 1.25,
            after_lift_s: 0.75,
            after_retract_s: 2.0,
        },
        ..MaterialProfile::default()
    };
    PrintJob {
        printer: printer(),
        plan: LayerPlan::of_count(material.layer_height_mm, layer_count as usize),
        material,
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
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = CtbEncryptedWriter
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(CtbEncryptedSink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(job.volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

#[test]
fn a_written_file_reads_back_as_it_went_in() {
    let mut first = [0u8; 128];
    first[17..30].fill(255);
    first[40..44].fill(128);
    let second = [255u8; 128];
    let layers = [layer_from(&first), layer_from(&second)];
    let job = job(2);
    let bytes = write(&job, &layers);

    let mut open = CtbEncryptedReader
        .open(Cursor::new(bytes))
        .expect("our own file opens and its signature holds");
    let facts = open.facts().clone();
    assert_eq!(
        (facts.width_px, facts.height_px, facts.layer_count()),
        (16, 8, 2)
    );
    assert_eq!(facts.resin.as_deref(), Some("Bench grey"));
    assert_eq!(facts.machine.as_deref(), Some(job.printer.machine_name()));
    assert_eq!(facts.stated.light_pwm, Some(211));
    assert_eq!(facts.stated.transition_layers, Some(2));
    assert_eq!(facts.stated.lift_distance_mm, Some(5.5));
    assert_eq!(facts.stated.waits, Some(job.material.waits));
    assert!((facts.volume_mm3.expect("stated") - 1234.0).abs() < 0.1);

    for (index, layer) in layers.iter().enumerate() {
        let runs = open.layer(index as u32).expect("the layer decodes");
        let mut decoded = LayerRuns::builder(WIDTH_PX, HEIGHT_PX);
        for run in runs {
            decoded.push(run.length, run.value);
        }
        let went_in = layer.to_mask();
        let came_out = decoded.finish().to_mask();
        for (pixel, (a, b)) in went_in.pixels().iter().zip(came_out.pixels()).enumerate() {
            assert!(
                a.abs_diff(*b) <= 1,
                "layer {index} pixel {pixel}: {a} went in, {b} came out; seven bits keep 1 in 2"
            );
        }
    }
}

#[test]
fn a_file_cut_short_is_refused_rather_than_read_past() {
    let bytes = write(&job(1), &[layer_from(&[255; 128])]);
    let refused = CtbEncryptedReader.open(Cursor::new(bytes[..bytes.len() - 40].to_vec()));
    assert!(refused.is_err(), "the signature is past the end");
}
