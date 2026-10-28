//! Writes one stack in every container and reads each back, so a reader and the writer
//! beside it are checked against each other rather than against a fixture.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::path::Path;

use core_format::{
    ExposurePlan, LayerPlan, LayerSink, OpenFile, PrintJob, SlicedFileWriter, Thumbnail,
};
use core_pipeline::open;
use core_raster::{Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Shading};
use printer_profiles::{Buffer, MaterialProfile, PrinterProfile, PrinterTuning, WaitMode, Waits};

const WIDTH_PX: u32 = 64;
const HEIGHT_PX: u32 = 40;
const LAYERS: u32 = 5;

fn printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(
        r#"
name = "Read Back"
manufacturer = "Test"

# One container matches the machine's own CL or CT code and refuses a name without one.
machine_name = "Read Back CL-60"

[display]
width_px = 64
height_px = 40
width_mm = 6.4
height_mm = 4.0

[build_volume]
x = 6.4
y = 4.0
z = 10.0
"#,
        Path::new("inline.toml"),
    )
    .expect("the inline profile is valid")
}

fn job() -> PrintJob {
    PrintJob {
        printer: printer(),
        material: MaterialProfile {
            layer_height_mm: 0.05,
            exposure_s: 2.5,
            bottom_exposure_s: 30.0,
            bottom_layers: 2,
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
        plan: LayerPlan::of_count(0.05, LAYERS as usize),
        volume_mm3: 0.0,
        exposure: ExposurePlan::default(),
        thumbnail: Some(Thumbnail::filled(8, 8, [200, 40, 40])),
        created_unix_s: 0,
    }
}

/// A block of full exposure in the middle with a one-pixel grey border around it, which is
/// the shape whose edge every container quantises differently.
fn layer() -> LayerRuns {
    let mut mask = LayerMask::new(WIDTH_PX, HEIGHT_PX);
    let pixels = mask.pixels_mut();
    for y in 8..32u32 {
        for x in 8..56u32 {
            let edge = x == 8 || x == 55 || y == 8 || y == 31;
            pixels[(y * WIDTH_PX + x) as usize] = if edge { 120 } else { 255 };
        }
    }
    LayerRuns::from_mask(&mask)
}

/// Every container that carries grey, with the name a file of it takes and the greys its
/// layer data holds. The `.svgx` is not here: it carries none, and has its own test below.
fn written() -> Vec<(&'static str, Vec<u8>, u16)> {
    written_for(&job())
}

/// Every container, each holding a stack printed with `material`.
fn written_with(material: &MaterialProfile) -> Vec<(&'static str, Vec<u8>)> {
    let job = PrintJob {
        material: material.clone(),
        ..job()
    };
    written_for(&job)
        .into_iter()
        .map(|(extension, bytes, _)| (extension, bytes))
        .collect()
}

fn written_for(job: &PrintJob) -> Vec<(&'static str, Vec<u8>, u16)> {
    vec![
        ("goo", write_job(&format_goo::GooWriter, job), 256),
        (
            "ctb",
            write_job(
                &format_chitu::CtbWriter::new(format_chitu::CtbVersion::V4),
                job,
            ),
            128,
        ),
        (
            "ctb",
            write_job(&format_ctb_encrypted::CtbEncryptedWriter, job),
            128,
        ),
        (
            "cbddlp",
            write_job(&format_chitu::CbddlpWriter::default(), job),
            format_chitu::GREY_PASSES as u16 + 1,
        ),
        (
            "pwmx",
            write_job(&format_anycubic::AnycubicWriter::default(), job),
            16,
        ),
        (
            "sl1",
            write_job(
                &format_sl1::Sl1Writer::new(format_sl1::Sl1Flavour::Sl1, "plate"),
                job,
            ),
            256,
        ),
        (
            "zip",
            write_job(&format_gcode_zip::GcodeZipWriter, job),
            256,
        ),
        ("cxdlp", write_job(&format_creality::CxdlpWriter, job), 256),
        (
            "cxdlpv4",
            write_job(&format_creality::CxdlpV4Writer, job),
            64,
        ),
        ("cws", write_job(&format_cws::CwsWriter, job), 256),
    ]
}

fn write_with<W: SlicedFileWriter>(writer: &W) -> Vec<u8> {
    write_job(writer, &job())
}

fn write_job<W: SlicedFileWriter>(writer: &W, job: &PrintJob) -> Vec<u8> {
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = writer
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for _ in 0..LAYERS {
            sink.push(W::Sink::encode(&layer())).expect("a layer");
        }
        sink.finish(1234.0).expect("every promised layer arrived");
    }
    file.into_inner()
}

