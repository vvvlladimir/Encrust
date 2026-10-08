//! What a run writes down as JSON: one report per model, from `slice`, `inspect` and each
//! model of a `batch`, and one over a whole batch.
//!
//! The shape is the report, not the code: every number a farm would want to gate on is a
//! field here rather than a line of prose to be grepped.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use core_analysis::{Measured, RiskKind, equivalent_disc_mm};
use core_geometry::Scalar;
use serde::Serialize;

use crate::estimate::Estimate;
use crate::pipeline::Outcome;
use crate::report::ImportReport;
use crate::slice_report::SliceReport;
use crate::stage::Part;

/// One model's run, from its file to what came out.
#[derive(Debug, Serialize)]
pub struct ModelReport {
    #[serde(flatten)]
    pub part: PartReport,
    /// Where the file went, or would have gone; absent for a model only inspected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub status: Status,
    /// The whole error chain, for a model that did not make it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicing: Option<Slicing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cured: Option<Cured>,
}

/// A plate of several models, or one out of a plate file or a project: one entry per
/// model for what was done to it, and the stack they make together.
#[derive(Debug, Serialize)]
pub struct PlateReport {
    pub input: Vec<String>,
    pub output: String,
    pub status: Status,
    pub seconds: f64,
    pub models: Vec<PartReport>,
    pub slicing: Slicing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cured: Option<Cured>,
}

/// What `estimate` found: the models, the stack, and with a printer the print.
#[derive(Debug, Serialize)]
pub struct EstimateReport {
    pub input: Vec<String>,
    pub status: Status,
    pub seconds: f64,
    pub models: Vec<PartReport>,
    pub slicing: Slicing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print: Option<PrintReport>,
}

/// What printing the stack takes. Absent without a printer to draw the masks for.
#[derive(Debug, Serialize)]
pub struct PrintReport {
    pub layers: u32,
    pub height_mm: f32,
    pub print_time_s: u32,
    /// What the masks cure, which is what the weight and the price are taken from.
    pub resin_mm3: f32,
    pub weight_g: f32,
    /// In `currency`; absent for a resin with no price.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f32>,
    pub currency: String,
    pub cured: Cured,
}

impl EstimateReport {
    pub fn of(inputs: &[PathBuf], parts: &[Part], estimate: &Estimate, took: Duration) -> Self {
        Self {
            input: inputs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            status: status_of(parts.iter().all(Part::is_clean) && estimate.is_clean()),
            seconds: took.as_secs_f64(),
            models: parts.iter().map(PartReport::of).collect(),
            slicing: Slicing::of(&estimate.slice),
            print: estimate.print.as_ref().map(|print| PrintReport {
                layers: print.job.layer_count(),
                height_mm: print.job.height_mm(),
                print_time_s: print.print_time_s(),
                resin_mm3: print.job.volume_mm3,
                weight_g: print.weight_g(),
                cost: print.cost(),
                currency: print.job.material.details.currency.clone(),
                cured: Cured::of(&print.measured),
            }),
        }
    }
}

