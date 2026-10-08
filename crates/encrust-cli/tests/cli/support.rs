//! What every file of this test binary builds its runs from: a model to slice, the
//! profiles to slice it with, and the binary spawned on them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Binary STL of an axis-aligned box, written the way an exporter would: three fresh
/// vertices per triangle. `faces` selects which of the twelve triangles to include, and
/// `invert` reverses the winding of the first `invert` of them.
pub fn write_box_stl(name: &str, size: f32, faces: usize, invert: usize) -> PathBuf {
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
pub fn shipped_profile() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/profiles/printers/elegoo-mars-4-ultra.toml")
}

/// A 200 x 200 px panel, small enough to rasterise in a test.
pub fn test_panel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-panel.toml")
}

/// A fresh, empty output directory of its own for each test.
pub fn output_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("encrust-cli-out-{name}"));
    let _ = fs::remove_dir_all(&path);
    path
}

/// A fresh output file path of its own for each test.
pub fn output_file(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("encrust-cli-out-{name}"));
    let _ = fs::remove_file(&path);
    path
}

/// A resin of the tests' own: the catalogue ships none (ADR 0196).
pub fn test_resin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-resin.toml")
}

pub fn layer_pixels(directory: &Path, index: usize) -> (u32, u32, Vec<u8>) {
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

pub fn written_layers(directory: &Path) -> usize {
    fs::read_dir(directory)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

/// Pixels the printer would expose on one layer of a stack.
pub fn lit_pixels(directory: &Path, index: usize) -> usize {
    let (_, _, pixels) = layer_pixels(directory, index);
    pixels.iter().filter(|&&value| value > 0).count()
}

pub fn encrust(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_encrust"))
        .args(args)
        .output()
        .expect("the encrust binary runs")
}

pub fn slice(args: &[&str]) -> Output {
    encrust(&[&["slice"], args].concat())
}

pub fn estimate(args: &[&str]) -> Output {
    encrust(&[&["estimate"], args].concat())
}

/// Runs the binary with its own profile directory, so a test never reads the real one.
pub fn encrust_with_profiles(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_encrust"))
        .env("ENCRUST_PROFILE_DIR", dir)
        .args(args)
        .output()
        .expect("the encrust binary runs")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf-8")
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf-8")
}

pub fn field<'a>(text: &'a str, label: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.trim_start().strip_prefix(label))
        .unwrap_or_else(|| panic!("no {label:?} line in:\n{text}"))
        .trim()
}

pub fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout is not one JSON document ({error}):\n{}",
            stdout(output)
        )
    })
}

/// A directory with two cubes in it and something that is not a model, for a batch run.
pub fn batch_input(name: &str) -> PathBuf {
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
