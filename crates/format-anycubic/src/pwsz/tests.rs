use std::collections::BTreeSet;
use std::io::{Cursor, Read};

use core_format::{FormatError, LayerSink, SlicedFileWriter};
use core_raster::LayerRuns;
use serde_json::Value;
use zip::ZipArchive;

use super::*;
use crate::decode;
use crate::fixtures::sample_job;

/// An 8 x 4 layer lit where `lit` says, at full brightness.
fn layer(lit: impl Fn(u32, u32) -> bool) -> LayerRuns {
    let mut builder = LayerRuns::builder(8, 4);
    for y in 0..4 {
        for x in 0..8 {
            builder.push(1, if lit(x, y) { 255 } else { 0 });
        }
    }
    builder.finish()
}

/// Columns 2 to 5 of rows 1 and 2: one piece of eight pixels in the middle of the panel.
fn centre_block(x: u32, y: u32) -> bool {
    (2..6).contains(&x) && (1..3).contains(&y)
}

fn written(flavour: PwszFlavour, layers: &[LayerRuns]) -> ZipArchive<Cursor<Vec<u8>>> {
    let mut buffer = Cursor::new(Vec::new());
    let job = sample_job(layers.len() as u32);
    let mut sink = PwszWriter::new(flavour)
        .begin(&job, &mut buffer)
        .expect("the job is sound");
    for layer in layers {
        sink.push(PwszSink::encode(layer)).expect("the layer fits");
    }
    sink.finish(1000.0).expect("every layer is in");
    ZipArchive::new(Cursor::new(buffer.into_inner())).expect("the archive reads back")
}

fn entry(archive: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .expect("the entry is there")
        .read_to_end(&mut bytes)
        .expect("the entry reads");
    bytes
}

fn json(archive: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str) -> Value {
    serde_json::from_slice(&entry(archive, name)).expect("the entry is JSON")
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

/// Where the first layer's row starts in `scene.slice`: the head, then the `<---` mark and
/// the row count.
const FIRST_SCENE_ROW: usize = 388 + 8;

#[test]
fn every_extension_round_trips_through_its_own_name() {
    for flavour in PwszFlavour::ALL {
        assert_eq!(
            PwszFlavour::of_extension(flavour.extension()),
            Some(flavour)
        );
    }
    assert_eq!(PwszFlavour::of_extension("PM7"), Some(PwszFlavour::Pm7));
    assert_eq!(PwszFlavour::of_extension("pwmx"), None);
}

#[test]
fn the_archive_holds_the_entries_the_vendor_writes() {
    let archive = written(
        PwszFlavour::Pm7,
        &[layer(centre_block), layer(centre_block)],
    );
    let names: BTreeSet<String> = archive
        .file_names()
        .flatten()
        .map(std::borrow::Cow::into_owned)
        .collect();
    let expected: BTreeSet<String> = [
        "preview_images/preview_0.png",
        "preview_images/preview_1.png",
        "software_info.conf",
        "layers_controller.conf",
        "print_info.json",
        "anycubic_photon_resins.pwsp",
        "scene.slice",
        "calc_layer_volumes.data",
        "layer_images/layer_0.pw0Img",
        "layer_images/layer_1.pw0Img",
    ]
    .map(str::to_owned)
    .into();
    assert_eq!(names, expected);
}

#[test]
fn a_layer_entry_is_the_layer_in_the_pw0_codec() {
    let mut archive = written(PwszFlavour::Pwsz, &[layer(centre_block)]);
    let runs = decode(&entry(&mut archive, "layer_images/layer_0.pw0Img")).expect("PW0 runs");
    let lit: u32 = runs
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 8, "the eight pixels of the block, and no others");
}

#[test]
fn the_scene_states_a_layer_as_one_piece_with_no_extent() {
    let two = |x: u32, y: u32| (x == 0 && y == 0) || ((4..7).contains(&x) && y == 3);
    let mut archive = written(PwszFlavour::Pwsz, &[layer(two)]);
    let scene = entry(&mut archive, "scene.slice");
    assert_eq!(&scene[..13], b"ANYCUBIC-PWSZ");
    assert_eq!(&scene[388..392], b"<---");
    assert_eq!(u32_at(&scene, 392), 1);

    // Four pixels of 0.01 mm2 in two pieces, stated as the vendor states them.
    let row = FIRST_SCENE_ROW;
    assert!((f32_at(&scene, row) - 0.05).abs() < 1e-5, "the layer's top");
    assert!(
        (f32_at(&scene, row + 4) - 0.04).abs() < 1e-5,
        "the cured area"
    );
    assert!(
        (0..4).all(|i| f32_at(&scene, row + 8 + 4 * i) == 0.0),
        "no extent"
    );
    assert_eq!(u32_at(&scene, row + 24), 1, "one piece");
    assert!(
        (f32_at(&scene, row + 28) - 0.04).abs() < 1e-5,
        "as large as the layer"
    );
    assert_eq!(&scene[row + 64..row + 68], b"--->");
}

