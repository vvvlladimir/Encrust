//! What a run writes down as JSON: one report per model, from `slice`, `inspect` and each
//! model of a `batch`, and one over a whole batch.
//!
//! The shape is the report, not the code: every number a farm would want to gate on is a
//! field here rather than a line of prose to be grepped.

use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use core_analysis::{Measured, RiskKind, equivalent_disc_mm};
use core_geometry::Scalar;
use serde::Serialize;

use crate::pipeline::Outcome;
use crate::report::ImportReport;

/// One model's run, from its file to what came out.
#[derive(Debug, Serialize)]
pub struct ModelReport {
    pub input: String,
    /// Where the file went, or would have gone; absent for a model only inspected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub status: Status,
    /// The whole error chain, for a model that did not make it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub seconds: f64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicing: Option<Slicing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cured: Option<Cured>,
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
}

#[derive(Debug, Serialize)]
pub struct Oriented {
    pub degrees: Scalar,
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
    pub contacts: usize,
    pub standing: usize,
    pub trunks: usize,
    pub no_room_for: usize,
}

#[derive(Debug, Serialize)]
pub struct Slicing {
    pub layers: usize,
    pub layer_height_mm: Scalar,
    pub resin_mm3: Scalar,
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
    /// past its, and islands taken out of the file.
    pub islands: usize,
    pub levers: usize,
    pub peels: usize,
    pub islands_removed: usize,
    /// The layer the print most likely fails on: the first island, or the worst lever.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub likely_fails_layer: Option<usize>,
}

impl Cured {
    fn of(measured: &Measured) -> Self {
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
            likely_fails_layer: measured.worst().map(|risk| risk.layer + 1),
        }
    }
}

impl ModelReport {
    pub fn sliced(outcome: &Outcome, took: Duration) -> Self {
        let status = if outcome.is_clean() {
            Status::Ok
        } else {
            Status::Unclean
        };
        Self {
            input: outcome.input.display().to_string(),
            output: Some(outcome.output.display().to_string()),
            status,
            error: None,
            seconds: took.as_secs_f64(),
            model: Some(model_of(&outcome.import)),
            repair: Some(repair_of(&outcome.import)),
            fit: fit_of(&outcome.import),
            oriented: outcome.oriented.as_ref().map(|found| Oriented {
                degrees: found.degrees,
                peak_section_mm2: found.peak_mm2,
                overhang_mm2: found.overhang_mm2,
            }),
            hollow: outcome.hollow.as_ref().map(|report| Hollow {
                wall_mm: report.wall().0,
                cavity_mm3: report.cavity_mm3(),
            }),
            supports: outcome.supports.as_ref().map(|report| Supports {
                contacts: report.contacts,
                standing: report.standing,
                trunks: report.trees,
                no_room_for: report.unsupported(),
            }),
            slicing: Some(Slicing {
                layers: outcome.slice.layer_count(),
                layer_height_mm: outcome.slice.settings.layer_height,
                resin_mm3: outcome.slice.resin_volume_mm3(),
                trapped_pockets: outcome.slice.trapped().len(),
                trapped_mm3: outcome
                    .slice
                    .trapped()
                    .iter()
                    .map(|pocket| pocket.volume_mm3)
                    .sum(),
                open_contours: outcome.slice.open_contours(),
            }),
            cured: outcome
                .raster
                .as_ref()
                .map(|raster| Cured::of(&raster.measured)),
        }
    }

    /// A model that was only looked at, for `inspect`.
    pub fn inspected(import: &ImportReport, took: Duration) -> Self {
        let status = if import.is_clean() {
            Status::Ok
        } else {
            Status::Unclean
        };
        Self {
            input: import.path.display().to_string(),
            output: None,
            status,
            error: None,
            seconds: took.as_secs_f64(),
            model: Some(model_of(import)),
            repair: Some(repair_of(import)),
            fit: fit_of(import),
            oriented: None,
            hollow: None,
            supports: None,
            slicing: None,
            cured: None,
        }
    }

    pub fn failed(input: &Path, output: &Path, error: &anyhow::Error, took: Duration) -> Self {
        let causes: Vec<String> = error.chain().map(ToString::to_string).collect();
        Self {
            input: input.display().to_string(),
            output: Some(output.display().to_string()),
            status: Status::Failed,
            error: Some(causes.join(": ")),
            seconds: took.as_secs_f64(),
            model: None,
            repair: None,
            fit: None,
            oriented: None,
            hollow: None,
            supports: None,
            slicing: None,
            cured: None,
        }
    }

    /// The one line this model gets in the terminal.
    pub fn line(&self) -> String {
        let name = Path::new(&self.input)
            .file_name()
            .map_or_else(|| self.input.clone(), |name| name.to_string_lossy().into());
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
                let what = match (&self.slicing, &self.model) {
                    (Some(slicing), _) => format!(
                        "{} layers, {:.1} ml",
                        slicing.layers,
                        slicing.resin_mm3 / 1000.0
                    ),
                    (None, Some(model)) => format!("{} triangles, not sliced", model.faces),
                    (None, None) => "nothing to report".to_owned(),
                };
                format!("  {mark:<8} {name}  {what}, {:.1} s", self.seconds)
            }
        }
    }
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
            // Rust sums floats from -0.0, so a run that sliced nothing would report a
            // negative volume.
            resin_mm3: reports
                .iter()
                .filter_map(|report| report.slicing.as_ref())
                .map(|slicing| slicing.resin_mm3)
                .sum::<Scalar>()
                .max(0.0),
            reports,
        }
    }

    pub fn line(&self, path: &Path) -> String {
        format!(
            "{} ok, {} unclean, {} failed; {:.1} ml of resin. Report: {}",
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
    })
}

pub fn write<T: Serialize>(path: &Path, report: &T) -> Result<()> {
    let text = serde_json::to_vec_pretty(report)?;
    std::fs::write(path, text)?;
    Ok(())
}