/// What was done to one model on its way onto the plate.
#[derive(Debug, Serialize)]
pub struct PartReport {
    pub input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<Model>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair: Option<Repair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit: Option<Fit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oriented: Option<Oriented>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hollow: Option<Hollow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports: Option<Supports>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Written, and nothing found that would spoil the print.
    Ok,
    /// Written, but something is wrong with it: an open contour, resin with no way out,
    /// a model that does not fit the plate.
    Unclean,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct Model {
    pub vertices: usize,
    pub faces: usize,
    pub size_mm: [Scalar; 3],
    pub volume_mm3: Scalar,
}

#[derive(Debug, Serialize)]
pub struct Repair {
    pub vertices_merged: usize,
    pub faces_removed: usize,
    pub faces_flipped: usize,
    pub closed: bool,
    pub boundary_edges: usize,
    pub shells: usize,
}

#[derive(Debug, Serialize)]
pub struct Fit {
    pub printer: String,
    pub fits: bool,
    pub overflow_mm: [Scalar; 3],
    /// How far the model reaches past the envelope where it stands, per axis.
    pub outside_mm: [Scalar; 3],
}

#[derive(Debug, Serialize)]
pub struct Oriented {
    pub degrees: Scalar,
    /// The angles `--rotate X,Y,Z` repeats the turn with, degrees.
    pub rotate_deg: [Scalar; 3],
    pub peak_section_mm2: Scalar,
    pub overhang_mm2: Scalar,
}

#[derive(Debug, Serialize)]
pub struct Hollow {
    pub wall_mm: Scalar,
    pub cavity_mm3: Scalar,
}

#[derive(Debug, Serialize)]
pub struct Supports {
    /// How far the model was stood off the plate to make room for them, millimetres.
    pub lifted_mm: Scalar,
    pub contacts: usize,
    pub standing: usize,
    pub trunks: usize,
    pub no_room_for: usize,
}

#[derive(Debug, Serialize)]
pub struct Slicing {
    pub layers: usize,
    pub layer_height_mm: Scalar,
    /// Pockets the drainage scan found with no way out, and what they hold.
    pub trapped_pockets: usize,
    pub trapped_mm3: Scalar,
    pub open_contours: usize,
}

/// What the written masks cure, and the layers that pull hardest on the film. Layers
/// count from one, as a printer's screen does.
#[derive(Debug, Serialize)]
pub struct Cured {
    pub resin_mm3: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardest_pull_layer: Option<usize>,
    /// The diameter of a disc that pulls as hard as that layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardest_pull_disc_mm: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widest_step_layer: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widest_step_mm2: Option<f32>,
    /// Pieces cured over nothing, necks pulled past their limit, layers pulling the film
    /// past its, and the parts taken out of the file as islands.
    pub islands: usize,
    pub levers: usize,
    pub peels: usize,
    pub islands_removed: usize,
    pub islands_removed_layers: usize,
    /// The layer the print most likely fails on: the first island, or the worst lever.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub likely_fails_layer: Option<usize>,
}

impl Cured {
    pub fn of(measured: &Measured) -> Self {
        let pull = measured.hardest_pull();
        let step = measured.largest_growth();
        let risks = measured.risks();
        let count =
            |kind: fn(&RiskKind) -> bool| risks.iter().filter(|risk| kind(&risk.kind)).count();
        Self {
            resin_mm3: measured.volume_mm3(),
            hardest_pull_layer: pull.map(|peak| peak.layer + 1),
            hardest_pull_disc_mm: pull.map(|peak| equivalent_disc_mm(peak.value)),
            widest_step_layer: step.map(|peak| peak.layer + 1),
            widest_step_mm2: step.map(|peak| peak.value),
            islands: count(|kind| matches!(kind, RiskKind::Island { .. })),
            levers: count(|kind| matches!(kind, RiskKind::Lever { .. })),
            peels: count(|kind| matches!(kind, RiskKind::Peel { .. })),
            islands_removed: measured.removed_islands(),
            islands_removed_layers: measured.removed_layers(),
            likely_fails_layer: measured.worst().map(|risk| risk.layer + 1),
        }
    }
}

impl PartReport {
    pub fn of(part: &Part) -> Self {
        let import = part.import.as_ref();
        Self {
            input: part.input.display().to_string(),
            model: import.map(model_of),
            repair: import.map(repair_of),
            fit: import.and_then(fit_of),
            oriented: part.oriented.as_ref().map(|found| Oriented {
                degrees: found.degrees(),
                rotate_deg: found.rotate_degrees().to_array(),
                peak_section_mm2: found.peak_mm2,
                overhang_mm2: found.overhang_mm2,
            }),
            hollow: part.hollow.as_ref().map(|report| Hollow {
                wall_mm: report.wall().0,
                cavity_mm3: report.cavity_mm3(),
            }),
            supports: part.supports.as_ref().map(|report| Supports {
                lifted_mm: report.lifted_mm,
                contacts: report.contacts,
                standing: report.standing,
                trunks: report.trees,
                no_room_for: report.unsupported(),
            }),
        }
    }

