use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Binary STL of an axis-aligned box, written the way an exporter would: three fresh
/// vertices per triangle. `faces` selects which of the twelve triangles to include, and
/// `invert` reverses the winding of the first `invert` of them.
fn write_box_stl(name: &str, size: f32, faces: usize, invert: usize) -> PathBuf {
    let s = size;
    let corners = [
        [0.0, 0.0, 0.0],
        [s, 0.0, 0.0],
        [s, s, 0.0],
        [0.0, s, 0.0],
        [0.0, 0.0, s],
        [s, 0.0, s],
        [s, s, s],
        [0.0, s, s],
    ];
    let mut triangles: Vec<[usize; 3]> = vec![
        [0, 3, 2],
        [0, 2, 1],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    triangles.truncate(faces);
    for triangle in triangles.iter_mut().take(invert) {
        triangle.swap(1, 2);
    }

    let mut bytes = vec![0u8; 80];
    bytes.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    for triangle in &triangles {
        bytes.extend_from_slice(&[0u8; 12]);
        for corner in triangle {
            for value in corners[*corner] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0u8; 2]);
    }

    let dir = std::env::temp_dir().join("encrust-cli-tests");
    fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("{name}.stl"));
    fs::write(&path, bytes).expect("write fixture");
    path
}

/// The profile shipped in the repository, resolved without depending on the test's
/// working directory.
fn shipped_profile() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/profiles/printers/elegoo-mars-4-ultra.toml")
}

/// A 200 x 200 px panel, small enough to rasterise in a test.
fn test_panel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-panel.toml")
}

/// A fresh, empty output directory of its own for each test.
fn output_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("encrust-cli-out-{name}"));
    let _ = fs::remove_dir_all(&path);
    path
}

fn layer_pixels(directory: &Path, index: usize) -> (u32, u32, Vec<u8>) {
    let path = directory.join(format!("layer_{index:04}.png"));
    let file = fs::File::open(&path).unwrap_or_else(|_| panic!("no {}", path.display()));
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .expect("the layer is a PNG");
    let mut pixels = vec![0; reader.output_buffer_size().expect("a bounded image")];
    let info = reader.next_frame(&mut pixels).expect("one frame");
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

fn written_layers(directory: &Path) -> usize {
    fs::read_dir(directory)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

fn slice(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_slice"))
        .args(args)
        .output()
        .expect("the slice binary runs")
}

/// Runs the binary with its own profile directory, so a test never reads the real one.
fn slice_with_profiles(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_slice"))
        .env("ENCRUST_PROFILE_DIR", dir)
        .args(args)
        .output()
        .expect("the slice binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf-8")
}

fn field<'a>(text: &'a str, label: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.trim_start().strip_prefix(label))
        .unwrap_or_else(|| panic!("no {label:?} line in:\n{text}"))
        .trim()
}