#[test]
fn the_volumes_state_each_layer_its_top_and_cured_area() {
    let layers: Vec<_> = (0..3).map(|_| layer(centre_block)).collect();
    let mut archive = written(PwszFlavour::Pwsz, &layers);
    let volumes = entry(&mut archive, "calc_layer_volumes.data");
    assert_eq!(u32_at(&volumes, 0), 0x2025_1024);
    assert_eq!(u32_at(&volumes, 12), 3, "a row per layer");
    assert_eq!(volumes.len(), 16 + 3 * 48);

    let third = 16 + 2 * 48;
    assert_eq!(u32_at(&volumes, third), 3, "rows count from one");
    assert!(
        (f32_at(&volumes, third + 4) - 0.15).abs() < 1e-5,
        "the layer's top"
    );
    assert!(
        (f32_at(&volumes, third + 8) - 0.05).abs() < 1e-5,
        "the layer height"
    );
    let area = f32_at(&volumes, third + 44);
    assert!(
        (area - 0.08).abs() < 1e-6,
        "eight pixels of 0.01 mm2, got {area}"
    );
}

#[test]
fn the_layer_settings_state_every_layer() {
    let mut archive = written(
        PwszFlavour::Pwsz,
        &[layer(centre_block), layer(centre_block)],
    );
    let layers = json(&mut archive, "layers_controller.conf");
    assert_eq!(layers["count"], 2);
    let second = &layers["paras"][1];
    assert_eq!(second["layer_index"], 1);
    let floor = second["layer_minheight"].as_f64().expect("a number");
    assert!(
        (floor - 0.05).abs() < 1e-6,
        "the second layer starts on the first"
    );
}

#[test]
fn the_machine_settings_keep_the_suffix_of_the_vendor_template() {
    let mut archive = written(PwszFlavour::Pm7m, &[layer(centre_block)]);
    let machine = json(&mut archive, "anycubic_photon_resins.pwsp");
    let kind = &machine["machine_type"];
    assert_eq!(
        kind["key_suffix"], "pwsz",
        "the M7 Pro template serves the M7 Max"
    );
    assert_eq!(kind["key_image_format"], "pwszImg");
    assert_eq!(
        (kind["res_x"].clone(), kind["res_y"].clone()),
        (8.into(), 4.into())
    );
    assert!(
        kind.get("rotate_z").is_none(),
        "only the P1 Max turns its plate"
    );

    let mut archive = written(PwszFlavour::Pp1m, &[layer(centre_block)]);
    let machine = json(&mut archive, "anycubic_photon_resins.pwsp");
    assert_eq!(machine["machine_type"]["rotate_z"], 180.0);
    assert_eq!(machine["machine_type"]["key_suffix"], "pp1m");
    let resin = &machine["machine_extern"]["user_resins"][0]["slicepara"];
    assert!(
        resin.get("bott_time_dual").is_some(),
        "its template's second bottom time"
    );
}

#[test]
fn a_job_without_layers_is_rejected_before_anything_is_written() {
    let mut buffer = Cursor::new(Vec::new());
    let err = PwszWriter::default()
        .begin(&sample_job(0), &mut buffer)
        .err()
        .expect("an empty job cannot be written");
    assert!(matches!(err, FormatError::EmptyJob));
}

#[test]
fn a_layer_of_the_wrong_size_is_rejected() {
    let mut buffer = Cursor::new(Vec::new());
    let mut sink = PwszWriter::default()
        .begin(&sample_job(1), &mut buffer)
        .expect("the job is sound");
    let err = sink
        .push(PwszSink::encode(&LayerRuns::builder(4, 4).finish()))
        .expect_err("a 4 x 4 layer does not fit an 8 x 4 panel");
    assert!(matches!(
        err,
        FormatError::ResolutionMismatch { index: 0, .. }
    ));
}

#[test]
fn a_layer_past_the_plan_is_rejected() {
    let mut buffer = Cursor::new(Vec::new());
    let mut sink = PwszWriter::default()
        .begin(&sample_job(1), &mut buffer)
        .expect("the job is sound");
    sink.push(PwszSink::encode(&layer(centre_block)))
        .expect("the one planned layer fits");
    let err = sink
        .push(PwszSink::encode(&layer(centre_block)))
        .expect_err("a second layer is one too many");
    assert!(matches!(
        err,
        FormatError::LayerCountMismatch {
            expected: 1,
            written: 2
        }
    ));
}

#[test]
fn finishing_short_of_the_plan_is_rejected() {
    let mut buffer = Cursor::new(Vec::new());
    let sink = PwszWriter::default()
        .begin(&sample_job(2), &mut buffer)
        .expect("the job is sound");
    let err = sink.finish(0.0).expect_err("no layer was written");
    assert!(matches!(
        err,
        FormatError::LayerCountMismatch {
            expected: 2,
            written: 0
        }
    ));
}

