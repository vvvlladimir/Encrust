//! Writes a `.cxdlp` at both revisions and reads each back through the public reader.
//! Version 3 has no table of offsets, so its reader has to walk the layers to find any of
//! them; version 4 has one and addresses the motion block in front of each layer.
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
use format_creality::{
    CxdlpReader, CxdlpSink, CxdlpV4Reader, CxdlpV4Sink, CxdlpV4Writer, CxdlpWriter,
};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 24;
const HEIGHT_PX: u32 = 12;

fn printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(
        r#"
name = "Halot Test CL-89"
manufacturer = "Creality"

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
            exposure_s: 2.6,
            bottom_exposure_s: 32.0,
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

/// A block of full exposure with a grey border, which is the shape whose edge a container
/// either keeps or quantises.
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
        let mut sink = CxdlpWriter
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(CxdlpSink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

#[test]
fn the_file_is_headed_and_tailed_by_the_magic_with_the_checksum_behind_it() {
    let file = write(&job(3), &[layer(), layer(), layer()], 1234.0);

    assert_eq!(&file[4..13], b"CXSW3DV2\0");
    assert_eq!(u16::from_be_bytes([file[13], file[14]]), 3, "version");

    let footer = file.len() - 4 - 13;
    assert_eq!(
        &file[footer..file.len() - 4],
        [&9u32.to_be_bytes()[..], b"CXSW3DV2\0"].concat(),
        "the footer is the magic again, with the four checksum bytes behind it"
    );
}

#[test]
fn the_checksum_covers_every_byte_including_the_area_table_filled_in_last() {
    let file = write(&job(4), &[layer(), layer(), layer(), layer()], 500.0);
    let stated = u32::from_be_bytes(
        file[file.len() - 4..]
            .try_into()
            .expect("four bytes of checksum"),
    );

    // The same reflected CRC-32 the firmware takes: no inverted register, no final xor.
    let mut table = [0u32; 256];
    for (index, cell) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 == 0 {
                value >> 1
            } else {
                (value >> 1) ^ 0xEDB8_8320
            };
        }
        *cell = value;
    }
    let mut expected = 0u32;
    for byte in &file[..file.len() - 4] {
        expected = table[((expected as u8) ^ byte) as usize] ^ (expected >> 8);
    }

    assert_eq!(
        stated, expected,
        "a wrong checksum is what a machine refuses the file for"
    );
}

#[test]
fn every_layer_reads_back_the_pixels_it_went_in_as() {
    let runs = layer();
    let file = write(&job(3), &[runs.clone(), runs.clone(), runs.clone()], 900.0);
    let mut source = Cursor::new(file);
    let mut opened = CxdlpReader
        .open(&mut source)
        .expect("what we wrote is one of these");

    let facts = opened.facts();
    assert_eq!(facts.layer_count(), 3);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.version, Some(3));
    assert_eq!(facts.machine.as_deref(), Some("CL-89"));
    assert_eq!(facts.resin.as_deref(), Some("Test resin"));
    assert!((facts.layer_height_mm - 0.05).abs() < 1e-6);
    assert!(
        (facts.exposure_s - 2.6).abs() < 1e-6,
        "{}",
        facts.exposure_s
    );
    assert!((facts.bottom_exposure_s - 32.0).abs() < 1e-6);
    assert_eq!(facts.bottom_layers, 2);
    assert_eq!(facts.display_mm, Some((2.4, 1.2)));

    for index in 0..3 {
        assert_eq!(
            opened.layer(index).expect("the layer reads back"),
            runs.runs(),
            "layer {index} carries all eight bits of every grey"
        );
    }
}

#[test]
fn a_blank_stack_is_a_file_with_no_lines_in_it() {
    let blank: Vec<_> = (0..2)
        .map(|_| LayerRuns::builder(WIDTH_PX, HEIGHT_PX).finish())
        .collect();
    let file = write(&job(2), &blank, 0.0);
    let mut source = Cursor::new(file);
    let mut opened = CxdlpReader.open(&mut source).expect("it still opens");

    assert_eq!(opened.facts().layer_count(), 2);
    let runs = opened.layer(1).expect("a blank layer reads back");
    assert_eq!(runs.len(), 1, "one dark run over the whole panel");
    assert_eq!(runs[0].value, 0);
}