#[test]
fn a_cube_is_welded_validated_and_reported() {
    let path = write_box_stl("cube", 10.0, 12, 0);
    let output = slice(&[path.to_str().unwrap()]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert_eq!(field(&text, "vertices"), "8 (welded from 36)");
    assert_eq!(field(&text, "triangles"), "12");
    assert_eq!(field(&text, "size"), "10.000 x 10.000 x 10.000 mm");
    assert_eq!(field(&text, "volume"), "1000.000 mm^3");
    assert_eq!(field(&text, "closed"), "yes (1 shell, euler 2)");
    assert_eq!(field(&text, "orientation"), "consistent");
}

#[test]
fn inverted_faces_are_reported_as_fixed() {
    let path = write_box_stl("inverted", 10.0, 12, 3);
    let text = stdout(&slice(&[path.to_str().unwrap()]));
    assert_eq!(field(&text, "orientation"), "fixed 3 inverted faces");
}

#[test]
fn an_open_mesh_is_reported_but_still_succeeds() {
    let path = write_box_stl("open", 10.0, 10, 0);
    let output = slice(&[path.to_str().unwrap()]);
    let text = stdout(&output);

    assert!(
        output.status.success(),
        "a defect alone must not fail the run"
    );
    assert_eq!(field(&text, "closed"), "no (1 shell, 4 open edges)");
    assert_eq!(field(&text, "defect"), "4 open edges");
    assert!(
        !text
            .lines()
            .any(|line| line.trim_start().starts_with("volume")),
        "an open mesh encloses no defined volume"
    );
}

#[test]
fn a_cube_is_sliced_into_layers_that_add_back_up_to_its_volume() {
    let path = write_box_stl("sliced", 10.0, 12, 0);
    let output = slice(&[path.to_str().unwrap(), "--layer-height", "0.1"]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert_eq!(field(&text, "layers"), "100");
    assert_eq!(field(&text, "contours"), "100 (up to 1 per layer)");
    assert_eq!(field(&text, "sliced volume"), "1000.000 mm^3");
    assert!(!text.contains("slice defect"));
}

#[test]
fn no_slice_stops_after_the_import_report() {
    let path = write_box_stl("no-slice", 10.0, 12, 0);
    let output = slice(&[path.to_str().unwrap(), "--no-slice"]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert!(!text.contains("layers"));
}

#[test]
fn a_missing_wall_is_reported_as_a_slice_defect() {
    // Ten of the twelve faces: the box loses one of its four walls.
    let path = write_box_stl("strict-gap", 10.0, 10, 0);
    let output = slice(&[path.to_str().unwrap(), "--layer-height", "1"]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("slice defect"));
}

#[test]
fn strict_fails_on_an_open_mesh_and_passes_on_a_closed_one() {
    let open = write_box_stl("strict-open", 10.0, 10, 0);
    let closed = write_box_stl("strict-closed", 10.0, 12, 0);

    assert!(
        !slice(&[open.to_str().unwrap(), "--strict"])
            .status
            .success()
    );
    assert!(
        slice(&[closed.to_str().unwrap(), "--strict"])
            .status
            .success()
    );
}

#[test]
fn scale_and_rotation_change_the_reported_size() {
    let path = write_box_stl("transformed", 10.0, 12, 0);
    let scaled = stdout(&slice(&[path.to_str().unwrap(), "--scale", "2,1,0.5"]));
    assert_eq!(field(&scaled, "size"), "20.000 x 10.000 x 5.000 mm");

    let rotated = stdout(&slice(&[path.to_str().unwrap(), "--rotate", "0,0,90"]));
    assert_eq!(field(&rotated, "size"), "10.000 x 10.000 x 10.000 mm");
    assert_eq!(field(&rotated, "volume"), "1000.000 mm^3");
}

#[test]
fn center_sits_the_model_on_the_plate() {
    let path = write_box_stl("centered", 10.0, 12, 0);
    let text = stdout(&slice(&[
        path.to_str().unwrap(),
        "--center",
        "--no-raster",
        "--profile",
        shipped_profile().to_str().unwrap(),
    ]));

    let bounds = field(&text, "bounds");
    assert!(
        bounds.starts_with("[71.680, 33.880, 0.000]"),
        "got {bounds}"
    );
    assert_eq!(field(&text, "fits"), "Mars 4 Ultra: yes");
}

#[test]
fn a_model_larger_than_the_machine_is_reported() {
    let path = write_box_stl("oversized", 200.0, 12, 0);
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
        "--no-raster",
        "--strict",
    ]);

    assert!(!output.status.success());
    assert_eq!(
        field(&stdout(&output), "fits"),
        "Mars 4 Ultra: no, over X by 46.640 mm, Y by 122.240 mm, Z by 35.000 mm"
    );
}

#[test]
fn an_unknown_extension_is_an_error() {
    let output = slice(&["model.gcode"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");

    assert!(!output.status.success());
    assert!(stderr.contains("gcode"), "got {stderr}");
}

#[test]
fn a_cube_becomes_one_greyscale_png_per_layer() {
    let path = write_box_stl("raster-cube", 10.0, 12, 0);
    let out = output_dir("cube");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert_eq!(field(&text, "panel"), "200 x 200 px at 0.1000 x 0.1000 mm");
    assert_eq!(
        field(&text, "shading"),
        "coverage, exact pixel area, all 255 greys down to 128",
        "the floor comes from the profile without being asked for"
    );
    assert_eq!(
        field(&text, "masks"),
        format!("10 written to {}", out.display())
    );
    assert!(!text.contains("raster defect"));
    assert_eq!(written_layers(&out), 10);

    let (width, height, pixels) = layer_pixels(&out, 5);
    assert_eq!((width, height), (200, 200));
    assert_eq!(
        pixels.iter().filter(|&&p| p == 255).count(),
        100 * 100,
        "a 10 mm cube at a 0.1 mm pitch exposes 100 x 100 whole pixels"
    );
}

#[test]
fn the_stack_sits_where_the_model_sits_on_the_plate() {
    let path = write_box_stl("raster-corner", 10.0, 12, 0);
    let out = output_dir("corner");
    slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "-o",
        out.to_str().unwrap(),
    ]);

    // The cube sits at the plate origin, which is the first row the file carries.
    let (_, _, pixels) = layer_pixels(&out, 0);
    assert_eq!(
        pixels[0], 255,
        "the first pixel of the first row is exposed"
    );
    assert_eq!(pixels[199 * 200], 0, "the last row is not");
}

#[test]
fn rounding_and_the_floor_are_asked_for_on_the_command_line() {
    let path = write_box_stl("raster-rungs", 10.0, 12, 0);
    let out = output_dir("rungs");
    let text = stdout(&slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "--grey-levels",
        "4",
        "--grey-floor",
        "0",
        "--rotate",
        "0,0,20",
        "-o",
        out.to_str().unwrap(),
    ]));

    assert_eq!(
        field(&text, "shading"),
        "coverage, exact pixel area, 4 greys"
    );
    let (_, _, pixels) = layer_pixels(&out, 0);
    let rungs = [0, 63, 127, 191, 255];
    assert!(
        pixels.iter().all(|p| rungs.contains(p)),
        "four levels leave only their own rungs"
    );
}