    fn named(input: &Path) -> Self {
        Self {
            input: input.display().to_string(),
            model: None,
            repair: None,
            fit: None,
            oriented: None,
            hollow: None,
            supports: None,
        }
    }
}

impl Slicing {
    fn of(slice: &SliceReport) -> Self {
        Self {
            layers: slice.layer_count(),
            layer_height_mm: slice.settings.layer_height,
            trapped_pockets: slice.trapped().len(),
            trapped_mm3: slice
                .trapped()
                .iter()
                .map(|pocket| pocket.volume_mm3)
                .sum::<Scalar>()
                .max(0.0),
            open_contours: slice.open_contours(),
        }
    }
}

fn status_of(clean: bool) -> Status {
    if clean { Status::Ok } else { Status::Unclean }
}

impl PlateReport {
    pub fn sliced(inputs: &[PathBuf], outcome: &Outcome, took: Duration) -> Self {
        Self {
            input: inputs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            output: outcome.output.display().to_string(),
            status: status_of(outcome.is_clean()),
            seconds: took.as_secs_f64(),
            models: outcome.parts.iter().map(PartReport::of).collect(),
            slicing: Slicing::of(&outcome.slice),
            cured: outcome
                .raster
                .as_ref()
                .map(|raster| Cured::of(&raster.measured)),
        }
    }
}

impl ModelReport {
    /// A run of one model, whose outcome has exactly the one part.
    pub fn sliced(outcome: &Outcome, took: Duration) -> Self {
        Self {
            part: outcome
                .parts
                .first()
                .map_or_else(|| PartReport::named(&outcome.output), PartReport::of),
            output: Some(outcome.output.display().to_string()),
            status: status_of(outcome.is_clean()),
            error: None,
            seconds: took.as_secs_f64(),
            slicing: Some(Slicing::of(&outcome.slice)),
            cured: outcome
                .raster
                .as_ref()
                .map(|raster| Cured::of(&raster.measured)),
        }
    }

    /// A model that was only looked at, for `inspect`.
    pub fn inspected(import: &ImportReport, took: Duration) -> Self {
        Self {
            part: PartReport {
                model: Some(model_of(import)),
                repair: Some(repair_of(import)),
                fit: fit_of(import),
                ..PartReport::named(&import.path)
            },
            output: None,
            status: status_of(import.is_clean()),
            error: None,
            seconds: took.as_secs_f64(),
            slicing: None,
            cured: None,
        }
    }

    pub fn failed(input: &Path, output: &Path, error: &anyhow::Error, took: Duration) -> Self {
        let causes: Vec<String> = error.chain().map(ToString::to_string).collect();
        Self {
            part: PartReport::named(input),
            output: Some(output.display().to_string()),
            status: Status::Failed,
            error: Some(causes.join(": ")),
            seconds: took.as_secs_f64(),
            slicing: None,
            cured: None,
        }
    }

    /// The one line this model gets in the terminal.
    pub fn line(&self) -> String {
        let input = &self.part.input;
        let name = Path::new(input)
            .file_name()
            .map_or_else(|| input.clone(), |name| name.to_string_lossy().into());
        match self.status {
            Status::Failed => format!(
                "  failed   {name}  {}",
                self.error.as_deref().unwrap_or("no reason given")
            ),
            status => {
                let mark = if status == Status::Ok {
                    "ok"
                } else {
                    "unclean"
                };
                let what = match (&self.slicing, &self.part.model) {
                    (Some(slicing), _) => match &self.cured {
                        Some(cured) => format!(
                            "{} layers, {:.1} ml",
                            slicing.layers,
                            cured.resin_mm3 / 1000.0
                        ),
                        None => format!("{} layers, no panel to cure them on", slicing.layers),
                    },
                    (None, Some(model)) => format!("{} triangles, not sliced", model.faces),
                    (None, None) => "nothing to report".to_owned(),
                };
                format!("  {mark:<8} {name}  {what}, {}", took(self.seconds))
            }
        }
    }
}

/// How long one model took, in the unit that says something: a small model is sliced in
/// milliseconds, and a column of `0.0 s` says nothing at all.
fn took(seconds: f64) -> String {
    if seconds < 1.0 {
        return format!("{:.0} ms", seconds * 1000.0);
    }
    format!("{seconds:.1} s")
}

/// Every model of one run, and what the run came to.
#[derive(Debug, Serialize)]
pub struct Summary {
    pub input: String,
    pub output: String,
    pub models: usize,
    pub ok: usize,
    pub unclean: usize,
    pub failed: usize,
    pub seconds: f64,
    /// What the models that would print take, cubic millimetres.
    pub resin_mm3: Scalar,
    pub reports: Vec<ModelReport>,
}

impl Summary {
    pub fn of(input: &Path, output: &Path, reports: Vec<ModelReport>) -> Self {
        let count = |wanted: Status| reports.iter().filter(|r| r.status == wanted).count();
        Self {
            input: input.display().to_string(),
            output: output.display().to_string(),
            models: reports.len(),
            ok: count(Status::Ok),
            unclean: count(Status::Unclean),
            failed: count(Status::Failed),
            // Wall time is not the sum when models run at once, so this is the work done
            // rather than how long the run took.
            seconds: reports.iter().map(|report| report.seconds).sum(),
            // Only what would print: a model that does not fit was cut as it stands and
            // clipped by the panel, so its resin is not resin the batch will use. Rust
            // sums floats from -0.0, so a run that sliced nothing would report a negative
            // volume.
            resin_mm3: reports
                .iter()
                .filter(|report| report.status == Status::Ok)
                .filter_map(|report| report.cured.as_ref())
                .map(|cured| Scalar::from(cured.resin_mm3))
                .sum::<Scalar>()
                .max(0.0),
            reports,
        }
    }

