use std::path::PathBuf;

use anyhow::Result;
use core_analysis::Measured;
use core_pipeline::{Tolerance, fold_group};
use core_raster::{LayerRuns, RasterSettings};
use core_slicer::{Layer, LayerPlan};
use rayon::prelude::*;

use crate::raster_report::RasterReport;

/// A report to fold every window's layers into with `stream_into`.
pub fn report_for(destination: PathBuf, settings: &RasterSettings, fold: Measured) -> RasterReport {
    RasterReport::new(destination, *settings, fold)
}

/// Rasterises `layers` and hands each one to `write`, in order, folding what it cost and
/// what it cures into `report`.
///
/// Layers are taken `window` at a time: the window is rasterised in parallel, folded in
/// order, which is where an island is taken out, compressed in parallel and written in
/// order, so the stack is never in memory at once. See ADR 0010 and 0066.
pub fn stream_into<T, E, W>(
    report: &mut RasterReport,
    layers: &[Layer],
    plan: &LayerPlan,
    tolerance: &Tolerance,
    window: usize,
    compress: E,
    mut write: W,
) -> Result<()>
where
    T: Send,
    E: Fn(&LayerRuns) -> Result<T> + Sync,
    W: FnMut(T) -> Result<()>,
{
    for group in layers.chunks(window.max(1)) {
        let folded = fold_group(
            group,
            &report.settings,
            plan,
            tolerance,
            &mut report.measured,
        )?;
        let compressed: Vec<T> = folded
            .par_iter()
            .map(|layer| compress(&layer.written()))
            .collect::<Result<_>>()?;
        for (layer, folded) in compressed.into_iter().zip(&folded) {
            write(layer)?;
            report.count(folded.overflow_px);
        }
    }
    Ok(())
}