#[test]
fn every_shipped_machine_that_reads_this_archive_can_be_written() {
    let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut found = 0;
    for entry in catalogue.printers() {
        let printer_profiles::OutputFormat::Pwsz { extension } = entry.profile.output else {
            continue;
        };
        found += 1;
        let flavour = PwszFlavour::of_extension(&extension.label()[1..])
            .expect("every extension a profile can state is written");
        let mut job = sample_job(1);
        job.printer = entry.profile.clone();
        job.raster.width_px = entry.profile.display.width_px;
        job.raster.height_px = entry.profile.display.height_px;

        let mut file = Cursor::new(Vec::new());
        let mut sink = PwszWriter::new(flavour)
            .begin(&job, &mut file)
            .unwrap_or_else(|error| panic!("{} cannot be written: {error}", entry.id));
        let blank = LayerRuns::builder(job.raster.width_px, job.raster.height_px).finish();
        sink.push(PwszSink::encode(&blank))
            .expect("a blank layer fits");
        sink.finish(0.0).expect("the one layer is in");
    }
    assert_eq!(found, 5, "the M7, M7 Pro, M7 Max, Mono 4 Ultra and P1 Max");
}

#[test]
fn the_lift_is_stated_as_two_stages_with_the_second_climbing_nothing() {
    let mut archive = written(PwszFlavour::Pwsz, &[layer(centre_block)]);
    let machine = json(&mut archive, "anycubic_photon_resins.pwsp");
    let resin = &machine["machine_extern"]["user_resins"][0];
    let (extra, single) = (&resin["slice_extpara"], &resin["slicepara"]);
    assert_eq!(
        extra["multi_state_used"], 1,
        "as the vendor's slicer states it"
    );
    let stages = &extra["multi_state_paras"];
    assert_eq!(stages["normal_0"]["height"], single["zup_height"]);
    assert_eq!(stages["normal_1"]["height"], 0.0);
    for stage in ["normal_0", "normal_1"] {
        assert_eq!(stages[stage]["down_speed"], single["zdown_speed"]);
        assert_eq!(stages[stage]["up_speed"], single["zup_speed"]);
    }
    assert_eq!(single["gray_level"], single["blur_level"]);
}

#[test]
fn an_archive_reads_back_what_it_was_told() {
    let archive = written(
        PwszFlavour::Pwsz,
        &[layer(centre_block), layer(|_, _| false)],
    );
    let bytes = archive.into_inner().into_inner();
    let mut open = core_format::SlicedFileReader::open(&PwszReader, Cursor::new(bytes))
        .expect("the archive opens");
    let job = sample_job(2);
    let facts = core_format::OpenFile::facts(&open).clone();
    assert_eq!((facts.width_px, facts.height_px), (8, 4));
    assert_eq!(facts.layer_count(), 2);
    assert_eq!(facts.bottom_layers, job.material.bottom_layers);
    assert!((facts.exposure_s - job.material.exposure_s).abs() < 1e-5);
    assert!(
        (facts.layers[1].z_mm - job.layer_z_mm(1)).abs() < 1e-5,
        "a layer's top"
    );

    let lit: u32 = core_format::OpenFile::layer(&mut open, 0)
        .expect("the first layer")
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 8);
}

#[test]
fn an_archive_without_its_settings_is_refused_by_name() {
    let mut buffer = Cursor::new(Vec::new());
    zip::ZipWriter::new(&mut buffer)
        .finish()
        .expect("an empty archive");
    let error = core_format::SlicedFileReader::open(&PwszReader, Cursor::new(buffer.into_inner()))
        .err()
        .expect("nothing to read");
    assert!(matches!(error, FormatError::Missing { .. }));
}

/// The archive `written` makes of one blank layer, with `name` holding `bytes` instead.
fn with_entry(name: &str, bytes: &[u8]) -> Cursor<Vec<u8>> {
    let mut source = written(PwszFlavour::Pwsz, &[layer(|_, _| false)]);
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        for index in 0..source.len() {
            let mut file = source.by_index(index).expect("listed");
            let entry_name = file.name().expect("named").into_owned();
            zip.start_file(entry_name.as_str(), options)
                .expect("writable");
            if entry_name == name {
                std::io::Write::write_all(&mut zip, bytes).expect("writable");
            } else {
                std::io::copy(&mut file, &mut zip).expect("copied");
            }
        }
        zip.finish().expect("closed");
    }
    Cursor::new(buffer.into_inner())
}

#[test]
fn a_broken_print_info_is_refused_rather_than_read_as_none() {
    let error =
        core_format::SlicedFileReader::open(&PwszReader, with_entry("print_info.json", b"{"))
            .err()
            .expect("the entry is not JSON");
    assert!(matches!(error, FormatError::Encoding { .. }), "{error:?}");
}

#[test]
fn a_panel_past_what_a_reader_holds_is_refused() {
    let mut settings = json(
        &mut written(PwszFlavour::Pwsz, &[layer(|_, _| false)]),
        "anycubic_photon_resins.pwsp",
    );
    settings["machine_type"]["res_x"] = Value::from(u64::from(u32::MAX) + 1);
    let bytes = serde_json::to_vec(&settings).expect("serialisable");
    let error = core_format::SlicedFileReader::open(
        &PwszReader,
        with_entry("anycubic_photon_resins.pwsp", &bytes),
    )
    .err()
    .expect("no panel is that wide");
    assert!(
        matches!(error, FormatError::PanelTooLarge { .. }),
        "{error:?}"
    );
}
