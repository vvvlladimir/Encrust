//! The progress bar on stderr, and the observer that feeds it and asks whether to stop.

use core_pipeline::Observer;
use core_slicer::Sliced;
use indicatif::{ProgressBar, ProgressStyle};

use crate::exit::Stop;
use crate::slice_report::SliceReport;

/// A bar over `what`, which the caller only builds when stderr is a terminal and nobody
/// asked for quiet or JSON.
pub fn bar(what: &str) -> ProgressBar {
    let template = format!("{{bar:40}} {{pos}}/{{len}} {what}, {{eta}} left");
    let style =
        ProgressStyle::with_template(&template).unwrap_or_else(|_| ProgressStyle::default_bar());
    ProgressBar::new(0).with_style(style)
}

/// What one model's run hears while its file is written: each window into the slice report,
/// each layer onto the bar, and Ctrl-C through `stop`.
pub struct Watching<'a> {
    pub report: &'a mut SliceReport,
    pub bar: Option<ProgressBar>,
    pub stop: &'a Stop,
}

impl Observer for Watching<'_> {
    fn window(&mut self, sliced: &Sliced) {
        self.report.absorb(sliced);
    }

    fn layers(&mut self, done: usize, total: usize) {
        if let Some(bar) = &self.bar {
            bar.set_length(total as u64);
            bar.set_position(done as u64);
        }
    }

    fn cancelled(&self) -> bool {
        self.stop.requested()
    }
}

impl Drop for Watching<'_> {
    fn drop(&mut self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }
}
