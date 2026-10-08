//! The runs behind the preview: cutting the window being shown, and measuring what the
//! stack cures, each on a thread of its own.

use std::sync::Arc;

use anyhow::{Result, bail};
use core_analysis::Measured;
use core_raster::RasterSettings;

use core_engine::{Cutting, bake};

use crate::job::{MeasureJob, MeasureOutcome, PreviewJob, PreviewOutcome, models_of};
use crate::scene::Scene;
use crate::status::Status;

use super::picture::as_seen;
use super::*;

impl Preview {
    /// Starts cutting everything visible on the plate. Fails when there is nothing there.
    pub fn build(&mut self, scene: &Scene, cutting: Cutting) -> Result<()> {
        let baked = bake(
            &models_of(scene, scene.active_plate()),
            &cutting.compensation,
        )
        .context("nothing visible on the plate to preview")?;
        self.job = Some(Build {
            job: PreviewJob::spawn(baked, cutting),
            fingerprint: stack_fingerprint(scene, cutting),
        });
        Ok(())
    }

    /// Drains a running build and measurement. Returns whether one is still going, which
    /// is what tells the window to keep repainting.
    pub fn poll(&mut self, status: &mut Status) -> bool {
        let building = self.poll_build(status);
        self.poll_measure(status) || building
    }

    pub(super) fn poll_build(&mut self, status: &mut Status) -> bool {
        let Some(build) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = build.job.poll() else {
            return true;
        };

        let fingerprint = build.fingerprint;
        self.job = None;
        match outcome {
            PreviewOutcome::Built(mesh, windows) => {
                self.layer = self.layer.min(windows.layer_count().saturating_sub(1));
                self.source = Some(Source::Cut(Stack {
                    mesh,
                    windows,
                    fingerprint,
                    cut: None,
                }));
                if let Some(height_mm) = self.parked.take() {
                    self.show_height(height_mm);
                }
            }
            PreviewOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    /// Starts measuring the stack as `settings` would write it, unless that measurement is
    /// already held or under way.
    pub fn measure(&mut self, settings: &RasterSettings, fold: Fold) {
        self.fold = fold;
        let Some(key) = self.measure_key(settings, fold) else {
            return;
        };
        let known = |held: Option<MeasureKey>| held == Some(key);
        if known(self.measured.as_ref().map(|(key, _)| *key))
            || known(self.measuring.as_ref().map(|(key, _)| *key))
        {
            return;
        }
        // Only a plate being cut is measured this way. A file's masks are already
        // written, so what they cure is read off them instead; see `read_measured`.
        let Some(stack) = self.source.as_ref().and_then(Source::stack) else {
            return;
        };
        // Measured as the picture shows it, so a risk's position lands on the picture.
        let job = MeasureJob::spawn(
            Arc::clone(&stack.mesh),
            stack.windows.clone(),
            as_seen(settings),
            fold.tolerance,
            fold.start(),
        );
        self.measuring = Some((key, job));
    }

    /// What the stack cures, once it has been measured for the panel as it now is.
    pub fn measured(&self, settings: &RasterSettings, fold: Fold) -> Option<&Measured> {
        let key = self.measure_key(settings, fold)?;
        let (measured_key, measured) = self.measured.as_ref()?;
        (*measured_key == key).then_some(measured)
    }

    pub fn is_measuring(&self) -> bool {
        self.measuring.is_some()
    }

    pub(super) fn measure_key(&self, settings: &RasterSettings, fold: Fold) -> Option<MeasureKey> {
        Some(MeasureKey {
            fingerprint: self.source.as_ref()?.fingerprint(),
            settings: *settings,
            fold,
        })
    }

    pub(super) fn poll_measure(&mut self, status: &mut Status) -> bool {
        let Some((key, job)) = self.measuring.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };
        let key = *key;
        self.measuring = None;
        match outcome {
            MeasureOutcome::Measured(measured) => self.measured = Some((key, *measured)),
            MeasureOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    /// Stops waiting for the build. The thread it left behind finishes into a channel
    /// nobody reads; a slicing run has no point inside it to stop at.
    pub fn cancel(&mut self) {
        self.job = None;
    }

    pub fn is_building(&self) -> bool {
        self.job.is_some()
    }

    /// Cuts the window the layer being shown falls in, unless it is the one already cut.
    /// A file being read has nothing to cut.
    pub(super) fn cut_window(&mut self) -> Result<()> {
        let layer = self.layer;
        let stack = match self.source.as_mut() {
            Some(Source::Cut(stack)) => stack,
            Some(Source::Read(_)) => return Ok(()),
            None => bail!("no plate to preview"),
        };
        let wanted = stack.windows.window_of(layer);
        if stack
            .cut
            .as_ref()
            .is_some_and(|(cached, _)| *cached == wanted)
        {
            return Ok(());
        }

        let sliced = stack
            .windows
            .cut(&stack.mesh, wanted.clone())
            .context("cannot cut the layers being previewed")?;
        stack.cut = Some((wanted, sliced));
        Ok(())
    }
}
