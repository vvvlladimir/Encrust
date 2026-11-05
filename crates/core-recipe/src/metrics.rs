//! What a condition can read off a model: the variables of `docs/design/recipes.md`.

use core_geometry::{Mesh, MeshDiagnostics, Scalar, Transform};

use crate::condition::{Kind, Value};
use crate::recipe::StepKind;

/// One variable a condition may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    HeightMm,
    WidthMm,
    DepthMm,
    VolumeMl,
    SurfaceCm2,
    Faces,
    Sound,
    Defects,
    Turned,
    Hollow,
    SavesMl,
    SavesShare,
    WorthHollowing,
    Fits,
    Trapped,
    Islands,
    PeelN,
    SupportMl,
}

impl Metric {
    pub const ALL: [Self; 18] = [
        Self::HeightMm,
        Self::WidthMm,
        Self::DepthMm,
        Self::VolumeMl,
        Self::SurfaceCm2,
        Self::Faces,
        Self::Sound,
        Self::Defects,
        Self::Turned,
        Self::Hollow,
        Self::SavesMl,
        Self::SavesShare,
        Self::WorthHollowing,
        Self::Fits,
        Self::Trapped,
        Self::Islands,
        Self::PeelN,
        Self::SupportMl,
    ];

    /// The name a condition reads it by.
    pub fn name(self) -> &'static str {
        // TODO(step-3): the names of the table in docs/design/recipes.md.
        todo!()
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|metric| metric.name() == name)
    }

    pub fn kind(self) -> Kind {
        // TODO(step-3): Sound, Turned, Hollow, WorthHollowing and Fits are booleans.
        todo!()
    }

    /// The steps any one of which, earlier in the recipe, measures it; empty when it is known
    /// from the start.
    pub fn known_after(self) -> &'static [StepKind] {
        // TODO(step-3): Trapped after Drain or Check; Islands, PeelN, SupportMl after Check.
        todo!()
    }
}

/// The variables measured on one model so far; a step's front end sets what it measured.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metrics {
    values: Vec<(Metric, Value)>,
}

impl Metrics {
    pub fn get(&self, metric: Metric) -> Option<Value> {
        self.values
            .iter()
            .find_map(|(held, value)| (*held == metric).then_some(*value))
    }

    pub fn set(&mut self, metric: Metric, value: Value) {
        match self.values.iter_mut().find(|(held, _)| *held == metric) {
            Some((_, held)) => *held = value,
            None => self.values.push((metric, value)),
        }
    }
}

/// A model as a front end holds it, what `measure` reads.
#[derive(Debug, Clone, Copy)]
pub struct Model<'a> {
    pub mesh: &'a Mesh,
    pub transform: &'a Transform,
    pub diagnostics: &'a MeshDiagnostics,
    /// The user turned it, so an orientation search would undo their choice.
    pub turned: bool,
    pub hollow: bool,
    /// It fits the build volume with what stands under it.
    pub fits: bool,
}

/// Every variable known from the start, `saves_ml` reckoned at `wall_mm`.
pub fn measure(model: &Model, wall_mm: Scalar) -> Metrics {
    // TODO(step-3): one pass over the faces, through core_engine and core_volume's estimate.
    let _ = (model, wall_mm);
    todo!()
}
