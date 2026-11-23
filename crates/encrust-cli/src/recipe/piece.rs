//! One model as a recipe's steps leave it: the object of the plate's document it is, what was
//! measured on it, and what each step decided (ADR 0472).

use std::path::PathBuf;

use anyhow::{Context, Result};
use core_edit::ObjectId;
use core_engine::{ANALYSIS_LAYER_MM, Drained, Model, bake, trapped_in};
use core_geometry::{MeshDiagnostics, Scalar, Transform, diagnose};
use core_recipe::{Metric, Metrics, Plan, Value, measure_planned};
use printer_profiles::{Compensation, PrinterProfile};

use super::prepared::Prepared;
use super::report::StepRun;
use crate::exit::{Cancelled, Stop};
use crate::hollowing::HollowReport;
use crate::pipeline::OrientSummary;
use crate::report::{FitCheck, ImportReport};
use crate::stage::{Finished, Found, Shaping, Staging};
use crate::supports::SupportReport;

/// One model on its way through a recipe: an object of the plate the run edits.
pub(super) struct Piece {
    pub input: PathBuf,
    /// What the flags and a plate file gave this model, which win over a step's settings.
    pub shaping: Shaping,
    /// Whether a supports step of the recipe stands it, which its turn and holes are judged on.
    pub stands: bool,
    pub id: ObjectId,
    /// The user turned it, or a search did, so a search now would undo a turn.
    pub turned: bool,
    /// The last orientation set it on the plate, needing no supports this way up.
    pub seated: bool,
    /// The profile its last supports step stood it on, by name.
    pub supported_by: Option<String>,
    pub import: ImportReport,
    pub oriented: Option<OrientSummary>,
    pub hollow: Option<HollowReport>,
    pub drains: Option<Drained>,
    pub supports: Option<SupportReport>,
    /// What steps measured on it since import, over what `measure` reads off it.
    pub measured: Metrics,
    pub prepared: Prepared,
    pub steps: Vec<StepRun>,
}

impl Piece {
    pub(super) fn new(
        (input, shaping, stands): (PathBuf, Shaping, bool),
        id: ObjectId,
        found: Found,
    ) -> Self {
        let (import, oriented) = found;
        let turned = shaping.import.transform.rotate.is_some() || oriented.is_some();
        Self {
            input,
            seated: oriented.as_ref().is_some_and(|turn| turn.on_plate),
            prepared: Prepared {
                repaired: import.mended.is_some(),
                oriented: oriented.is_some(),
                ..Prepared::default()
            },
            shaping,
            stands,
            id,
            turned,
            supported_by: None,
            import,
            oriented,
            hollow: None,
            drains: None,
            supports: None,
            measured: Metrics::default(),
            steps: Vec::new(),
        }
    }

    /// Every variable of the model as it stands on `staging`, `saves_ml` at `wall_mm`, with
    /// what steps measured on it since.
    pub(super) fn metrics(
        &self,
        staging: &Staging,
        plan: &Plan,
        printer: Option<&PrinterProfile>,
    ) -> Metrics {
        let mesh = staging.mesh(self.id).unwrap_or_default();
        let diagnostics = self.diagnostics(&mesh);
        let fits = match (staging.standing_bounds(self.id), printer) {
            (Some(bounds), Some(printer)) => FitCheck::of(&bounds, printer).fit.fits(),
            _ => true,
        };
        let model = core_recipe::Model {
            mesh: &mesh,
            transform: &Transform::default(),
            diagnostics: &diagnostics,
            turned: self.turned,
            hollow: staging.hollowed(self.id),
            fits,
        };
        let wall_mm = self.shaping.hollow.thickness_mm;
        measure_planned(plan, &model, wall_mm, &self.measured)
    }

    fn diagnostics(&self, mesh: &core_geometry::Mesh) -> MeshDiagnostics {
        self.import
            .diagnostics
            .clone()
            .unwrap_or_else(|| diagnose(mesh))
    }

    /// Sets `metric` to `value` on this model from here on.
    pub(super) fn note(&mut self, metric: Metric, value: Scalar) {
        self.measured.set(metric, Value::Number(value));
    }

    /// The pockets of resin with no way out of it, by the drainage scan; none with no cavity.
    pub(super) fn trapped(&self, staging: &Staging, stop: &Stop) -> Result<usize> {
        if !staging.hollowed(self.id) {
            return Ok(0);
        }
        let model = staging.model(self.id).context("the model left the plate")?;
        let model = Model {
            supports: Vec::new(),
            raft: None,
            ..model
        };
        let baked = bake(&[model], &Compensation::default()).context("nothing to scan")?;
        let found = trapped_in(&baked, ANALYSIS_LAYER_MM, |_, _| !stop.requested())
            .context("cannot scan the model for trapped resin")?
            .ok_or(Cancelled)?;
        Ok(found.len())
    }

    /// What import found and the search chose, and what hollowing, drilling and supporting
    /// found, as the plate's parts say them.
    pub(super) fn found(self) -> (Found, Finished, (Prepared, Vec<StepRun>)) {
        let finished = Finished {
            hollow: self.hollow,
            drains: self.drains,
            supports: self.supports,
        };
        (
            (self.import, self.oriented),
            finished,
            (self.prepared, self.steps),
        )
    }
}
