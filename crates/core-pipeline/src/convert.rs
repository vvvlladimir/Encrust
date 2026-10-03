use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use core_format::{
    ExposurePlan, ExposureRange, LayerSink, OpenFile, PrintJob, SlicedFile, WRITE_BUFFER_BYTES,
    WriteSeek,
};
use core_raster::{LayerRuns, Run, Shading};
use core_slicer::LayerPlan;
use printer_profiles::{MaterialProfile, PrinterProfile};
use rayon::prelude::*;

use crate::error::PipelineError;
use crate::format::SlicedFormat;
use crate::panel::{PanelOverrides, raster_settings};
use crate::write::{Destination, Feed, Observer};

/// A sliced file read back and written again in another container (ADR 0180).
pub struct Converting<'a> {
    pub format: SlicedFormat,
    /// What the new file is called, without its directory or extension.
    pub name: &'a str,
    /// The machine the new file is for. Its panel must be the one the masks were drawn
    /// for: a mask is never resampled.
    pub printer: &'a PrinterProfile,
    /// Where the lifts, waits and price come from. The layer height and every exposure are
    /// the file's own.
    pub material: &'a MaterialProfile,
    /// When the new file is made, seconds since the Unix epoch.
    pub created_unix_s: u64,
}

/// What a conversion wrote.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Converted {
    pub layers: usize,
    /// What the masks cure, cubic millimetres.
    pub volume_mm3: f32,
}

/// Rewrites every layer of `source` into `sink` as `converting` asks. `None` means the
/// observer cancelled it, and what reached `sink` is a file stopped half way.
pub fn convert_to(
    source: &mut impl OpenFile,
    converting: &Converting<'_>,
    sink: &mut dyn WriteSeek,
    observer: &mut dyn Observer,
) -> Result<Option<Converted>, PipelineError> {
    let job = job_of(source.facts(), converting)?;
    let destination = Destination {
        format: converting.format,
        name: converting.name,
        job: &job,
    };
    let pixel_mm2 = job.raster.pitch.x * job.raster.pitch.y;
    destination.write(
        sink,
        &mut Layers {
            source,
            plan: &job.plan,
            pixel_mm2,
        },
        observer,
    )
}

/// The same into a file at `path`, which is removed again when the run fails or is
/// cancelled.
pub fn convert(
    source: &mut impl OpenFile,
    converting: &Converting<'_>,
    path: &Path,
    observer: &mut dyn Observer,
) -> Result<Option<Converted>, PipelineError> {
    let file = File::create(path).map_err(|source| PipelineError::Create {
        path: path.to_owned(),
        source,
    })?;
    let mut buffered = BufWriter::with_capacity(WRITE_BUFFER_BYTES, file);
    let converted = convert_to(source, converting, &mut buffered, observer);
    if !matches!(converted, Ok(Some(_))) {
        drop(buffered);
        let _ = std::fs::remove_file(path);
    }
    converted
}

/// The header the new file carries: the file's stack and exposures on the new machine,
/// with the resin's motion around them.
fn job_of(facts: &SlicedFile, converting: &Converting<'_>) -> Result<PrintJob, PipelineError> {
    let printer = converting.printer;
    let display = &printer.display;
    if (display.width_px, display.height_px) != (facts.width_px, facts.height_px) {
        return Err(PipelineError::PanelMismatch {
            file_px: (facts.width_px, facts.height_px),
            printer_px: (display.width_px, display.height_px),
        });
    }
    let plan = plan_of(facts);
    if !plan.is_uniform() {
        // TODO(step-B6): carry a stack of varying heights, which needs an exposure per
        // layer rather than bands the header rescales by thickness.
        return Err(PipelineError::VaryingHeights);
    }

    let mut material = converting.material.clone();
    material.layer_height_mm = plan.nominal_thickness();
    material.exposure_s = facts.exposure_s;
    material.bottom_exposure_s = facts.bottom_exposure_s;
    material.bottom_layers = facts.bottom_layers;
    // Whatever ramp the file had is in its per-layer exposures, which become bands.
    material.transition_layers = 0;

    let shading = if facts.grey_steps <= 2 {
        Shading::Binary
    } else {
        Shading::Coverage
    };
    Ok(PrintJob {
        printer: printer.clone(),
        raster: raster_settings(
            printer,
            PanelOverrides {
                shading,
                ..PanelOverrides::default()
            },
        ),
        exposure: bands_of(facts, &plan),
        plan,
        material,
        volume_mm3: facts.volume_mm3.unwrap_or(0.0),
        thumbnail: None,
        created_unix_s: converting.created_unix_s,
    })
}

