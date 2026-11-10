//! The stack the window cuts: for Preview and Check as they are opened, and for the quick
//! check in the background once the plate stands still (ADR 0419).

use std::time::Duration;

use web_time::Instant;

use super::SlicerApp;
use crate::i18n::tr;
use crate::preview::stack_fingerprint;
use crate::status::Status;
use crate::workspace::{Mode, Tool};

/// How often a quick check under way is looked in on: it says nothing until it ends.
const QUICK_POLL: Duration = Duration::from_millis(250);

impl SlicerApp {
    /// Cuts the plate as it is for Preview or Check when the stack is stale or missing, else
    /// measures it; with nothing to cut it stays empty and the transport says why.
    pub(super) fn refresh_preview(&mut self) {
        let reads_the_stack = self.mode == Mode::Preview || self.tool == Tool::Check;
        if !reads_the_stack {
            return;
        }
        if self.machine.preview.is_building() {
            self.machine.preview.show_build();
            return;
        }
        // A recipe holding the plate cuts and measures for its own steps (ADR 0419).
        if self.tools.prepare.is_holding() {
            return;
        }
        // A stack cut before the supports land would be cut without them.
        let plate = self.doc.scene.active_plate();
        if self.doc.scene.supports_pending(plate) {
            return;
        }
        let fingerprint = stack_fingerprint(&self.doc.scene, self.machine.slicing.cutting());
        let wanted =
            self.machine.preview.layer_count() == 0 || self.machine.preview.is_stale(fingerprint);
        let Some(settings) = self.machine.slicing.raster_settings() else {
            return;
        };
        if !wanted {
            // Preview says what the print takes, so the stack is measured as it is written.
            self.machine
                .preview
                .measure(&settings, self.machine.slicing.fold());
            return;
        }
        if !self.doc.scene.has_printable(self.doc.scene.active_plate()) {
            return;
        }

        let started = self
            .machine
            .preview
            .build(&self.doc.scene, self.machine.slicing.cutting());
        if let Err(error) = started {
            self.machine.status = Status::failed(&error.context(tr!("app-cannot-build-preview")));
        }
    }

    /// Cuts and quickly checks the plate once it has stood still for `quick::SETTLE` and no
    /// recipe holds it (ADR 0419); answers how long until it looks again, `None` for no need.
    pub(super) fn check_in_background(&mut self, now: Instant) -> Option<Duration> {
        if self.tools.prepare.is_holding() {
            self.machine.preview.stop_quick();
            return None;
        }
        let polling = self.machine.preview.poll_quick().then_some(QUICK_POLL);
        let scene = &self.doc.scene;
        let plate = scene.active_plate();
        if scene.supports_pending(plate) || !scene.has_printable(plate) {
            return polling;
        }
        let settings = self.machine.slicing.raster_settings()?;
        let cutting = self.machine.slicing.cutting();
        let fingerprint = stack_fingerprint(scene, cutting);
        let preview = &mut self.machine.preview;
        preview.note_plate(fingerprint, now);
        let wait = preview.settles_in(now)?;
        if !wait.is_zero() {
            return Some(wait);
        }
        if preview.is_building() {
            return polling;
        }
        if preview.layer_count() == 0 || preview.is_stale(fingerprint) {
            if preview.try_cut_for(fingerprint) {
                // A plate that cannot be cut is said so by Preview and Check, not here.
                let _ = preview.build_in_background(scene, cutting);
            }
            return polling;
        }
        preview.measure_quick(&settings, self.machine.slicing.fold());
        preview.poll_quick().then_some(QUICK_POLL)
    }
}

#[cfg(test)]
mod tests;
