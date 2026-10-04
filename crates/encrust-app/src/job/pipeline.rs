use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use core_analysis::Measured;
use core_engine::{EngineError, Plate, Run};
#[cfg(target_arch = "wasm32")]
use core_format::WriteSeek;
use core_geometry::{Mesh, Scalar};
use core_pipeline::{Observer, Tolerance, Written};
use core_raster::RasterSettings;
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Sliced, Windows};

use crate::job::{Outcome, Progress, Stage};

/// Everything one slicing run needs, owned so that it can cross to the worker thread.
pub struct SliceRequest {
    /// The plate as the engine takes it: the models where they stand, the machine, the
    /// resin, and how the stack is cut and drawn.
    pub plate: Plate,
    pub output: PathBuf,
    /// Threads the job may use. See [`worker_threads`].
    pub threads: usize,
}

/// Threads a background job runs on: one fewer than the machine has.
///
/// The window has to keep drawing while the job runs, and a desktop slicer that pins
/// every core is a slicer that makes the machine unusable and hot; see
/// `docs/decisions/0019-slicing-job-thread-budget.md`.
pub fn worker_threads() -> usize {
    crate::job::cores().saturating_sub(1).max(1)
}

/// Slices, rasterises and writes the sliced file to `request.output`, reporting as it goes.
///
/// Every failure comes back as `Outcome::Failed` rather than a `Result`: the caller is a
/// worker thread whose only channel to the user is `report`.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(
    request: &SliceRequest,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(Progress) + Send),
) -> Outcome {
    let pool = match thread_pool(request.threads) {
        Ok(pool) => pool,
        Err(error) => return Outcome::Failed(error.to_string()),
    };

    pool.install(|| {
        flattened(slice_and_write(request, cancel, report, |run, observer| {
            run.write_file(&request.output, observer)
        }))
    })
}

/// The same, into `sink`, under the name `request.output` gives the container.
///
/// In a browser the sink is a file only this thread may touch, so the job runs here, on
/// the shared pool, rather than in a pool of its own.
#[cfg(target_arch = "wasm32")]
pub fn run_into(
    request: &SliceRequest,
    sink: &mut dyn WriteSeek,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(Progress) + Send),
) -> Outcome {
    let name = request
        .output
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    flattened(slice_and_write(request, cancel, report, |run, observer| {
        run.write(&name, sink, observer)
    }))
}

fn flattened(result: Result<Outcome>) -> Outcome {
    result.unwrap_or_else(|error| {
        Outcome::Failed(
            error
                .chain()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(": "),
        )
    })
}

/// A pool of `threads` threads for one job to run on.
///
/// Every job builds its own, so the thread budget covers the slicer's parallelism as well
/// as the rasteriser's, and the rest of the machine keeps a core.
pub fn thread_pool(threads: usize) -> Result<Pool> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads.max(1))
            .build()
            .map(Pool)
            .context("cannot start the slicing threads")
    }
    // In a browser a thread is a worker started at a cost, so every job shares one pool,
    // held to the same budget; see `crate::web::thread`.
    #[cfg(target_arch = "wasm32")]
    {
        let _ = threads;
        Ok(Pool)
    }
}

/// Where a job's parallel loops run.
#[cfg(not(target_arch = "wasm32"))]
pub struct Pool(rayon::ThreadPool);

#[cfg(target_arch = "wasm32")]
pub struct Pool;

impl Pool {
    /// Runs `op` with its parallel loops on this pool.
    pub fn install<R: Send>(&self, op: impl FnOnce() -> R + Send) -> R {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.0.install(op)
        }
        #[cfg(target_arch = "wasm32")]
        {
            op()
        }
    }
}

/// Cuts a mesh standing in plate coordinates into its whole stack of closed contours.
///
/// Only for the readers that need every layer at once — support placement walks the
/// stack from the plate up. Writing a file cuts a window at a time instead.
pub fn slice_mesh(mesh: &Mesh, layer_height_mm: Scalar) -> Result<Sliced> {
    let settings = SliceSettings {
        layer_height: layer_height_mm,
        ..SliceSettings::default()
    };
    PlaneSliceEngine
        .slice(mesh, &settings)
        .context("cannot slice the model")
}

/// Measures what the stack cures without writing it, so Preview can say what the print
/// takes. `None` when cancelled.
pub fn measure_stack(
    mesh: &Mesh,
    windows: &Windows,
    settings: &RasterSettings,
    tolerance: &Tolerance,
    fold: Measured,
    cancel: &AtomicBool,
) -> Result<Option<Measured>> {
    core_pipeline::measure(
        mesh,
        windows,
        settings,
        tolerance,
        fold,
        &mut Cancel(cancel),
    )
    .context("cannot measure the stack")
}

/// The worker's cancel flag and progress channel, as the engine wants to see them.
struct Worker<'a> {
    cancel: &'a AtomicBool,
    report: &'a mut (dyn FnMut(Progress) + Send),
}

impl Observer for Worker<'_> {
    fn layers(&mut self, done: usize, total: usize) {
        (self.report)(Progress::Layers { done, total });
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// The same, for a run with nothing to report.
struct Cancel<'a>(&'a AtomicBool);

impl Observer for Cancel<'_> {
    fn cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

fn slice_and_write(
    request: &SliceRequest,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(Progress) + Send),
    write: impl FnOnce(&Run, &mut Worker<'_>) -> Result<Option<Written>, EngineError>,
) -> Result<Outcome> {
    report(Progress::Stage(Stage::Slicing));
    let run = Run::of(&request.plate)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(Outcome::Cancelled);
    }

    report(Progress::Stage(Stage::Rasterising));
    let written = write(&run, &mut Worker { cancel, report })?;

    let Some(written) = written else {
        return Ok(Outcome::Cancelled);
    };
    Ok(Outcome::Written {
        path: request.output.clone(),
        layers: written.layers,
        clipped_layers: written.clipped_layers,
        volume_mm3: written.measured.volume_mm3(),
    })
}

#[cfg(test)]
mod tests {
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
        let vertices = vec![
            Vec3::new(mins.x, mins.y, mins.z),
            Vec3::new(maxs.x, mins.y, mins.z),
            Vec3::new(maxs.x, maxs.y, mins.z),
            Vec3::new(mins.x, maxs.y, mins.z),
            Vec3::new(mins.x, mins.y, maxs.z),
            Vec3::new(maxs.x, mins.y, maxs.z),
            Vec3::new(maxs.x, maxs.y, maxs.z),
            Vec3::new(mins.x, maxs.y, maxs.z),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
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
        Mesh::new(vertices, faces)
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
}
