//! The window's path from a model file to a sliced file, without a window.
//!
//! What this pins down is that a model imported, placed and sliced the way the window does
//! it reaches `core_pipeline::write` intact; `encrust-cli/tests/in_process.rs` is the same
//! test for the command line.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use encrust_app::{BuildPlate, Handed, Scene, Slicing, Status, prepare};
use format_goo::decode;
use printer_profiles::Catalogue;

const PRINTER: &str = "elegoo-mars-4-ultra";

/// Where the header and the layer records sit; see docs/formats/goo.md
const TOTAL_LAYERS: usize = 195_310;
const HEADER_SIZE: usize = 0x2FB95;
const LAYER_DEFINITION_SIZE: usize = 66;

/// Binary STL of an axis-aligned cube of side `size` mm, three fresh vertices per
/// triangle the way an exporter writes one.
fn write_cube_stl(name: &str, size: f32) -> PathBuf {
    let corner = |index: usize| {
        [
            if index & 1 == 0 { 0.0 } else { size },
            if index & 2 == 0 { 0.0 } else { size },
            if index & 4 == 0 { 0.0 } else { size },
        ]
    };
    let triangles: [[usize; 3]; 12] = [
        [0, 2, 3],
        [0, 3, 1],
        [4, 5, 7],
        [4, 7, 6],
        [0, 1, 5],
        [0, 5, 4],
        [1, 3, 7],
        [1, 7, 5],
        [3, 2, 6],
        [3, 6, 7],
        [2, 0, 4],
        [2, 4, 6],
    ];

    let mut bytes = vec![0u8; 80];
    bytes.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    for triangle in triangles {
        bytes.extend_from_slice(&[0u8; 12]);
        for index in triangle {
            for component in corner(index) {
                bytes.extend_from_slice(&component.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0u8; 2]);
    }

    let path = temp(name, "stl");
    fs::write(&path, bytes).expect("the temporary directory is writable");
    path
}

fn temp(name: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "encrust-app-{}-{name}.{extension}",
        std::process::id()
    ))
}

/// Polls the job once a frame, as the window does, until it has ended.
fn finish(slicing: &mut Slicing, scene: &Scene) -> Status {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut status = Status::default();
    while slicing.poll(scene, &mut status) {
        assert!(Instant::now() < deadline, "the job never ended");
        std::thread::sleep(Duration::from_millis(10));
    }
    status
}

/// Every layer's lit area in pixels, a grey pixel counting for the share of it lit.
fn lit_px_per_layer(file: &[u8]) -> Vec<f32> {
    let bytes = |at: usize| [file[at], file[at + 1], file[at + 2], file[at + 3]];
    let count = u32::from_be_bytes(bytes(TOTAL_LAYERS)) as usize;

    let mut cursor = HEADER_SIZE;
    (0..count)
        .map(|_| {
            cursor += LAYER_DEFINITION_SIZE;
            let size = u32::from_be_bytes(bytes(cursor)) as usize;
            // Past the size, the magic byte opens the runs and the checksum closes them.
            let runs = &file[cursor + 5..cursor + 4 + size - 1];
            cursor += 4 + size + 2;
            decode(runs)
                .expect("the writer's own runs decode")
                .iter()
                .map(|run| run.length as f32 * f32::from(run.value) / 255.0)
                .sum()
        })
        .collect()
}

#[test]
fn a_cube_imported_and_sliced_as_the_window_does_reaches_the_file_whole() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue loads");
    let printer = catalogue.printer(PRINTER).expect("the printer ships");
    let (pitch_x, pitch_y) = printer.profile.display.pixel_pitch_mm();
    let plate = BuildPlate::from_profile(&printer.profile);

    let model = write_cube_stl("cube", 10.0);
    let mut scene = Scene::default();
    scene.insert(
        prepare(&Handed::Path(model.clone()), &plate, &mut |_| {}).expect("a closed cube imports"),
    );

    let mut slicing = Slicing::default();
    slicing.set_printer(printer.profile.clone(), Some(printer.id.clone()));
    assert_eq!(slicing.blocker(&scene), None, "the plate is ready to slice");

    let output = temp("cube", "goo");
    slicing
        .start(&scene, scene.active_plate(), output.clone())
        .expect("the plate has a model on it");
    let status = finish(&mut slicing, &scene);
    assert!(matches!(status, Status::Info(_)), "got {status:?}");

    let file = fs::read(&output).expect("the job wrote the file");
    let layers = lit_px_per_layer(&file);
    let expected_layers = (10.0 / slicing.layer_height_mm()).round() as usize;
    assert_eq!(
        layers.len(),
        expected_layers,
        "10 mm of cube, cut at the resin's height"
    );

    // A 10 x 10 mm face, in pixels of this panel; the edges are anti-aliased, so the
    // tolerance is a rim one pixel wide around it.
    let face_px = 100.0 / (pitch_x * pitch_y);
    let rim_px = 40.0 / pitch_x.min(pitch_y);
    for (index, lit) in layers.iter().enumerate() {
        assert!(
            (lit - face_px).abs() < rim_px,
            "layer {index} lights {lit} px, the face is {face_px} px"
        );
    }

    let _ = fs::remove_file(&model);
    let _ = fs::remove_file(&output);
}
