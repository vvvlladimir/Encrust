//! One model as a recipe's steps leave it: where it stands, its cavity, holes and supports,
//! what was measured on it, and what each step decided.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use core_engine::{Drained, Model, SupportPlan, bake, trapped_in};
use core_geometry::{Aabb, BoundingVolume, Mesh, MeshDiagnostics, Scalar, Transform, Vec3};
use core_geometry::{diagnose, placed_aabb, signed_volume, transform_mesh};
use core_recipe::{Metric, Metrics, Plan, Value, measure_planned};
use core_volume::{DrainHole, drill, laid_beside};
use printer_profiles::{Compensation, PrinterProfile};

use super::prepared::Prepared;
use super::report::{RecipeRun, StepRun};
use crate::exit::{Cancelled, Stop};
use crate::hollowing::{HollowReport, pierced};
use crate::pipeline::OrientSummary;
use crate::report::{FitCheck, ImportReport};
use crate::stage::{Found, Part, Shaping, standing_of};
use crate::supports::{Printed, SupportReport};

/// One model on its way through a recipe, in plate coordinates throughout.
pub(super) struct Piece {
    pub input: PathBuf,
    /// What the flags and a plate file gave this model, which win over a step's settings.
    pub shaping: Shaping,
    /// Whether a supports step of the recipe stands it, which its turn and holes are judged on.
    pub stands: bool,
    /// The model as it prints: its shell once hollowed, lifted with its supports.
    pub mesh: Mesh,
    /// The user turned it, or a search did, so a search now would undo a turn.
    pub turned: bool,
    /// The last orientation set it on the plate, needing no supports this way up.
    pub seated: bool,
    pub cavity: Option<Range<usize>>,
    /// The wall the cavity was cut to and the lattice it was cut on, millimetres.
    pub wall: Option<(Scalar, Scalar)>,
    pub holes: Vec<DrainHole>,
    /// A plug for each hole a drain step asked plugs for, with the hole as it was cut then.
    pub plugs: Vec<(Mesh, DrainHole)>,
    /// The supports standing under it and what stood them, with the plan they were built to.
    pub supports: Option<(SupportReport, Mesh)>,
    pub plan: Option<SupportPlan>,
    pub import: ImportReport,
    pub oriented: Option<OrientSummary>,
    pub hollow: Option<HollowReport>,
    pub drains: Option<Drained>,
    /// What steps measured on it since import, over what `measure` reads off it.
    pub measured: Metrics,
    pub prepared: Prepared,
    pub steps: Vec<StepRun>,
}

