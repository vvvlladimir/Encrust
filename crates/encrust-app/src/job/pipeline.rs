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
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Sliced, Solid, Windows};

use crate::i18n::tr;
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
            .context(tr!("job-cannot-start-threads"))
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
        .context(tr!("job-cannot-slice"))
}

/// Measures what the stack cures without writing it, so Preview can say what the print
/// takes. `None` when cancelled.
pub fn measure_stack(
    solid: &Solid,
    windows: &Windows,
    settings: &RasterSettings,
    tolerance: &Tolerance,
    fold: Measured,
    cancel: &AtomicBool,
) -> Result<Option<Measured>> {
    core_pipeline::measure(
        solid,
        windows,
        settings,
        tolerance,
        fold,
        &mut Cancel(cancel),
    )
    .context(tr!("job-cannot-measure"))
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
    let written = write(&run, &mut Worker { cancel, report }).with_context(|| {
        tr!(
            "error-cannot-write",
            path = request.output.display().to_string()
        )
    })?;

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
mod tests;
