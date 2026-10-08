//! Written containers: what a `.goo` and a `.ctb` carry, reading one back with `info`,
//! and writing it again with `convert`.

use std::fs;
use std::path::Path;

use crate::support::*;

fn be_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn be_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Walks the layer contents of a `.goo` file, returning each layer's exposure and pixels.
///
/// Field offsets come from `docs/formats/goo.md`.
fn goo_layers(file: &[u8]) -> Vec<(f32, Vec<u8>)> {
    const HEADER_SIZE: usize = 0x0002_FB95;
    const DEFINITION_SIZE: usize = 2 + 15 * 4 + 2 + 2;

    let layer_count = be_u32(file, 195_310) as usize;
    let mut layers = Vec::with_capacity(layer_count);
    let mut cursor = HEADER_SIZE;

    for _ in 0..layer_count {
        let exposure = be_f32(file, cursor + 10);
        cursor += DEFINITION_SIZE;

        let size = be_u32(file, cursor) as usize;
        cursor += 4;
        assert_eq!(file[cursor], 0x55, "a layer must open with the magic byte");

        let data = &file[cursor + 1..cursor + size - 1];
        assert_eq!(
            file[cursor + size - 1],
            format_goo::checksum(data),
            "the checksum must be the negated byte sum"
        );
        cursor += size + 2;

        let pixels = format_goo::decode(data)
            .expect("the file we just wrote decodes")
            .iter()
            .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
            .collect();
        layers.push((exposure, pixels));
    }

    assert_eq!(
        cursor + 11,
        file.len(),
        "nothing may follow the ending string"
    );
    layers
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn le_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Walks the layer table of a `.ctb` file, returning each layer's Z, exposure and pixels.
///
/// Field offsets come from `docs/formats/chitu.md`.
fn ctb_layers(file: &[u8]) -> Vec<(f32, f32, Vec<u8>)> {
    const LAYER_DEF_BYTES: usize = 36;
    const LAYER_DEF_EX_BYTES: u32 = 84;

    let table = le_u32(file, 0x40) as usize;
    let layer_count = le_u32(file, 0x44) as usize;
    let mut layers = Vec::with_capacity(layer_count);

    for index in 0..layer_count {
        let row = table + index * LAYER_DEF_BYTES;
        let address = le_u32(file, row + 12) as usize;
        let size = le_u32(file, row + 16) as usize;
        assert_eq!(
            le_u32(file, row + 24),
            LAYER_DEF_EX_BYTES,
            "layer {index} must declare the extended block in front of its data"
        );

        // The extended block repeats the row and states the bytes it covers with the data.
        let extended = address - LAYER_DEF_EX_BYTES as usize;
        assert_eq!(
            &file[extended..extended + LAYER_DEF_BYTES],
            &file[row..row + LAYER_DEF_BYTES],
            "layer {index} must repeat its table row in front of its data"
        );
        assert_eq!(
            le_u32(file, extended + LAYER_DEF_BYTES) as usize,
            LAYER_DEF_EX_BYTES as usize + size,
            "layer {index} must state the size of its own block"
        );

        let pixels = core_format::decode_rle7(&file[address..address + size])
            .expect("the file we just wrote decodes")
            .iter()
            .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
            .collect();
        layers.push((le_f32(file, row), le_f32(file, row + 4), pixels));
    }
    layers
}

#[test]
fn a_cube_becomes_a_printable_goo_file() {
    let path = write_box_stl("goo-cube", 10.0, 12, 0);
    let out = output_file("cube.goo");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--material",
        test_resin().to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);
    assert!(output.status.success(), "{text}");
    assert_eq!(
        field(&text, "masks"),
        format!("10 written to {}", out.display())
    );

    let file = fs::read(&out).expect("the goo file was written");
    assert_eq!(&file[..4], b"V3.0");
    assert_eq!(
        &file[4..12],
        [0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00]
    );
    assert_eq!(
        &file[file.len() - 11..],
        [
            0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00
        ]
    );
    assert_eq!(be_u32(&file, 195_310), 10);
    // The header records where layer content starts, seven bytes before its own end.
    assert_eq!(be_u32(&file, 0x0002_FB95 - 7), 0x0002_FB95);

    let layers = goo_layers(&file);
    assert_eq!(layers.len(), 10);
    for (index, (_, pixels)) in layers.iter().enumerate() {
        assert_eq!(
            pixels.len(),
            200 * 200,
            "layer {index} must define every pixel of the panel"
        );
        assert_eq!(
            pixels.iter().filter(|&&p| p == 255).count(),
            100 * 100,
            "a 10 mm cube at a 0.1 mm pitch exposes 100 x 100 whole pixels"
        );
    }
}