#[test]
fn binary_shading_writes_no_intermediate_grey() {
    let path = write_box_stl("raster-binary", 10.0, 12, 0);
    let out = output_dir("binary");
    let text = stdout(&slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "--no-anti-alias",
        "--rotate",
        "0,0,20",
        "-o",
        out.to_str().unwrap(),
    ]));

    assert_eq!(field(&text, "shading"), "binary, no intermediate grey");
    let (_, _, pixels) = layer_pixels(&out, 0);
    assert!(
        pixels.iter().all(|&p| p == 0 || p == 255),
        "a turned cube has diagonal edges, and none of them may come out grey"
    );
}

#[test]
fn a_blur_widens_the_grey_at_every_edge() {
    let path = write_box_stl("raster-blur", 10.0, 12, 0);
    let grey_pixels = |name: &str, blur: &str| {
        let out = output_dir(name);
        let text = stdout(&slice(&[
            path.to_str().unwrap(),
            "--profile",
            test_panel().to_str().unwrap(),
            "--layer-height",
            "5",
            "--grey-floor",
            "0",
            "--blur",
            blur,
            "-o",
            out.to_str().unwrap(),
        ]));
        let (_, _, pixels) = layer_pixels(&out, 0);
        let grey = pixels.iter().filter(|&&p| p != 0 && p != 255).count();
        (text, grey)
    };

    let (_, sharp) = grey_pixels("blur-off", "0");
    let (text, blurred) = grey_pixels("blur-on", "2");
    assert!(field(&text, "shading").ends_with("blurred 2 px"), "{text}");
    assert!(
        blurred > sharp + 4 * 10,
        "a radius of two greys several more pixels along each side: {sharp} -> {blurred}"
    );
}