#[test]
fn every_container_reads_back_what_it_was_told() {
    for (extension, bytes, grey_steps) in written() {
        let name = format!("plate.{extension}");
        let mut source = Cursor::new(bytes);
        let opened = open(Path::new(&name), &mut source)
            .unwrap_or_else(|error| panic!("a .{extension} we wrote must open: {error}"));
        let facts = opened.facts();

        assert_eq!(facts.layer_count(), LAYERS, "{extension} layer count");
        assert_eq!(facts.width_px, WIDTH_PX, "{extension} width");
        assert_eq!(facts.height_px, HEIGHT_PX, "{extension} height");
        assert_eq!(facts.grey_steps, grey_steps, "{extension} greys");
        assert!(
            (facts.layer_height_mm - 0.05).abs() < 1e-6,
            "{extension} layer height"
        );
        assert!(
            (facts.exposure_s - 2.5).abs() < 1e-3,
            "{extension} exposure, got {}",
            facts.exposure_s
        );
        assert!(
            (facts.height_mm() - 0.25).abs() < 1e-3,
            "{extension} stack height, got {}",
            facts.height_mm()
        );
    }
}

/// A resin whose every motion and light setting is a number no default holds, each one
/// surviving a container's units exactly.
fn told() -> MaterialProfile {
    MaterialProfile {
        lift_distance_mm: 5.5,
        retract_distance_mm: 5.5,
        lift_speed_mm_min: 60.0,
        retract_speed_mm_min: 180.0,
        bottom_lift_distance_mm: 6.5,
        bottom_lift_speed_mm_min: 120.0,
        bottom_retract_speed_mm_min: 150.0,
        light_pwm: 211,
        bottom_light_pwm: 233,
        transition_layers: 2,
        waits: Waits {
            mode: WaitMode::Rest,
            before_lift_s: 1.25,
            after_lift_s: 0.75,
            after_retract_s: 2.0,
        },
        ..job().material
    }
}

/// What each container has a place for of `told`, as its reader states it back.
fn stated_by(extension: &str) -> PrinterTuning {
    let peel = PrinterTuning {
        lift_distance_mm: Some(5.5),
        retract_distance_mm: Some(5.5),
        lift_speed_mm_min: Some(60.0),
        retract_speed_mm_min: Some(180.0),
        // The printer rests, so the light-off delay every container states is none.
        light_off_delay_s: Some(0.0),
        ..PrinterTuning::default()
    };
    let bottom = PrinterTuning {
        bottom_lift_distance_mm: Some(6.5),
        bottom_lift_speed_mm_min: Some(120.0),
        light_pwm: Some(211),
        bottom_light_pwm: Some(233),
        ..peel.clone()
    };
    let rests = PrinterTuning {
        transition_layers: Some(2),
        bottom_retract_speed_mm_min: Some(150.0),
        waits: Some(told().waits),
        ..bottom.clone()
    };
    match extension {
        "goo" | "ctb" => rests,
        "cbddlp" => bottom,
        "zip" => PrinterTuning {
            transition_layers: Some(2),
            bottom_retract_speed_mm_min: Some(150.0),
            bottom_light_off_delay_s: Some(0.0),
            ..bottom
        },
        "pwmx" => PrinterTuning {
            transition_layers: Some(2),
            ..peel
        },
        "cws" => peel,
        // The test printer has no buffer, so the block states it off (ADR 0371).
        "cxdlpv4" => PrinterTuning {
            buffer: Some(Buffer::default()),
            ..PrinterTuning::default()
        },
        // Two bottom layers and a ramp of two fade over five; read back, the firmware's one
        // first layer leaves a ramp of three ending on the same layer.
        "sl1" => PrinterTuning {
            transition_layers: Some(3),
            ..PrinterTuning::default()
        },
        _ => PrinterTuning::default(),
    }
}

#[test]
fn every_container_reads_back_the_resin_it_has_a_place_for() {
    for (extension, bytes) in written_with(&told()) {
        let mut source = Cursor::new(bytes);
        let opened = open(Path::new(&format!("plate.{extension}")), &mut source)
            .unwrap_or_else(|error| panic!("a .{extension} we wrote must open: {error}"));
        assert_eq!(
            opened.facts().stated,
            stated_by(extension),
            ".{extension} states back every setting it carries and none it does not"
        );
    }
}