#[test]
fn the_goo_layers_carry_the_bottom_and_transition_exposures() {
    let path = write_box_stl("goo-exposure", 10.0, 12, 0);
    let out = output_file("exposure.goo");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--material",
        test_resin().to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stdout(&output));

    let exposures: Vec<f32> = goo_layers(&fs::read(&out).expect("the goo file was written"))
        .iter()
        .map(|(exposure, _)| *exposure)
        .collect();

    // Six bottom layers at 30 s, then eight transition layers ramping down towards 2.5 s.
    assert!((exposures[0] - 30.0).abs() < 1e-5);
    assert!((exposures[5] - 30.0).abs() < 1e-5);
    assert!(exposures[6] < 30.0 && exposures[6] > 2.5);
    assert!(
        exposures[9] < exposures[6],
        "the ramp keeps falling layer by layer"
    );
}

#[test]
fn a_goo_name_without_a_machine_is_refused_rather_than_left_unwritten() {
    let path = write_box_stl("goo-no-profile", 10.0, 12, 0);
    let out = output_file("no-profile.goo");
    let output = slice(&[path.to_str().unwrap(), "-o", out.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "a run that wrote no file must not report success: {}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("--printer"), "{}", stderr(&output));
    assert!(!out.exists(), "without a panel there is nothing to write");
}

#[test]
fn a_cube_becomes_a_printable_ctb_file() {
    let path = write_box_stl("ctb-cube", 10.0, 12, 0);
    let out = output_file("cube.ctb");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--material",
        test_resin().to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);
    assert!(output.status.success(), "{text}");
    assert_eq!(
        field(&text, "masks"),
        format!("10 written to {}", out.display())
    );

    let file = fs::read(&out).expect("the ctb file was written");
    assert_eq!(le_u32(&file, 0), 0x12FD_0106, "the version 4 magic");
    assert_eq!(le_u32(&file, 4), 4, "the default revision");
    assert_eq!(le_u32(&file, 0x44), 10);
    assert_eq!(le_u32(&file, 0x64), 0, "the layer data is left unencrypted");

    let layers = ctb_layers(&file);
    assert_eq!(layers.len(), 10);
    for (index, (z, _, pixels)) in layers.iter().enumerate() {
        assert!(
            (z - (index + 1) as f32).abs() < 1e-5,
            "layer {index} sits at the top of its own millimetre"
        );
        assert_eq!(
            pixels.len(),
            200 * 200,
            "layer {index} must define every pixel of the panel"
        );
        assert_eq!(
            pixels.iter().filter(|&&p| p == 255).count(),
            100 * 100,
            "a 10 mm cube at a 0.1 mm pitch exposes 100 x 100 whole pixels"
        );
    }
}

#[test]
fn the_ctb_version_flag_picks_the_revision() {
    let path = write_box_stl("ctb-five", 10.0, 12, 0);
    let out = output_file("five.ctb");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--material",
        test_resin().to_str().unwrap(),
        "--layer-height",
        "2",
        "--ctb-version",
        "5",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stdout(&output));

    let file = fs::read(&out).expect("the ctb file was written");
    assert_eq!(le_u32(&file, 0), 0x12FD_0106, "version 5 shares the magic");
    assert_eq!(le_u32(&file, 4), 5);
    assert_eq!(ctb_layers(&file).len(), 5);
}

#[test]
fn a_goo_name_on_a_chitu_machine_is_written_but_reported() {
    let path = write_box_stl("format-mismatch", 10.0, 12, 0);
    let out = output_file("mismatch.goo");
    let output = slice(&[
        path.to_str().unwrap(),
        "--printer",
        "elegoo-mars-3-pro",
        "--material",
        test_resin().to_str().unwrap(),
        "--layer-height",
        "2",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert!(out.exists(), "the extension still decides the writer");

    let warning = stderr(&output);
    assert!(
        warning.contains(".ctb v4") && warning.contains(".goo"),
        "the warning is a log line, so it goes to stderr: {warning}"
    );
}

#[test]
fn adaptive_is_refused_before_the_stack_is_cut_on_a_machine_that_steps_by_the_header() {
    let path = write_box_stl("adaptive-refused", 10.0, 12, 0);
    let out = output_file("adaptive-refused.goo");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "--adaptive",
        "-o",
        out.to_str().unwrap(),
    ]);

    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("variable_layer_height"),
        "the refusal names the profile field that allows it: {}",
        stderr(&output)
    );
    assert!(
        !out.exists(),
        "nothing was written on the way to the refusal"
    );
}