#[test]
fn a_model_hanging_off_the_display_is_clipped_and_fails_strict() {
    // The fixture panel is 20 mm wide and its build volume is 40, so this model passes
    // the millimetre fit check and still runs off the display.
    let path = write_box_stl("raster-oversized", 30.0, 12, 0);
    let out = output_dir("clipped");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "--strict",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);

    assert_eq!(field(&text, "fits"), "Test panel: yes");
    assert_eq!(
        field(&text, "raster defect"),
        "6 layers clipped, up to 100.0 px past the panel"
    );
    assert!(
        !output.status.success(),
        "--strict must fail on a layer that does not fit the display"
    );
    assert_eq!(written_layers(&out), 6, "a clipped layer is still written");
}

#[test]
fn no_raster_stops_after_the_slice_report() {
    let path = write_box_stl("no-raster", 10.0, 12, 0);
    let out = output_dir("no-raster");
    let text = stdout(&slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "--no-raster",
        "-o",
        out.to_str().unwrap(),
    ]));

    assert!(text.contains("layers        2"));
    assert!(!text.contains("masks"));
    assert!(!out.exists(), "nothing may be written when raster is off");
}

#[test]
fn without_a_profile_the_panel_is_unknown_and_no_masks_are_written() {
    let path = write_box_stl("no-profile", 10.0, 12, 0);
    let out = output_dir("no-profile");
    let output = slice(&[
        path.to_str().unwrap(),
        "--layer-height",
        "5",
        "-o",
        out.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "the reports are still worth having"
    );
    assert!(!stdout(&output).contains("masks"));
    assert!(!out.exists());
}

/// A fresh output file path of its own for each test.
fn output_file(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("encrust-cli-out-{name}"));
    let _ = fs::remove_file(&path);
    path
}

fn shipped_resin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/profiles/resins/generic-resin.toml")
}

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

