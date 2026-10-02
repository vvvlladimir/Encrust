use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use core_analysis::Measured;
use core_pipeline::Tolerance;
use core_raster::{LayerRuns, RasterSettings};

use crate::raster_report::RasterReport;
use crate::slice_report::SliceReport;
use crate::slicing::Plan;
use crate::stack::{report_for, stream_into};

/// Slices and rasterises into `directory` as `layer_NNNN.png`, a window at a time.
pub fn write_stack(
    plan: &Plan,
    slice: &mut SliceReport,
    settings: &RasterSettings,
    directory: &Path,
    tolerance: &Tolerance,
    window: usize,
    fold: Measured,
) -> Result<RasterReport> {
    fs::create_dir_all(directory)
        .with_context(|| format!("cannot create {}", directory.display()))?;

    let mut index = 0;
    let mut report = report_for(directory.to_owned(), settings, fold);
    plan.stream(|sliced| {
        slice.absorb(sliced);
        stream_into(
            &mut report,
            &sliced.layers,
            plan.layers(),
            tolerance,
            window,
            to_png,
            |png| {
                let path = layer_path(directory, index);
                index += 1;
                fs::write(&path, &png).with_context(|| format!("cannot write {}", path.display()))
            },
        )
    })?;
    Ok(report)
}

/// Expands a layer into the same 8-bit greyscale PNG the archive containers hold one of
/// per layer, which is why the encoding lives beside what every format shares.
fn to_png(layer: &LayerRuns) -> Result<Vec<u8>> {
    core_format::encode_grey(layer).context("cannot encode a layer as a PNG")
}

fn layer_path(directory: &Path, index: usize) -> PathBuf {
    directory.join(format!("layer_{index:04}.png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_numbers_are_padded_so_they_sort() {
        assert_eq!(
            layer_path(Path::new("out"), 7),
            PathBuf::from("out/layer_0007.png")
        );
    }
}
