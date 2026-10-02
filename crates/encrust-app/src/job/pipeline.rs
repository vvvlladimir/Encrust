use std::num::NonZeroU8;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use core_analysis::Measured;
use core_format::{ExposurePlan, PrintJob};
use core_geometry::{Mesh, Scalar, Transform};
use core_pipeline::{Observer, PanelOverrides, SlicedFormat, Tolerance, Writing, raster_settings};
use core_raster::{RasterSettings, Shading};
use core_slicer::{
    AdaptiveSettings, ONE_SAMPLE, PlaneSliceEngine, SliceEngine, SliceSettings, Sliced,
    WINDOW_LAYERS, Windows, adaptive_plan,
};
use core_thumbnail::{Part, ThumbnailSettings, render as render_thumbnail};
use format_chitu::CtbVersion;
use printer_profiles::{Compensation, MaterialProfile, PrinterProfile};

use crate::job::{Outcome, Progress, Stage};

/// Everything one slicing run needs, owned so that it can cross to the worker thread.
pub struct SliceRequest {
    /// Every visible model baked into plate coordinates, cut a window at a time as the
    /// file is written; see `docs/decisions/0068-the-window-holds-no-stack.md`.
    pub mesh: Arc<Mesh>,
    /// Every visible mesh and where it stands, for the file's thumbnail.
    pub plate: Vec<(Arc<Mesh>, Transform)>,
    pub output: PathBuf,
    pub cutting: Cutting,
    pub printer: PrinterProfile,
    pub material: MaterialProfile,
    /// Exposure bands over the resin's own; empty means the resin's throughout.
    pub exposure: ExposurePlan,
    pub shading: Shading,
    /// How many greys an anti-aliased edge is rounded to, or `None` for all 255.
    pub grey_levels: Option<NonZeroU8>,
    /// Radius an edge is faded over, pixels; `0` leaves it sharp.
    pub blur_px: u8,
    /// Whether every island is taken out of the file.
    pub remove_islands: bool,
    /// Revision a `.ctb` output is written at; ignored for every other format.
    pub ctb_version: CtbVersion,
    /// Layers rasterised at once. Peak memory is this many masks; see
    /// `docs/decisions/0010-streaming-raster-stack.md`.
    pub window: usize,
    /// Threads the job may use. See `worker_threads`.
    pub threads: usize,
}

impl SliceRequest {
    /// The panel this run draws its masks for.
    pub fn panel(&self) -> RasterSettings {
        raster_settings(
            &self.printer,
            PanelOverrides {
                shading: self.shading,
                grey_levels: self.grey_levels,
                grey_floor: None,
                blur_px: self.blur_px,
            },
        )
    }
}

/// Threads a background job runs on: one fewer than the machine has.
///
/// The window has to keep drawing while the job runs, and a desktop slicer that pins
/// every core is a slicer that makes the machine unusable and hot; see
/// `docs/decisions/0019-slicing-job-thread-budget.md`.
pub fn worker_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |cores| cores.get().saturating_sub(1).max(1))
}

/// Slices, rasterises and writes the sliced file, reporting as it goes.
///
/// Every failure comes back as `Outcome::Failed` rather than a `Result`: the caller is a
/// worker thread whose only channel to the user is `report`.
pub fn run(
    request: &SliceRequest,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(Progress) + Send),
) -> Outcome {
    let pool = match thread_pool(request.threads) {
        Ok(pool) => pool,
        Err(error) => return Outcome::Failed(error.to_string()),
    };

    pool.install(|| match slice_and_write(request, cancel, report) {
        Ok(outcome) => outcome,
        Err(error) => Outcome::Failed(
            error
                .chain()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(": "),
        ),
    })
}

/// A pool of `threads` threads for one job to run on.
///
/// Every job builds its own, so the thread budget covers the slicer's parallelism as well
/// as the rasteriser's, and the rest of the machine keeps a core.
pub fn thread_pool(threads: usize) -> Result<rayon::ThreadPool> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
        .context("cannot start the slicing threads")
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

/// How a stack is cut: one thickness throughout, or as thick as the surface allows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cutting {
    /// The thickest layer the stack may use, millimetres.
    pub layer_height_mm: Scalar,
    /// The rules an adaptive stack follows, or `None` for one thickness throughout.
    pub adaptive: Option<AdaptiveSettings>,
    /// How many planes are sampled inside each layer's band.
    pub samples: NonZeroU8,
    /// The resin's corrections, of which only the shrinkage changes what is cut.
    pub compensation: Compensation,
}

impl Cutting {
    pub fn uniform(layer_height_mm: Scalar) -> Self {
        Self {
            layer_height_mm,
            adaptive: None,
            samples: ONE_SAMPLE,
            compensation: Compensation::default(),
        }
    }
}