    pub fn line(&self, path: &Path) -> String {
        let over = match self.ok == self.models {
            true => String::new(),
            false => format!(" over the {} that would print", self.ok),
        };
        format!(
            "{} ok, {} unclean, {} failed; {:.1} ml of resin{over}. Report: {}",
            self.ok,
            self.unclean,
            self.failed,
            self.resin_mm3 / 1000.0,
            path.display()
        )
    }
}

fn model_of(import: &ImportReport) -> Model {
    let size = import.stats.size();
    Model {
        vertices: import.stats.vertices,
        faces: import.stats.faces,
        size_mm: [size.x, size.y, size.z],
        volume_mm3: import.stats.volume,
    }
}

fn repair_of(import: &ImportReport) -> Repair {
    Repair {
        vertices_merged: import.welded.vertices_merged,
        faces_removed: import.welded.faces_removed(),
        faces_flipped: import
            .orientation
            .as_ref()
            .map_or(0, |found| found.flipped_faces),
        closed: import
            .diagnostics
            .as_ref()
            .is_some_and(core_geometry::MeshDiagnostics::is_closed),
        boundary_edges: import
            .diagnostics
            .as_ref()
            .map_or(0, |found| found.boundary_edges),
        shells: import.diagnostics.as_ref().map_or(0, |found| found.shells),
    }
}

fn fit_of(import: &ImportReport) -> Option<Fit> {
    let fit = import.fit.as_ref()?;
    Some(Fit {
        printer: fit.printer.clone(),
        fits: fit.fits(),
        overflow_mm: [fit.overflow.x, fit.overflow.y, fit.overflow.z],
        outside_mm: [fit.outside.x, fit.outside.y, fit.outside.z],
    })
}

pub fn write<T: Serialize>(path: &Path, report: &T) -> Result<()> {
    let text = serde_json::to_vec_pretty(report)?;
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model whose file was written, whose masks cure `resin_mm3`.
    fn reported(name: &str, status: Status, resin_mm3: f32) -> ModelReport {
        ModelReport {
            part: PartReport::named(Path::new(name)),
            output: Some(name.to_owned()),
            status,
            error: None,
            seconds: 0.5,
            slicing: Some(Slicing {
                layers: 10,
                layer_height_mm: 0.05,
                trapped_pockets: 0,
                trapped_mm3: 0.0,
                open_contours: 0,
            }),
            cured: Some(Cured {
                resin_mm3,
                hardest_pull_layer: None,
                hardest_pull_disc_mm: None,
                widest_step_layer: None,
                widest_step_mm2: None,
                islands: 0,
                levers: 0,
                peels: 0,
                islands_removed: 0,
                islands_removed_layers: 0,
                likely_fails_layer: None,
            }),
        }
    }

    #[test]
    fn the_batch_total_leaves_out_a_model_that_would_not_print() {
        let summary = Summary::of(
            Path::new("in"),
            Path::new("out"),
            vec![
                reported("small.stl", Status::Ok, 1_000.0),
                reported("too-big.stl", Status::Unclean, 600_000.0),
            ],
        );

        assert!(
            (summary.resin_mm3 - 1_000.0).abs() < 1.0e-3,
            "a model clipped by the panel does not spend its resin: {}",
            summary.resin_mm3
        );
        assert!(
            summary
                .line(Path::new("out/batch.json"))
                .contains("1.0 ml of resin over the 1 that would print"),
            "{}",
            summary.line(Path::new("out/batch.json"))
        );
    }

    #[test]
    fn a_model_sliced_in_milliseconds_is_timed_in_them() {
        assert_eq!(took(0.017), "17 ms", "a column of 0.0 s says nothing");
        assert_eq!(took(1.0), "1.0 s");
        assert_eq!(took(123.45), "123.5 s");
    }
}
