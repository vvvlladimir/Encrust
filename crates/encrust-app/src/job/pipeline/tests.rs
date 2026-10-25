use super::*;
use core_engine::{Cutting, Model};
use core_format::ExposurePlan;
use core_geometry::{Transform, Vec3};
use core_pipeline::{PanelOverrides, SlicedFormat};
use printer_profiles::{MaterialProfile, PrinterProfile};
use std::path::Path;
use std::sync::Arc;

/// Panel of 64 x 32 pixels at a 0.2 mm pitch: 12.8 x 6.4 mm of build area.
const PROFILE: &str = r#"
name = "Test panel"
manufacturer = "Test"

[display]
width_px = 64
height_px = 32
width_mm = 12.8
height_mm = 6.4

[build_volume]
x = 12.8
y = 6.4
z = 10.0
"#;

/// A closed box, twelve triangles, wound outward.
fn box_mesh(mins: Vec3, maxs: Vec3) -> Mesh {
    core_geometry::cuboid(mins, maxs)
}

/// A path of its own per test, since the job writes to a real file.
fn temp_goo(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("encrust-{}-{name}.goo", std::process::id()))
}

/// A plate holding one 4 x 4 x 2 mm box, standing well inside the panel.
fn plate() -> Plate {
    let mesh = Arc::new(box_mesh(Vec3::new(1.0, 1.0, 0.0), Vec3::new(5.0, 5.0, 2.0)));
    Plate {
        models: vec![Model::placed(mesh, Transform::default())],
        printer: PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
            .expect("the inline profile is valid"),
        material: MaterialProfile::default(),
        panel: PanelOverrides::default(),
        cutting: Cutting::uniform(0.5),
        exposure: ExposurePlan::default(),
        remove_islands: false,
        hanging_passes: 0,
        format: SlicedFormat::Goo,
        raster_window: 2,
        created_unix_s: 0,
    }
}

fn request(output: PathBuf) -> SliceRequest {
    SliceRequest {
        plate: plate(),
        output,
        threads: 2,
    }
}

#[test]
fn a_job_leaves_the_machine_a_core_to_work_with() {
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let threads = worker_threads();
    assert!(threads >= 1, "a job always gets at least one thread");
    assert!(threads < cores || cores == 1, "one core stays free");
}

#[test]
fn a_run_writes_every_layer_and_reports_them_in_order() {
    let output = temp_goo("written");
    let request = request(output.clone());
    let cancel = AtomicBool::new(false);
    let mut messages = Vec::new();

    let outcome = run(&request, &cancel, &mut |message| messages.push(message));

    // 2 mm of model at half a millimetre a layer.
    let Outcome::Written {
        layers,
        clipped_layers,
        volume_mm3,
        ..
    } = outcome
    else {
        panic!("the job should have written a file, got {outcome:?}");
    };
    assert_eq!(layers, 4);
    assert_eq!(clipped_layers, 0, "the model is well inside the panel");
    // 4 x 4 mm of footprint over 2 mm of height.
    assert!((volume_mm3 - 32.0).abs() < 0.5, "expected about 32 mm^3");

    let reported: Vec<usize> = messages
        .iter()
        .filter_map(|message| match message {
            Progress::Layers { done, total } => {
                assert_eq!(*total, 4);
                Some(*done)
            }
            _ => None,
        })
        .collect();
    assert_eq!(reported, vec![2, 4]);
    assert!(messages.contains(&Progress::Stage(Stage::Slicing)));
    assert!(messages.contains(&Progress::Stage(Stage::Rasterising)));

    let written = std::fs::metadata(&output).expect("the file is on disk");
    assert!(written.len() > 0);
    std::fs::remove_file(&output).expect("the test wrote the file");
}

#[test]
fn an_empty_plate_comes_back_as_a_failure() {
    let output = temp_goo("empty");
    let request = SliceRequest {
        plate: Plate {
            models: Vec::new(),
            ..plate()
        },
        ..request(output.clone())
    };

    let Outcome::Failed(message) = run(&request, &AtomicBool::new(false), &mut |_| {}) else {
        panic!("a plate with nothing on it cannot be sliced");
    };
    assert!(message.contains("nothing on the plate"), "got {message}");
    assert!(
        !output.exists(),
        "nothing is created before the plate is cut"
    );
}

#[test]
fn a_job_cancelled_before_it_starts_writes_nothing() {
    let output = temp_goo("cancelled-early");
    let request = request(output.clone());
    let cancel = AtomicBool::new(true);

    let outcome = run(&request, &cancel, &mut |_| {});
    assert_eq!(outcome, Outcome::Cancelled);
    assert!(!output.exists(), "a cancelled job leaves no file behind");
}

#[test]
fn cancelling_part_way_stops_and_removes_the_partial_file() {
    let output = temp_goo("cancelled-late");
    let request = SliceRequest {
        plate: Plate {
            raster_window: 1,
            ..plate()
        },
        ..request(output.clone())
    };
    let cancel = AtomicBool::new(false);
    let mut reported = 0;

    let outcome = run(&request, &cancel, &mut |message| {
        if let Progress::Layers { done, .. } = message {
            reported = done;
            cancel.store(true, Ordering::Relaxed);
        }
    });

    assert_eq!(outcome, Outcome::Cancelled);
    assert_eq!(reported, 1, "the cancel landed after the first layer");
    assert!(!output.exists(), "a cancelled job leaves no file behind");
}

#[test]
fn a_file_that_cannot_be_created_comes_back_as_a_failure() {
    let output = temp_goo("missing-directory")
        .join("nested")
        .join("cube.goo");
    let request = request(output);
    let cancel = AtomicBool::new(false);

    let Outcome::Failed(message) = run(&request, &cancel, &mut |_| {}) else {
        panic!("writing into a directory that does not exist must fail");
    };
    assert!(message.contains("cannot create"), "got {message}");
}

#[test]
fn a_file_the_format_refuses_names_its_whole_path() {
    let output = temp_goo("refused");
    let mut request = request(output.clone());
    request.plate.exposure =
        ExposurePlan::new(vec![core_format::ExposureRange::new(0.0, 1.0, -1.0)]);
    let cancel = AtomicBool::new(false);

    let Outcome::Failed(message) = run(&request, &cancel, &mut |_| {}) else {
        panic!("a negative exposure must fail the file");
    };
    assert!(
        message.contains(&output.display().to_string()),
        "the user is told which file, not just its stem: {message}"
    );
}
