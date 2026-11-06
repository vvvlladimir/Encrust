//! What a run's conditions read off a model as it stands: `core_recipe::measure_planned`,
//! with what the run measured since.

use core_recipe::{Decision, Metric, Metrics, Model, Value, decide, measure_planned};

use super::{Run, unturned};
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};

/// Whether `object` fits the build volume with what stands under it.
fn fits(object: &SceneObject, plate: &BuildPlate) -> bool {
    crate::plate::bounds(object).is_some_and(|bounds| plate.fit(&bounds).fits())
}

impl Run {
    /// Every variable of `object` as it stands, with what the run measured on it since;
    /// `saves_ml` at the wall the plan's hollow step works out for it, where it does.
    pub(super) fn metrics_of(&self, object: &SceneObject, plate: &BuildPlate) -> Metrics {
        let fits = fits(object, plate);
        let model = Model {
            mesh: &object.mesh,
            transform: &object.transform,
            diagnostics: &object.summary.diagnostics,
            turned: !unturned(object),
            hollow: object.hollow.is_hollow(),
            fits,
        };
        let none = Metrics::default();
        let earlier = self.measured.iter().find(|(held, _)| *held == object.id);
        let earlier = earlier.map_or(&none, |(_, earlier)| earlier);
        measure_planned(&self.plan, &model, None, earlier)
    }

    /// Reckons the steps the run takes in all: those done, the one in hand, and each later
    /// one some model would run as the plate stands now, or that reads what is not known yet.
    pub(super) fn recount(&mut self, scene: &Scene, plate: &BuildPlate) {
        let metrics: Vec<Metrics> = scene
            .named(&self.ids)
            .map(|object| self.metrics_of(object, plate))
            .collect();
        let later = (self.next..self.plan.recipe().steps.len())
            .filter(|index| {
                metrics.iter().any(|metrics| {
                    matches!(
                        decide(&self.plan, *index, metrics),
                        Ok(Decision::Run { .. }) | Err(_)
                    )
                })
            })
            .count();
        self.steps = self.done + 1 + later;
    }

    pub(super) fn measured_of(&mut self, id: ObjectId) -> &mut Metrics {
        let at = match self.measured.iter().position(|(held, _)| *held == id) {
            Some(at) => at,
            None => {
                self.measured.push((id, Metrics::default()));
                self.measured.len() - 1
            }
        };
        &mut self.measured[at].1
    }

    /// Sets `metric` to `value` on model `id` from here on.
    pub(super) fn note(&mut self, id: ObjectId, metric: Metric, value: f32) {
        self.measured_of(id).set(metric, Value::Number(value));
    }
}