#[test]
fn every_shipped_machine_that_reads_this_container_has_a_model_code_in_its_name() {
    let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut found = 0;

    for entry in catalogue.printers() {
        if entry.profile.output != printer_profiles::OutputFormat::Cxdlp3 {
            continue;
        }
        found += 1;
        let mut job = job(1);
        job.printer = entry.profile.clone();
        job.raster.width_px = entry.profile.display.width_px;
        job.raster.height_px = entry.profile.display.height_px;

        // The header has to carry the machine's own CL or CT code, so a profile whose name
        // does not hold one cannot be written at all.
        let mut file = Cursor::new(Vec::new());
        CxdlpWriter
            .begin(&job, &mut file)
            .unwrap_or_else(|error| panic!("{} cannot be written: {error}", entry.id));
    }
    assert!(found >= 10, "only {found} machines name this container");
}

/// The same stack through version 4, whose writer takes a second pass over the blocks in
/// front of the layers once the resin volume is known.
fn write_v4(job: &PrintJob, layers: &[LayerRuns], volume_mm3: f32) -> Vec<u8> {
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = CxdlpV4Writer
            .begin(job, &mut file)
            .expect("the job matches the panel");
        for layer in layers {
            sink.push(CxdlpV4Sink::encode(layer))
                .expect("a layer is written");
        }
        sink.finish(volume_mm3)
            .expect("every promised layer arrived");
    }
    file.into_inner()
}

/// The reflected CRC-32 both revisions end with: no inverted register, no final xor.
fn checksum(bytes: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (index, cell) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 == 0 {
                value >> 1
            } else {
                (value >> 1) ^ 0xEDB8_8320
            };
        }
        *cell = value;
    }
    bytes.iter().fold(0u32, |state, byte| {
        table[((state as u8) ^ byte) as usize] ^ (state >> 8)
    })
}

#[test]
fn a_version_four_file_is_headed_by_the_same_magic_at_the_later_revision() {
    let file = write_v4(&job(3), &[layer(), layer(), layer()], 1234.0);

    assert_eq!(&file[4..13], b"CXSW3DV2\0");
    assert_eq!(u16::from_be_bytes([file[13], file[14]]), 4, "version");

    let stated = u32::from_be_bytes(
        file[file.len() - 4..]
            .try_into()
            .expect("four bytes of checksum"),
    );
    assert_eq!(
        stated,
        checksum(&file[..file.len() - 4]),
        "the sum covers the header and the table, both written after the layers"
    );
}

#[test]
fn every_version_four_layer_reads_back_the_pixels_it_went_in_as() {
    let runs = layer();
    let file = write_v4(&job(3), &[runs.clone(), runs.clone(), runs.clone()], 900.0);
    let mut source = Cursor::new(file);
    let mut opened = CxdlpV4Reader
        .open(&mut source)
        .expect("what we wrote is one of these");

    let facts = opened.facts();
    assert_eq!(facts.version, Some(4));
    assert_eq!(facts.layer_count(), 3);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.machine.as_deref(), Some("CL-89"));
    assert_eq!(facts.bottom_layers, 2);
    assert!(
        (facts.exposure_s - 2.6).abs() < 1e-6,
        "{}",
        facts.exposure_s
    );
    assert!((facts.bottom_exposure_s - 32.0).abs() < 1e-6);
    assert_eq!(facts.volume_mm3, Some(900.0));
    assert!((facts.layer_height_mm - 0.05).abs() < 1e-6);

    // Seven bits of grey a pixel, so a layer comes back with its lowest bit set rather
    // than as it went in; see docs/formats/creality.md.
    let expected: Vec<u8> = runs
        .runs()
        .iter()
        .flat_map(|run| {
            let value = if run.value == 0 {
                0
            } else {
                (run.value >> 1) << 1 | 1
            };
            std::iter::repeat_n(value, run.length as usize)
        })
        .collect();
    for index in 0..3 {
        let read: Vec<u8> = opened
            .layer(index)
            .expect("the layer reads back")
            .iter()
            .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
            .collect();
        assert_eq!(read, expected, "layer {index}");
    }
}

#[test]
fn the_two_revisions_are_told_apart_by_the_field_behind_the_magic() {
    let three = write(&job(2), &[layer(), layer()], 10.0);
    let four = write_v4(&job(2), &[layer(), layer()], 10.0);

    assert!(CxdlpV4Reader.open(&mut Cursor::new(three)).is_err());
    assert!(CxdlpReader.open(&mut Cursor::new(four)).is_err());
}
