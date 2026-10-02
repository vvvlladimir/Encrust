use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_analysis::Measured;
use core_geometry::Mesh;
use core_pipeline::Tolerance;
use core_raster::RasterSettings;
use core_slicer::Windows;

use crate::job::pipeline::{measure_stack, thread_pool, worker_threads};

/// How a measuring pass ended.
#[derive(Debug, Clone, PartialEq)]
pub enum MeasureOutcome {
    Measured(Box<Measured>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// The stack Preview shows, rasterised and measured on its own thread without a file.
/// Dropping the handle stops it at the next window.
pub struct MeasureJob {
    result: Receiver<MeasureOutcome>,
    cancel: Arc<AtomicBool>,
}

impl MeasureJob {
    /// Starts folding `windows` of `mesh` into `fold`.
    pub fn spawn(
        mesh: Arc<Mesh>,
        windows: Windows,
        settings: RasterSettings,
        tolerance: Tolerance,
        fold: Measured,
    ) -> Self {
        let (sender, result) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);

        std::thread::spawn(move || {
            let measured = thread_pool(worker_threads()).and_then(|pool| {
                pool.install(|| {
                    measure_stack(&mesh, &windows, &settings, &tolerance, fold, &worker_cancel)
                })
            });
            let outcome = match measured {
                Ok(Some(measured)) => MeasureOutcome::Measured(Box::new(measured)),
                Ok(None) => return,
                Err(error) => MeasureOutcome::Failed(
                    error
                        .chain()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(": "),
                ),
            };
            let _ = sender.send(outcome);
        });

        Self { result, cancel }
    }

    /// The outcome on the frame the pass ends, and `None` while it is still running.
    pub fn poll(&mut self) -> Option<MeasureOutcome> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(MeasureOutcome::Failed(
                "the measuring thread stopped without finishing".to_owned(),
            )),
        }
    }
}

impl Drop for MeasureJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;
    use core_raster::{Grey, PixelPitch, Shading};
    use core_slicer::{ONE_SAMPLE, SliceSettings, WINDOW_LAYERS};

    /// A 2 mm cube standing on the plate.
    fn cube() -> Mesh {
        let corner = |i: usize| {
            Vec3::new(
                if i & 1 == 0 { 1.0 } else { 3.0 },
                if i & 2 == 0 { 1.0 } else { 3.0 },
                if i & 4 == 0 { 0.0 } else { 2.0 },
            )
        };
        Mesh::new(
            (0..8).map(corner).collect(),
            vec![
                [0, 2, 1],
                [1, 2, 3],
                [4, 5, 6],
                [5, 7, 6],
                [0, 1, 4],
                [1, 5, 4],
                [2, 6, 3],
                [3, 6, 7],
                [0, 4, 2],
                [2, 4, 6],
                [1, 3, 5],
                [3, 7, 5],
            ],
        )
    }

    fn panel() -> RasterSettings {
        RasterSettings {
            width_px: 200,
            height_px: 200,
            pitch: PixelPitch { x: 0.02, y: 0.02 },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        }
    }

    #[test]
    fn a_cube_measures_as_its_own_volume() {
        let mesh = cube();
        let settings = SliceSettings {
            layer_height: 0.1,
            samples: ONE_SAMPLE,
        };
        let windows = Windows::new(&mesh, settings, WINDOW_LAYERS).expect("a cube slices");
        let mut job = MeasureJob::spawn(
            Arc::new(mesh),
            windows,
            panel(),
            Tolerance::default(),
            Measured::new(0),
        );
        let outcome = loop {
            if let Some(outcome) = job.poll() {
                break outcome;
            }
            std::thread::yield_now();
        };
        let MeasureOutcome::Measured(measured) = outcome else {
            panic!("a cube on the panel measures: {outcome:?}");
        };
        // 2 x 2 x 2 mm, on pixel edges, so the masks cure it exactly.
        assert!(
            (measured.volume_mm3() - 8.0).abs() < 0.01,
            "{}",
            measured.volume_mm3()
        );
        assert_eq!(measured.layer_count(), 20);
    }
}