/// The windows a mesh standing in plate coordinates will be cut in.
pub fn windows_of(mesh: &Mesh, cutting: Cutting) -> Result<Windows> {
    let Some(adaptive) = cutting.adaptive else {
        let settings = SliceSettings {
            layer_height: cutting.layer_height_mm,
            samples: cutting.samples,
        };
        return Windows::new(mesh, settings, WINDOW_LAYERS).context("cannot slice the model");
    };
    let plan = adaptive_plan(
        mesh,
        &AdaptiveSettings {
            max_height_mm: cutting.layer_height_mm,
            ..adaptive
        },
    )
    .context("cannot plan the layers")?;
    Ok(Windows::planned(plan, cutting.samples, WINDOW_LAYERS))
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

/// The worker's cancel flag and progress channel, as the pipeline wants to see them.
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
) -> Result<Outcome> {
    report(Progress::Stage(Stage::Slicing));
    let windows = windows_of(&request.mesh, request.cutting)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(Outcome::Cancelled);
    }

    let raster = request.panel();
    raster
        .validate()
        .context("the panel cannot produce a usable mask")?;

    let path = &request.output;
    let format = SlicedFormat::of(path, request.ctb_version)
        .with_context(|| format!("{} names no sliced-file format", path.display()))?
        .at_revision_of(request.printer.output);

    let job = PrintJob {
        printer: request.printer.clone(),
        material: request.material.clone(),
        raster,
        plan: windows.plan().clone(),
        // Only known once the stack has been cut; `finish` writes it (ADR 0067).
        volume_mm3: 0.0,
        exposure: request.exposure.clone(),
        thumbnail: thumbnail(request),
    };

    let mut fold = Measured::new(request.material.bottom_layers as usize);
    if request.remove_islands {
        fold = fold.removing_islands();
    }

    report(Progress::Stage(Stage::Rasterising));
    let written = core_pipeline::write(
        &Writing {
            format,
            path,
            job: &job,
            mesh: &request.mesh,
            windows: &windows,
            settings: &raster,
            window: request.window,
            fold,
        },
        &mut Worker { cancel, report },
    )?;

    let Some(written) = written else {
        return Ok(Outcome::Cancelled);
    };
    Ok(Outcome::Written {
        path: path.clone(),
        layers: written.layers,
        clipped_layers: written.clipped_layers,
        volume_mm3: written.measured.volume_mm3(),
    })
}

/// The picture of the plate the file carries, or `None` when nothing was handed over.
fn thumbnail(request: &SliceRequest) -> Option<core_thumbnail::Thumbnail> {
    let parts: Vec<Part<'_>> = request
        .plate
        .iter()
        .map(|(mesh, transform)| Part::new(mesh, *transform))
        .collect();
    (!parts.is_empty()).then(|| render_thumbnail(&parts, &ThumbnailSettings::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;
    use std::path::Path;

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

    /// A path of its own per test, since the pipeline writes to a real file.
    fn temp_goo(name: &str) -> PathBuf {
        temp_output(name, "goo")
    }

    fn temp_output(name: &str, extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!("encrust-{}-{name}.{extension}", std::process::id()))
    }

    fn request(output: PathBuf) -> SliceRequest {
        SliceRequest {
            grey_levels: None,
            blur_px: 0,
            remove_islands: false,
            // 4 x 4 x 2 mm, standing on the plate well inside the panel.
            mesh: Arc::new(box_mesh(Vec3::new(1.0, 1.0, 0.0), Vec3::new(5.0, 5.0, 2.0))),
            plate: vec![(
                Arc::new(box_mesh(Vec3::new(1.0, 1.0, 0.0), Vec3::new(5.0, 5.0, 2.0))),
                Transform::default(),
            )],
            output,
            cutting: Cutting::uniform(0.5),
            printer: PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
                .expect("the inline profile is valid"),
            material: MaterialProfile::default(),
            exposure: ExposurePlan::default(),
            shading: Shading::Coverage,
            ctb_version: CtbVersion::V4,
            window: 2,
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
    fn the_requested_revision_reaches_the_ctb_header() {
        let output = temp_output("version-five", "ctb");
        let request = SliceRequest {
            ctb_version: CtbVersion::V5,
            ..request(output.clone())
        };
        let cancel = AtomicBool::new(false);

        let outcome = run(&request, &cancel, &mut |_| {});
        assert!(
            matches!(outcome, Outcome::Written { .. }),
            "got {outcome:?}"
        );

        let file = std::fs::read(&output).expect("the file is on disk");
        assert_eq!(u32::from_le_bytes([file[4], file[5], file[6], file[7]]), 5);
        std::fs::remove_file(&output).expect("the test wrote the file");
    }

    #[test]
    fn a_name_no_format_claims_fails_without_creating_a_file() {
        let output = temp_output("unknown", "sliced");
        let request = request(output.clone());
        let cancel = AtomicBool::new(false);

        let Outcome::Failed(message) = run(&request, &cancel, &mut |_| {}) else {
            panic!("an extension no writer claims cannot be written");
        };
        assert!(message.contains("no sliced-file format"), "got {message}");
        assert!(
            !output.exists(),
            "nothing is created before the format is known"
        );
    }

    #[test]
    fn the_written_file_carries_a_picture_of_the_plate() {
        let output = temp_goo("thumbnail");
        let request = request(output.clone());
        run(&request, &AtomicBool::new(false), &mut |_| {});

        let file = std::fs::read(&output).expect("the run wrote the file");
        // The small preview is 116 by 116 RGB565 at a fixed offset; see docs/formats/goo.md
        let preview = &file[194..194 + 2 * 116 * 116];
        assert!(
            preview.iter().any(|byte| *byte != 0),
            "the box on the plate reaches the preview record"
        );
        std::fs::remove_file(&output).expect("the test wrote the file");
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
            window: 1,
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