#[test]
fn every_container_covers_the_whole_panel_in_every_layer() {
    let expected = u64::from(WIDTH_PX) * u64::from(HEIGHT_PX);

    for (extension, bytes, _) in written() {
        let name = format!("plate.{extension}");
        let mut source = Cursor::new(bytes);
        let mut opened = open(Path::new(&name), &mut source).expect("it opens");

        for index in 0..LAYERS {
            let runs = opened
                .layer(index)
                .unwrap_or_else(|error| panic!(".{extension} layer {index}: {error}"));
            let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
            assert_eq!(
                covered, expected,
                ".{extension} layer {index} must cover every pixel"
            );
        }
    }
}

#[test]
fn the_lit_area_survives_every_container_and_the_grey_follows_its_depth() {
    // The mask is 48 by 24 lit, of which the one-pixel border is grey 120 and the rest is
    // full. Only a container that cannot carry 120 at all loses the border.
    let lit = 48 * 24u32;

    for (extension, bytes, grey_steps) in written() {
        let name = format!("plate.{extension}");
        let mut source = Cursor::new(bytes);
        let mut opened = open(Path::new(&name), &mut source).expect("it opens");
        let runs = opened.layer(0).expect("the first layer");

        let found: u32 = runs
            .iter()
            .filter(|run| run.value > 0)
            .map(|run| run.length)
            .sum();
        assert_eq!(found, lit, ".{extension} must keep every lit pixel");

        let greys: std::collections::BTreeSet<u8> = runs
            .iter()
            .map(|run| run.value)
            .filter(|v| *v > 0)
            .collect();
        assert_eq!(
            greys.len(),
            2,
            ".{extension} carries the border and the middle as two different greys, got {greys:?}"
        );
        let border = *greys.iter().next().expect("two greys");
        let step = 256 / u32::from(grey_steps);
        assert!(
            u32::from(border.abs_diff(120)) <= step,
            ".{extension} puts grey 120 within one step of {step} of itself, at {border}"
        );
    }
}

#[test]
fn a_container_is_recognised_by_its_bytes_when_the_name_does_not_say() {
    for (extension, bytes, _) in written() {
        let mut source = Cursor::new(bytes);
        let opened = open(Path::new("plate.bin"), &mut source)
            .unwrap_or_else(|error| panic!("a .{extension} names itself in its bytes: {error}"));
        assert_eq!(opened.facts().layer_count(), LAYERS);
    }
}

/// The vector container, which carries no grey at all: a path is filled or it is not, so the
/// border of grey 120 is below the cut and the block inside it comes back whole.
#[test]
fn the_vector_container_keeps_what_it_can_carry() {
    let bytes = write_with(&format_svgx::SvgxWriter);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes as well"
    );

    let mut source = Cursor::new(bytes);
    let mut opened = open(Path::new("plate.svgx"), &mut source).expect("an .svgx we wrote opens");

    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    assert!(
        (facts.exposure_s - 2.5).abs() < 1e-3,
        "{}",
        facts.exposure_s
    );

    let runs = opened.layer(0).expect("the first layer");
    let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
    assert_eq!(covered, u64::from(WIDTH_PX) * u64::from(HEIGHT_PX));

    let greys: std::collections::BTreeSet<u8> = runs
        .iter()
        .map(|run| run.value)
        .filter(|v| *v > 0)
        .collect();
    assert_eq!(
        greys.len(),
        1,
        "one grey, which is full exposure: {greys:?}"
    );

    // The 48 by 24 block less the one-pixel border the container cannot hold.
    let lit: u32 = runs
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 46 * 22, "the border falls below the cut");
}

#[test]
fn the_older_chitu_container_is_named_as_the_file_was_opened() {
    let bytes = write_with(&format_chitu::CbddlpWriter::new(
        format_chitu::CbddlpFlavour::Photon,
    ));
    for (name, format) in [
        ("plate.photon", "photon"),
        ("plate.cbddlp", "cbddlp"),
        // Renamed, the bytes alone name the family and `.cbddlp` is what they are.
        ("plate.bin", "cbddlp"),
    ] {
        let mut source = Cursor::new(bytes.clone());
        let opened = open(Path::new(name), &mut source).expect("the file opens");
        assert_eq!(
            opened.facts().format,
            format,
            "{name} must not be reported under another extension"
        );
    }
}

