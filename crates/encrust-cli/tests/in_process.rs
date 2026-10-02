//! The CLI driven through its library rather than by spawning the binary.
//!
//! What this pins down is that the flags a user types reach the same
//! `core_pipeline::write` the window calls; `cli.rs` covers the process surface — exit
//! codes, stdout, and what a broken invocation prints.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use encrust_cli::{Cli, Exit, Stop, exit_code, run};

/// Binary STL of an axis-aligned box, written the way an exporter would: three fresh
/// vertices per triangle.
fn write_box_stl(name: &str, size: f32) -> PathBuf {
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
    let triangles: [[usize; 3]; 12] = [
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

    let mut bytes = vec![0u8; 80];
    bytes.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    for triangle in triangles {
        bytes.extend_from_slice(&[0u8; 12]);
        for corner in triangle {
            for component in corners[corner] {
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
        "encrust-cli-in-process-{}-{name}.{extension}",
        std::process::id()
    ))
}

/// `encrust slice` as a user would type it, parsed the way `main` parses it and run
/// until `stop` is asked for.
fn slice_until(arguments: &[&str], stop: &Stop) -> anyhow::Result<Exit> {
    let mut argv = vec!["encrust", "slice"];
    argv.extend_from_slice(arguments);
    run(&Cli::parse_from(argv), stop)
}

fn slice(arguments: &[&str]) -> anyhow::Result<Exit> {
    slice_until(arguments, &Stop::default())
}

fn as_str(path: &Path) -> String {
    path.to_str()
        .expect("the temporary path is UTF-8")
        .to_owned()
}

/// Closes a `.goo` header; see `docs/formats/goo.md`.
const GOO_MAGIC: [u8; 8] = [0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00];

#[test]
fn a_box_typed_at_the_command_line_becomes_a_printable_goo_file() {
    let model = write_box_stl("cube", 10.0);
    let out = temp("cube", "goo");

    let clean = slice(&[
        &as_str(&model),
        "--printer",
        "elegoo-mars-4-ultra",
        "--center",
        "-o",
        &as_str(&out),
    ])
    .expect("a closed box slices");

    assert_eq!(clean, Exit::Success, "a closed box has no defect to report");
    let file = fs::read(&out).expect("the run wrote the file");
    assert_eq!(&file[4..12], &GOO_MAGIC, "the goo writer wrote the header");
    let _ = fs::remove_file(&model);
    let _ = fs::remove_file(&out);
}

#[test]
fn the_ctb_revision_flag_reaches_the_file_the_window_would_have_written() {
    let model = write_box_stl("revision", 10.0);
    let out = temp("revision", "ctb");

    slice(&[
        &as_str(&model),
        "--printer",
        "elegoo-mars-4-ultra",
        "--center",
        "--ctb-version",
        "5",
        "-o",
        &as_str(&out),
    ])
    .expect("a closed box slices");

    let file = fs::read(&out).expect("the run wrote the file");
    assert_eq!(
        u32::from_le_bytes([file[4], file[5], file[6], file[7]]),
        5,
        "the extension picks the family and the flag picks the revision"
    );
    let _ = fs::remove_file(&model);
    let _ = fs::remove_file(&out);
}

#[test]
fn an_open_mesh_fails_a_strict_run_without_an_error() {
    // A box missing its lid: --strict reports it as unclean rather than as a failure.
    let model = temp("open", "stl");
    let whole = fs::read(write_box_stl("open-source", 10.0)).expect("the box is written");
    let mut bytes = whole[..84].to_vec();
    bytes[80..84].copy_from_slice(&10u32.to_le_bytes());
    bytes.extend_from_slice(&whole[84..84 + 10 * 50]);
    fs::write(&model, bytes).expect("the temporary directory is writable");

    let out = temp("open", "goo");
    let clean = slice(&[
        &as_str(&model),
        "--printer",
        "elegoo-mars-4-ultra",
        "--center",
        "--strict",
        "-o",
        &as_str(&out),
    ])
    .expect("an open mesh still slices");

    assert_eq!(
        clean,
        Exit::Unclean,
        "--strict has to notice the missing faces"
    );
    let _ = fs::remove_file(&model);
    let _ = fs::remove_file(&out);
}

#[test]
fn a_name_no_writer_claims_is_written_as_a_png_stack() {
    let model = write_box_stl("png", 4.0);
    let out = temp("png-stack", "d");

    slice(&[
        &as_str(&model),
        "--printer",
        "elegoo-mars-4-ultra",
        "--center",
        "-o",
        &as_str(&out),
    ])
    .expect("a closed box slices");

    let layers = fs::read_dir(&out)
        .expect("the run made the directory")
        .count();
    assert!(layers > 0, "a directory name asks for a PNG stack");
    let _ = fs::remove_file(&model);
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn a_run_stopped_by_ctrl_c_exits_130_and_leaves_no_file() {
    let model = write_box_stl("cancelled", 10.0);
    let out = temp("cancelled", "goo");
    let stop = Stop::default();
    stop.request();

    let result = slice_until(
        &[
            &as_str(&model),
            "--printer",
            "elegoo-mars-4-ultra",
            "--center",
            "-o",
            &as_str(&out),
        ],
        &stop,
    );

    assert_eq!(exit_code(&result), 130, "got {result:?}");
    assert!(!out.exists(), "a cancelled run removes what it started");
    let _ = fs::remove_file(&model);
}