#[test]
fn a_cube_becomes_a_printable_goo_file() {
    let path = write_box_stl("goo-cube", 10.0, 12, 0);
    let out = output_file("cube.goo");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--material",
        shipped_resin().to_str().unwrap(),
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
        shipped_resin().to_str().unwrap(),
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
fn a_goo_file_needs_a_printer_profile() {
    let path = write_box_stl("goo-no-profile", 10.0, 12, 0);
    let out = output_file("no-profile.goo");
    let output = slice(&[path.to_str().unwrap(), "-o", out.to_str().unwrap()]);

    assert!(output.status.success());
    assert!(!out.exists(), "without a panel there is nothing to write");
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
fn a_cube_becomes_a_printable_ctb_file() {
    let path = write_box_stl("ctb-cube", 10.0, 12, 0);
    let out = output_file("cube.ctb");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--material",
        shipped_resin().to_str().unwrap(),
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
        shipped_resin().to_str().unwrap(),
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
fn the_catalogue_is_listed_without_a_mesh() {
    let output = slice(&["--list-profiles"]);
    assert!(output.status.success(), "{}", stdout(&output));

    let text = stdout(&output);
    assert!(text.contains("elegoo-mars-4-ultra"), "{text}");
    assert!(text.contains("phrozen-sonic-mini-8k"), "{text}");
    assert!(text.contains("generic-resin"), "{text}");
}

#[test]
fn a_printer_id_replaces_the_profile_path() {
    let path = write_box_stl("by-id", 10.0, 12, 0);
    let output = slice(&[
        path.to_str().unwrap(),
        "--printer",
        "elegoo-mars-3-pro",
        "--no-slice",
    ]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(field(&stdout(&output), "fits"), "Mars 3 Pro: yes");
}

#[test]
fn an_unknown_printer_id_is_refused() {
    let path = write_box_stl("unknown-id", 10.0, 12, 0);
    let output = slice(&[
        path.to_str().unwrap(),
        "--printer",
        "no-such-machine",
        "--no-slice",
    ]);
    assert!(!output.status.success());
    let errors = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert!(errors.contains("no-such-machine"), "{errors}");
}

#[test]
fn a_resin_id_arrives_tuned_for_the_chosen_printer() {
    let path = write_box_stl("tuned-resin", 10.0, 12, 0);
    let out = output_file("tuned.goo");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--resin",
        "standard-grey",
        "--printer",
        "elegoo-mars-3-pro",
        "--layer-height",
        "2",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stdout(&output));

    // The path wins for the panel, so the file is the 200 px test panel, while the
    // exposure is the 3.2 s standard grey was tuned to on a Mars 3 Pro.
    let file = fs::read(&out).expect("the goo file was written");
    let text = String::from_utf8_lossy(&file[0..1000]).to_string();
    assert!(text.contains("Test panel"), "{text}");
}

#[test]
fn a_user_profile_overrides_the_catalogue_by_id() {
    let dir = std::env::temp_dir().join("encrust-cli-user-profiles");
    let printers = dir.join("printers");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&printers).expect("temp dir");
    fs::copy(test_panel(), printers.join("elegoo-mars-4-ultra.toml")).expect("copy the panel");

    let path = write_box_stl("user-override", 10.0, 12, 0);
    let output = slice_with_profiles(
        &dir,
        &[
            path.to_str().unwrap(),
            "--printer",
            "elegoo-mars-4-ultra",
            "--no-slice",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(
        field(&stdout(&output), "fits"),
        "Test panel: yes",
        "the user's file wins over the bundled machine of the same id"
    );

    fs::remove_dir_all(&dir).expect("the temporary directory goes away");
}

#[test]
fn a_goo_name_on_a_chitu_machine_is_written_but_reported() {
    let path = write_box_stl("format-mismatch", 10.0, 12, 0);
    let out = output_file("mismatch.goo");
    let output = slice(&[
        path.to_str().unwrap(),
        "--printer",
        "elegoo-mars-3-pro",
        "--layer-height",
        "2",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert!(out.exists(), "the extension still decides the writer");

    let text = stdout(&output);
    assert!(text.contains(".ctb v4") && text.contains(".goo"), "{text}");
}

/// Pixels the printer would expose on one layer of a stack.
fn lit_pixels(directory: &Path, index: usize) -> usize {
    let (_, _, pixels) = layer_pixels(directory, index);
    pixels.iter().filter(|&&value| value > 0).count()
}

/// A 16 mm cube sliced at 1 mm, with whatever hollowing arguments follow.
fn hollow_cube(name: &str, extra: &[&str]) -> (PathBuf, String) {
    let path = write_box_stl(&format!("hollow-{name}"), 16.0, 12, 0);
    let out = output_dir(name);
    let mut args = vec![
        path.to_str().unwrap().to_owned(),
        "--profile".to_owned(),
        test_panel().to_str().unwrap().to_owned(),
        "--layer-height".to_owned(),
        "1".to_owned(),
        "-o".to_owned(),
        out.to_str().unwrap().to_owned(),
    ];
    args.extend(extra.iter().map(|argument| (*argument).to_owned()));

    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = slice(&borrowed);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (out, stdout(&output))
}

#[test]
fn a_hollowed_cube_exposes_a_ring_on_the_layers_that_cross_its_cavity() {
    let (out, _) = hollow_cube("ring", &["--hollow", "3"]);

    // Halfway up, a 16 mm cube with a 3 mm wall is a 16 mm square with a 10 mm square out
    // of the middle: 156 mm2, which is 15 600 pixels at a 0.1 mm pitch.
    let ring = lit_pixels(&out, 8);
    assert!(
        (ring as f32 - 15_600.0).abs() / 15_600.0 < 0.02,
        "a 16 mm ring around a 10 mm cavity is 15 600 pixels, got {ring}"
    );

    // The floor of an internal hollow is solid, so the first layer is the whole square.
    let floor = lit_pixels(&out, 0);
    assert!(
        (floor as f32 - 25_600.0).abs() / 25_600.0 < 0.02,
        "the floor of an internal hollow is a whole 16 mm square, got {floor}"
    );
}

#[test]
fn bottom_through_takes_the_floor_out_of_the_stack() {
    let (out, _) = hollow_cube(
        "through",
        &["--hollow", "3", "--hollow-mode", "bottom-through"],
    );

    let floor = lit_pixels(&out, 0);
    assert!(
        (floor as f32 - 15_600.0).abs() / 15_600.0 < 0.03,
        "bottom through opens the first layer into the same ring, got {floor}"
    );
}

#[test]
fn hollowing_reports_the_resin_it_saves() {
    let (_, text) = hollow_cube("saved", &["--hollow", "3"]);

    assert_eq!(field(&text, "wall"), "3.000 mm, internal");
    let saved: f32 = field(&text, "resin saved")
        .trim_end_matches(" mm^3")
        .parse()
        .expect("the saved volume is a number");
    assert!(
        (saved - 1000.0).abs() / 1000.0 < 0.03,
        "a 10 mm cube of resin is 1000 mm^3, got {saved}"
    );
}

#[test]
fn infill_puts_material_back_into_the_cavity() {
    let (empty, _) = hollow_cube("empty", &["--hollow", "3"]);
    let (filled, text) = hollow_cube(
        "filled",
        &[
            "--hollow",
            "3",
            "--infill",
            "grid",
            "--infill-size",
            "4",
            "--infill-density",
            "0.3",
        ],
    );

    assert_eq!(
        field(&text, "infill"),
        "grid, 4.0 mm cells at 30%, 0.65 mm walls"
    );
    assert!(
        lit_pixels(&filled, 8) > lit_pixels(&empty, 8),
        "the grid must show up inside the cavity"
    );
}

#[test]
fn a_wall_thicker_than_the_model_is_reported_as_a_defect() {
    let (_, text) = hollow_cube("thick", &["--hollow", "12"]);
    assert!(
        text.contains("the wall left no room for a cavity"),
        "a 12 mm wall in a 16 mm cube hollows nothing:\n{text}"
    );
}

#[test]
fn a_drain_hole_opens_the_lid_of_a_hollow_box() {
    let path = write_box_stl("drained", 20.0, 12, 0);
    let input = path.to_str().unwrap();
    let panel = test_panel();
    let panel = panel.to_str().unwrap();

    let solid_out = output_dir("drain-closed");
    let solid = slice(&[
        input,
        "--profile",
        panel,
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "-o",
        solid_out.to_str().unwrap(),
    ]);
    assert!(solid.status.success(), "{}", stdout(&solid));

    let drained_out = output_dir("drain-open");
    let drained = slice(&[
        input,
        "--profile",
        panel,
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "--drain",
        "6",
        "--drain-depth",
        "4",
        "--drain-at",
        "10,10,25",
        "-o",
        drained_out.to_str().unwrap(),
    ]);
    let text = stdout(&drained);
    assert!(drained.status.success(), "{text}");
    assert_eq!(field(&text, "drains"), "1 hole(s), 0 channel(s)");

    // The lid of the box, which is solid without the hole and a frame with it. The box
    // stands from the plate's origin, so its middle is the centre of the 200 px panel.
    let top = written_layers(&solid_out) - 2;
    let (width, height, closed) = layer_pixels(&solid_out, top);
    let (_, _, open) = layer_pixels(&drained_out, top);
    let middle = (height as usize / 2) * width as usize + width as usize / 2;

    assert_eq!(
        closed[middle], 0xFF,
        "the lid is solid without a hole in it"
    );
    assert_eq!(open[middle], 0x00, "the hole goes right through the lid");
    let lit = |pixels: &[u8]| pixels.iter().filter(|&&pixel| pixel > 0).count();
    assert!(lit(&open) < lit(&closed), "and takes resin with it");
}

#[test]
fn a_drain_hole_is_cut_into_a_solid_model_too() {
    let path = write_box_stl("solid-drain", 20.0, 12, 0);
    let input = path.to_str().unwrap();
    let panel = test_panel();
    let panel = panel.to_str().unwrap();

    let out = output_dir("solid-drain");
    let drilled = slice(&[
        input,
        "--profile",
        panel,
        "--layer-height",
        "0.5",
        "--drain",
        "6",
        "--drain-depth",
        "4",
        "--drain-at",
        "10,10,25",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&drilled);
    assert!(drilled.status.success(), "{text}");
    assert_eq!(field(&text, "drains"), "1 hole(s), 0 channel(s)");

    // No --hollow anywhere: the hole is cut into the solid all the same, four millimetres
    // down from the lid.
    let top = written_layers(&out) - 2;
    let (width, height, lid) = layer_pixels(&out, top);
    let middle = (height as usize / 2) * width as usize + width as usize / 2;
    assert_eq!(lid[middle], 0x00, "the hole is open at the top of the box");

    let floor = written_layers(&out) / 2;
    let (_, _, half_way) = layer_pixels(&out, floor);
    assert_eq!(
        half_way[middle], 0xFF,
        "and stops at its own depth rather than going through the box"
    );
}

#[test]
fn a_hollow_box_with_no_hole_reports_the_resin_it_traps() {
    let path = write_box_stl("trapped", 20.0, 12, 0);
    let input = path.to_str().unwrap();
    let sealed = stdout(&slice(&[
        input,
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "--no-raster",
    ]));

    let reported = field(&sealed, "trapped resin");
    let held: f32 = reported
        .split_whitespace()
        .next()
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("no volume in {reported:?}"));
    let cavity: f32 = field(&sealed, "resin saved")
        .split_whitespace()
        .next()
        .and_then(|number| number.parse().ok())
        .expect("the hollow report states what it took out");
    assert!(
        (held - cavity).abs() / cavity < 0.05,
        "the pocket holds the cavity the run cut: {held} against {cavity}"
    );

    let drained = stdout(&slice(&[
        input,
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "--no-raster",
        "--drain",
        "4",
        "--drain-at",
        "10,10,20",
        "--drain-depth",
        "3",
    ]));
    assert!(
        !drained.contains("trapped resin"),
        "a hole through the lid lets it all out:\n{drained}"
    );
}

#[test]
fn trapped_resin_fails_a_strict_run() {
    let path = write_box_stl("trapped-strict", 20.0, 12, 0);
    let output = slice(&[
        path.to_str().unwrap(),
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "--no-raster",
        "--strict",
    ]);

    assert!(
        !output.status.success(),
        "resin that cannot get out is a defect:\n{}",
        stdout(&output)
    );
}

#[test]
fn a_solid_model_is_not_checked_for_drainage_unless_it_is_asked_for() {
    let path = write_box_stl("drainage-flag", 10.0, 12, 0);
    let input = path.to_str().unwrap();

    let quiet = stdout(&slice(&[input, "--layer-height", "1", "--no-raster"]));
    assert!(!quiet.contains("trapped resin"));

    let checked = stdout(&slice(&[
        input,
        "--layer-height",
        "1",
        "--no-raster",
        "--check-drainage",
    ]));
    assert!(
        !checked.contains("trapped resin"),
        "a solid box traps nothing either way"
    );
}

#[test]
fn a_hole_shallower_than_the_wall_still_goes_through_it() {
    let path = write_box_stl("shallow-drain", 20.0, 12, 0);
    let out = output_dir("shallow-drain");
    let panel = test_panel();
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        // Half a millimetre into a two millimetre wall, drilled into the side of the box.
        "--drain",
        "4",
        "--drain-depth",
        "0.5",
        "--drain-at",
        "0,10,10",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stdout(&output));

    // The layer through the middle of the hole, along the row the hole is centred on: the
    // box stands from the plate's origin on a 0.1 mm panel.
    let (width, height, pixels) = layer_pixels(&out, 20);
    let row = height as usize / 2;
    let start = row * width as usize;
    let wall: Vec<u8> = pixels[start..start + 30].to_vec();

    assert!(
        wall.iter().all(|pixel| *pixel == 0),
        "the hole clears the whole 2 mm wall in front of it, got {wall:?}"
    );
}

/// A directory with two cubes in it and something that is not a model, for a batch run.
fn batch_input(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-cli-batch-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    for model in ["alpha", "beta"] {
        let cube = write_box_stl(&format!("{name}-{model}"), 10.0, 12, 0);
        fs::copy(&cube, dir.join(format!("{model}.stl"))).expect("copy the fixture in");
    }
    fs::write(dir.join("notes.txt"), b"not a model").expect("write the decoy");
    dir
}

#[test]
fn a_directory_of_models_is_sliced_one_file_and_one_report_each() {
    let input = batch_input("stack");
    let out = output_dir("batch-stack");

    let output = slice(&[
        input.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "-o",
        out.to_str().expect("ascii path"),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));

    for model in ["alpha", "beta"] {
        let sliced = out.join(format!("{model}.goo"));
        assert!(sliced.is_file(), "no {}", sliced.display());
        let report = out.join(format!("{model}.json"));
        let text =
            fs::read_to_string(&report).unwrap_or_else(|_| panic!("no {}", report.display()));
        assert!(text.contains("\"status\": \"ok\""), "{text}");
        assert!(text.contains("\"layers\""), "{text}");
    }
    assert!(
        !out.join("notes.goo").exists(),
        "a text file in the directory is not a model"
    );

    let summary = fs::read_to_string(out.join("batch.json")).expect("the batch report is written");
    assert!(summary.contains("\"models\": 2"), "{summary}");
    assert!(summary.contains("\"ok\": 2"), "{summary}");
    assert!(summary.contains("\"failed\": 0"), "{summary}");
}

#[test]
fn a_model_that_cannot_be_loaded_is_reported_and_the_rest_still_run() {
    let input = batch_input("broken");
    fs::write(input.join("broken.stl"), b"not an stl at all").expect("write the bad model");
    let out = output_dir("batch-broken");

    let output = slice(&[
        input.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "-o",
        out.to_str().expect("ascii path"),
    ]);
    assert!(
        output.status.success(),
        "one bad model does not fail the run without --strict: {}",
        stderr(&output)
    );

    assert!(out.join("alpha.goo").is_file(), "the good ones still wrote");
    let summary = fs::read_to_string(out.join("batch.json")).expect("the batch report is written");
    assert!(summary.contains("\"failed\": 1"), "{summary}");
    assert!(summary.contains("\"ok\": 2"), "{summary}");

    let broken = fs::read_to_string(out.join("broken.json")).expect("the bad model reports too");
    assert!(broken.contains("\"status\": \"failed\""), "{broken}");
    assert!(broken.contains("\"error\""), "{broken}");
}

#[test]
fn strict_fails_the_run_when_a_model_does() {
    let input = batch_input("strict");
    fs::write(input.join("broken.stl"), b"not an stl at all").expect("write the bad model");
    let out = output_dir("batch-strict");

    let output = slice(&[
        input.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "-o",
        out.to_str().expect("ascii path"),
        "--strict",
    ]);
    assert!(
        !output.status.success(),
        "a failed model has to fail --strict"
    );
}

/// Whether a cube needs holding is settled in `supports.rs`; what this pins down is that
/// the flag reaches the pipeline and the run says what it stood.
#[test]
fn asking_for_supports_reports_what_was_stood() {
    let model = write_box_stl("supported", 10.0, 12, 0);
    let held = output_dir("supports-held");

    let with = slice(&[
        model.to_str().expect("ascii path"),
        "--profile",
        test_panel().to_str().expect("ascii path"),
        "--supports",
        "medium",
        "-o",
        held.to_str().expect("ascii path"),
    ]);
    assert!(with.status.success(), "{}", stderr(&with));
    assert!(
        stdout(&with).contains("Supports"),
        "the run says what it stood: {}",
        stdout(&with)
    );
}