/// The one-bit container: the border of grey 120 is below its cut, and the resin comes
/// back as `.ctb` states it.
#[test]
fn the_one_bit_container_keeps_the_lit_area_and_the_resin() {
    let job = PrintJob {
        material: told(),
        ..job()
    };
    let bytes = write_job(&format_chitu::FdgWriter, &job);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes"
    );

    let mut source = Cursor::new(bytes);
    let mut opened = open(Path::new("plate.fdg"), &mut source).expect("an .fdg we wrote opens");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    assert_eq!(facts.stated, stated_by("ctb"));

    let runs = opened.layer(0).expect("the first layer");
    let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
    assert_eq!(covered, u64::from(WIDTH_PX) * u64::from(HEIGHT_PX));
    let lit: u32 = runs
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 46 * 22, "the border falls below the cut");
}

/// The Photon S's container: one bit a pixel, and of the resin only its peel and wait.
#[test]
fn the_photon_s_container_keeps_the_lit_area_and_the_peel() {
    let job = PrintJob {
        material: told(),
        ..job()
    };
    let bytes = write_job(&format_anycubic::PhotonsWriter, &job);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes"
    );

    let mut source = Cursor::new(bytes);
    let mut opened =
        open(Path::new("plate.photons"), &mut source).expect("a .photons we wrote opens");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    assert_eq!(facts.stated, stated_by("cws"), "the same peel as the .cws");

    let runs = opened.layer(0).expect("the first layer");
    let lit: u32 = runs
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 46 * 22, "the border falls below the cut");
}

/// The Sonic Mini's container: the `.fdg` layout under another header, the resin as `.ctb`
/// states it.
#[test]
fn the_sonic_mini_container_keeps_the_lit_area_and_the_resin() {
    let job = PrintJob {
        material: told(),
        ..job()
    };
    let bytes = write_job(&format_chitu::PhzWriter, &job);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes"
    );

    let mut source = Cursor::new(bytes);
    let mut opened = open(Path::new("plate.phz"), &mut source).expect("a .phz we wrote opens");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    assert_eq!(facts.stated, stated_by("ctb"));

    let runs = opened.layer(0).expect("the first layer");
    let lit: u32 = runs
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 46 * 22, "the border falls below the cut");
}

/// The D158's container: polygons traced out of the mask, so the border of grey 120 is below
/// the cut and the block inside it comes back whole; the light is stated as it was told.
#[test]
fn the_polygon_container_of_the_creality_family_keeps_the_lit_area() {
    let job = PrintJob {
        material: told(),
        ..job()
    };
    let bytes = write_job(&format_creality::CxlineWriter, &job);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes"
    );

    let mut source = Cursor::new(bytes);
    let mut opened =
        open(Path::new("plate.cxline"), &mut source).expect("a .cxline we wrote opens");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    let told = stated_by("goo");
    assert_eq!(
        (facts.stated.light_pwm, facts.stated.bottom_light_pwm),
        (told.light_pwm, told.bottom_light_pwm)
    );

    let runs = opened.layer(0).expect("the first layer");
    let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
    assert_eq!(covered, u64::from(WIDTH_PX) * u64::from(HEIGHT_PX));
    let lit: u32 = runs
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 46 * 22, "the border falls below the cut");
}

#[test]
fn a_renamed_container_opens_under_the_extension_its_machines_read() {
    for (name, bytes) in [
        ("plate.prz", write_with(&format_goo::GooWriter)),
        (
            "plate.crmaslice",
            write_with(&format_gcode_zip::GcodeZipWriter),
        ),
    ] {
        let mut source = Cursor::new(bytes);
        let opened = open(Path::new(name), &mut source)
            .unwrap_or_else(|error| panic!("{name} is the same bytes under another name: {error}"));
        assert_eq!(opened.facts().layer_count(), LAYERS, "{name}");
    }
}

#[test]
fn an_anycubic_file_names_the_machine_back_at_every_revision() {
    for version in [
        format_anycubic::AnycubicVersion::V1,
        format_anycubic::AnycubicVersion::V516,
    ] {
        let bytes = write_with(&format_anycubic::AnycubicWriter::new(
            format_anycubic::AnycubicFlavour::Pwmx,
            version,
        ));
        let mut source = Cursor::new(bytes);
        let opened = open(Path::new("plate.pwmx"), &mut source).expect("a .pwmx we wrote opens");
        assert_eq!(
            opened.facts().machine.as_deref(),
            Some(printer().machine_name()),
            "{version:?} carries the machine block, as the vendor's slicer writes every one"
        );
    }
}