/// The layers as the file places them, or as many of the header's height where the
/// container records no height per layer.
fn plan_of(facts: &SlicedFile) -> LayerPlan {
    let mut bounds = vec![0.0];
    for entry in &facts.layers {
        let below = bounds.last().copied().unwrap_or(0.0);
        if entry.z_mm <= below {
            return LayerPlan::of_count(facts.layer_height_mm, facts.layers.len());
        }
        bounds.push(entry.z_mm);
    }
    let ceiling = bounds.last().copied().unwrap_or(0.0);
    LayerPlan::from_bounds(bounds, ceiling)
}

/// One band per run of layers above the bottom block exposed differently from the header,
/// each reaching half a layer either side of the tops it covers.
fn bands_of(facts: &SlicedFile, plan: &LayerPlan) -> ExposurePlan {
    let mut bands: Vec<ExposureRange> = Vec::new();
    let half = plan.nominal_thickness() / 2.0;
    let differs =
        |exposure_s: f32| exposure_s > 0.0 && (exposure_s - facts.exposure_s).abs() > 1e-3;
    for (index, entry) in facts
        .layers
        .iter()
        .enumerate()
        .skip(facts.bottom_layers as usize)
    {
        let Some(top) = plan.top_of(index) else { break };
        if !differs(entry.exposure_s) {
            continue;
        }
        match bands.last_mut() {
            Some(band)
                if (band.exposure_s - entry.exposure_s).abs() <= 1e-3
                    && (band.to_mm - (top - half)).abs() < half / 2.0 =>
            {
                band.to_mm = top + half;
            }
            _ => bands.push(ExposureRange::new(top - half, top + half, entry.exposure_s)),
        }
    }
    ExposurePlan::new(bands)
}

/// Layers read back one group at a time, encoded in parallel and pushed in order.
struct Layers<'a, O> {
    source: &'a mut O,
    plan: &'a LayerPlan,
    pixel_mm2: f32,
}

impl<O: OpenFile> Feed for Layers<'_, O> {
    type Done = Converted;

    fn feed<S: LayerSink>(
        &mut self,
        sink: &mut S,
        name: &str,
        observer: &mut dyn Observer,
    ) -> Result<Option<Converted>, PipelineError> {
        let facts = self.source.facts();
        let (width_px, height_px) = (facts.width_px, facts.height_px);
        let total = facts.layers.len();
        let group = rayon::current_num_threads().max(1);
        let mut converted = Converted {
            layers: 0,
            volume_mm3: 0.0,
        };

        for start in (0..total).step_by(group) {
            if observer.cancelled() {
                return Ok(None);
            }
            let read = (start..total.min(start + group))
                .map(|index| {
                    let runs = self.source.layer(index as u32).map_err(|source| {
                        PipelineError::Decode {
                            layer: index,
                            source,
                        }
                    })?;
                    Ok((index, runs))
                })
                .collect::<Result<Vec<_>, PipelineError>>()?;
            let encoded: Vec<_> = read
                .par_iter()
                .map(|(index, runs)| {
                    let thickness = self.plan.thickness_of(*index).unwrap_or(0.0);
                    let layer = layer_of(width_px, height_px, runs);
                    (
                        S::encode(&layer),
                        cured_mm3(runs, self.pixel_mm2 * thickness),
                    )
                })
                .collect();
            for (layer, volume_mm3) in encoded {
                sink.push(layer).map_err(|source| PipelineError::Write {
                    name: name.to_owned(),
                    source,
                })?;
                converted.layers += 1;
                converted.volume_mm3 += volume_mm3;
            }
            observer.layers(converted.layers, total);
        }
        Ok(Some(converted))
    }

    fn volume_mm3(done: &Converted) -> f32 {
        done.volume_mm3
    }
}

fn layer_of(width_px: u32, height_px: u32, runs: &[Run]) -> LayerRuns {
    let mut layer = LayerRuns::builder(width_px, height_px);
    for run in runs {
        layer.push(run.length, run.value);
    }
    layer.finish()
}

/// What a layer cures: each pixel's grey as a share of a full voxel of `voxel_mm3`.
fn cured_mm3(runs: &[Run], voxel_mm3: f32) -> f32 {
    let lit: f32 = runs
        .iter()
        .map(|run| run.length as f32 * f32::from(run.value) / 255.0)
        .sum();
    lit * voxel_mm3
}