#[test]
fn convert_writes_the_same_masks_into_another_container() {
    let path = write_box_stl("convert-me", 10.0, 12, 0);
    let goo = output_file("convert-me.goo");
    let panel = test_panel();
    let written = slice(&[
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        goo.to_str().unwrap(),
    ]);
    assert!(written.status.success(), "{}", stderr(&written));

    let ctb = output_file("convert-me.ctb");
    let output = encrust(&[
        "convert",
        goo.to_str().unwrap(),
        "-o",
        ctb.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--json",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(json(&output)["layers"].as_u64(), Some(10));

    // The same layer out of either file is the same image.
    let layer = |file: &Path, name: &str| {
        let dir = output_dir(name);
        fs::create_dir_all(&dir).expect("a fresh directory");
        let png = dir.join("layer_0000.png");
        let out = encrust(&[
            "info",
            file.to_str().unwrap(),
            "--layer",
            "5",
            "--png",
            png.to_str().unwrap(),
        ]);
        assert!(out.status.success(), "{}", stderr(&out));
        layer_pixels(&dir, 0)
    };
    assert_eq!(layer(&goo, "convert-goo"), layer(&ctb, "convert-ctb"));
}

#[test]
fn convert_refuses_a_machine_it_cannot_draw_the_masks_for() {
    let path = write_box_stl("convert-refused", 10.0, 12, 0);
    let goo = output_file("convert-refused.goo");
    let panel = test_panel();
    let written = slice(&[
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "2",
        "-o",
        goo.to_str().unwrap(),
    ]);
    assert!(written.status.success(), "{}", stderr(&written));
    let ctb = output_file("convert-refused.ctb");

    let no_printer = encrust(&[
        "convert",
        goo.to_str().unwrap(),
        "-o",
        ctb.to_str().unwrap(),
    ]);
    assert_eq!(no_printer.status.code(), Some(1));
    assert!(
        stderr(&no_printer).contains("--printer"),
        "{}",
        stderr(&no_printer)
    );

    let other_panel = encrust(&[
        "convert",
        goo.to_str().unwrap(),
        "-o",
        ctb.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
    ]);
    assert_eq!(other_panel.status.code(), Some(1));
    assert!(
        stderr(&other_panel).contains("resampled"),
        "{}",
        stderr(&other_panel)
    );
    assert!(!ctb.exists(), "nothing is left behind");
}

#[test]
fn info_takes_one_layer_out_as_a_png() {
    let path = write_box_stl("layer-out", 10.0, 12, 0);
    let goo = output_file("layer-out.goo");
    let written = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "--no-anti-alias",
        "-o",
        goo.to_str().unwrap(),
    ]);
    assert!(written.status.success(), "{}", stderr(&written));

    let dir = output_dir("layer-out-png");
    fs::create_dir_all(&dir).expect("a fresh directory");
    let png = dir.join("layer_0000.png");
    let output = encrust(&[
        "info",
        goo.to_str().unwrap(),
        "--layer",
        "3",
        "--png",
        png.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));

    // A 10 mm cube on a 0.1 mm pitch lights a square 100 pixels a side.
    let (width, height, pixels) = layer_pixels(&dir, 0);
    assert_eq!((width, height), (200, 200), "the whole panel");
    assert_eq!(pixels.iter().filter(|&&grey| grey > 0).count(), 100 * 100);

    let past = encrust(&[
        "info",
        goo.to_str().unwrap(),
        "--layer",
        "99",
        "--png",
        png.to_str().unwrap(),
    ]);
    assert_eq!(past.status.code(), Some(1), "the stack has ten layers");
    assert!(
        stderr(&past).contains("layer 99 was asked for in a file of 10 layers"),
        "the number answered is the number asked for: {}",
        stderr(&past)
    );
}

#[test]
fn info_reports_one_layer_on_its_own_and_every_run_of_exposures() {
    let path = write_box_stl("layer-facts", 10.0, 12, 0);
    let goo = output_file("layer-facts.goo");
    let written = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--material",
        test_resin().to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        goo.to_str().unwrap(),
    ]);
    assert!(written.status.success(), "{}", stderr(&written));

    let one = encrust(&["info", goo.to_str().unwrap(), "--layer", "3"]);
    assert!(one.status.success(), "{}", stderr(&one));
    let text = stdout(&one);
    assert!(text.contains("layer         3 of 10"), "{text}");
    assert!(text.contains("3.000 mm above the plate"), "{text}");
    assert!(text.contains("1.0000 mm thick"), "{text}");
    assert!(text.contains("lit pixels"), "{text}");

    let document = json(&encrust(&[
        "info",
        goo.to_str().unwrap(),
        "--layer",
        "3",
        "--json",
    ]));
    assert_eq!(document["layer"]["layer"], 3);
    assert_eq!(document["layer"]["of"], 10);
    assert!(
        document["layer"]["exposure_s"].as_f64().unwrap_or(0.0) > 0.0,
        "{document}"
    );
    assert!(
        document["layer"].get("png").is_none(),
        "no image was asked for: {document}"
    );

    // The test resin ramps over the eight layers above its six bottom ones, so the last
    // four layers of a ten-layer stack carry an exposure of their own.
    let whole = encrust(&["info", goo.to_str().unwrap()]);
    assert!(whole.status.success(), "{}", stderr(&whole));
    assert!(stdout(&whole).contains("layers 7-10"), "{}", stdout(&whole));

    let document = json(&encrust(&["info", goo.to_str().unwrap(), "--json"]));
    let bands = document["exposure_bands"]
        .as_array()
        .expect("a list of runs");
    assert_eq!(bands.len(), 1, "{document}");
    assert_eq!(bands[0]["from_layer"], 7);
    assert_eq!(bands[0]["to_layer"], 10);
}