impl Piece {
    pub(super) fn new(
        (input, shaping, stands): (PathBuf, Shaping, bool),
        mesh: Mesh,
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
            mesh,
            turned,
            cavity: None,
            wall: None,
            holes: Vec::new(),
            plugs: Vec::new(),
            supports: None,
            plan: None,
            import,
            oriented,
            hollow: None,
            drains: None,
            measured: Metrics::default(),
            steps: Vec::new(),
        }
    }

    /// How far its supports stood it up, millimetres.
    pub(super) fn lifted_mm(&self) -> Scalar {
        self.supports
            .as_ref()
            .map_or(0.0, |(report, _)| report.lifted_mm)
    }

    /// Moves the model, its supports and holes by `transform`.
    pub(super) fn moved(&mut self, transform: Transform) {
        self.mesh = transform_mesh(&self.mesh, transform);
        if let Some((_, supports)) = &mut self.supports {
            *supports = transform_mesh(supports, transform);
        }
        let matrix = transform.to_matrix();
        for hole in &mut self.holes {
            hole.at = matrix.transform_point3(hole.at);
            hole.axis = matrix.transform_vector3(hole.axis);
        }
    }

    /// Takes the supports off and sets the model down where it stood before they lifted it.
    pub(super) fn take_off_supports(&mut self) {
        let down = Transform::from_translation(Vec3::NEG_Z * self.lifted_mm());
        self.supports = None;
        self.moved(down);
    }

    /// The bodies its holes cut, deepened through the wall where it has one.
    pub(super) fn cuts(&self) -> Result<Option<Mesh>> {
        if self.holes.is_empty() {
            return Ok(None);
        }
        let holes = pierced(&self.holes, self.wall);
        drill(&holes, &[]).map(Some).context("drilling the drains")
    }

    /// Which faces bound its cavity, and how its cuts are taken out.
    pub(super) fn printed(&self) -> Printed {
        Printed::of(self.cavity.clone(), self.hollow.as_ref())
    }

    /// The model as a run cuts it, its supports under it.
    pub(super) fn model(&self) -> Result<Model> {
        let cuts = self.cuts()?;
        Ok(Model {
            mesh: Arc::new(self.mesh.clone()),
            transform: Transform::default(),
            drilled: self.printed().drilled(&self.mesh, cuts.as_ref()),
            cuts: cuts.map(Arc::new),
            supports: self
                .supports
                .iter()
                .map(|(_, supports)| Arc::new(supports.clone()))
                .collect(),
        })
    }

    /// The model without the cavity it carries: what a plug is cut out of.
    pub(super) fn solid(&self) -> Mesh {
        match &self.cavity {
            Some(cavity) => Mesh::new(
                self.mesh.vertices.clone(),
                self.mesh.faces[..cavity.start].to_vec(),
            ),
            None => self.mesh.clone(),
        }
    }

    /// Its plugs, laid in a row past its right-hand side as `--drain-plugs` lays them.
    pub(super) fn plug_models(&self) -> Vec<Model> {
        let Some(beside) = self.bounds() else {
            return Vec::new();
        };
        let (meshes, holes): (Vec<Mesh>, Vec<DrainHole>) = self.plugs.iter().cloned().unzip();
        laid_beside(&meshes, &holes, Transform::default(), beside)
            .into_iter()
            .map(|plug| Model {
                mesh: Arc::new(plug),
                transform: Transform::default(),
                cuts: None,
                drilled: None,
                supports: Vec::new(),
            })
            .collect()
    }

    /// The box it and its supports stand in.
    pub(super) fn bounds(&self) -> Option<Aabb> {
        let supports = self.supports.iter().filter_map(|(_, mesh)| mesh.aabb());
        placed_aabb(&self.mesh, Transform::default())
            .into_iter()
            .chain(supports)
            .reduce(|one, other| one.merged(&other))
    }

    /// The resin in its supports, millilitres.
    pub(super) fn support_ml(&self) -> Scalar {
        let mm3 = self
            .supports
            .as_ref()
            .map_or(0.0, |(_, mesh)| signed_volume(mesh).abs());
        mm3 / 1000.0
    }

    /// Every variable of the model as it stands, `saves_ml` at `wall_mm`, with what steps
    /// measured on it since.
    pub(super) fn metrics(&self, plan: &Plan, printer: Option<&PrinterProfile>) -> Metrics {
        let diagnostics = self.diagnostics();
        let fits = match (self.bounds(), printer) {
            (Some(bounds), Some(printer)) => FitCheck::of(&bounds, printer).fit.fits(),
            _ => true,
        };
        let model = core_recipe::Model {
            mesh: &self.mesh,
            transform: &Transform::default(),
            diagnostics: &diagnostics,
            turned: self.turned,
            hollow: self.cavity.is_some(),
            fits,
        };
        let wall_mm = self.shaping.hollow.thickness_mm;
        measure_planned(plan, &model, wall_mm, &self.measured)
    }

    fn diagnostics(&self) -> MeshDiagnostics {
        self.import
            .diagnostics
            .clone()
            .unwrap_or_else(|| diagnose(&self.mesh))
    }

    /// Sets `metric` to `value` on this model from here on.
    pub(super) fn note(&mut self, metric: Metric, value: Scalar) {
        self.measured.set(metric, Value::Number(value));
    }

    /// The pockets of resin with no way out of it, by the drainage scan; none with no cavity.
    pub(super) fn trapped(&self, layer_height_mm: Scalar, stop: &Stop) -> Result<usize> {
        if self.cavity.is_none() {
            return Ok(0);
        }
        let model = Model {
            supports: Vec::new(),
            ..self.model()?
        };
        let baked = bake(&[model], &Compensation::default()).context("nothing to scan")?;
        let found = trapped_in(&baked, layer_height_mm, |_, _| !stop.requested())
            .context("cannot scan the model for trapped resin")?
            .ok_or(Cancelled)?;
        Ok(found.len())
    }

    /// The part the report states, the model the plate cuts and its plugs, from the piece as
    /// the recipe `name` left it; `prepared` keeps what `--prepare` says of it.
    pub(super) fn laid(
        self,
        (name, prepared): (&str, bool),
        printer: Option<&PrinterProfile>,
        talk: bool,
    ) -> Result<(Part, Model, Vec<Model>)> {
        let model = self.model()?;
        let plugs = self.plug_models();
        let standing = standing_of(self.bounds(), printer, self.supports.is_some(), talk);
        let part = Part {
            input: self.input,
            import: Some(self.import),
            oriented: self.oriented,
            hollow: self.hollow,
            drains: self.drains,
            supports: self.supports.map(|(report, _)| report),
            standing,
            meets: Vec::new(),
            prepared: prepared.then_some(self.prepared),
            recipe: Some(RecipeRun {
                name: name.to_owned(),
                steps: self.steps,
            }),
        };
        Ok((part, model, plugs))
    }
}
