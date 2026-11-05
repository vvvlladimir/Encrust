//! What a run's conditions read off a model as it stands: `core_recipe::measure`, with
//! what the run measured since and the wall a hollow step works out for it.

use core_engine::Hollowing;
use core_geometry::Scalar;
use core_recipe::{Decision, Metric, Metrics, Model, Plan, Resolved, Setting, StepKind, Value};
use core_recipe::{decide, measure};
use core_volume::{HollowSettings, hollowing_saves_mm3, placed_volume_and_area, raw_saving_mm3};

use super::{Run, unturned};
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};

impl Run {
    /// Every variable of `object` as it stands, with what the run measured on it since;
    /// `saves_ml` at the wall the plan's hollow step works out for it, where it does.
    pub(super) fn metrics_of(&self, object: &SceneObject, plate: &BuildPlate) -> Metrics {
        let fits = crate::plate::bounds(object).is_some_and(|bounds| plate.fit(&bounds).fits());
        let model = Model {
            mesh: &object.mesh,
            transform: &object.transform,
            diagnostics: &object.summary.diagnostics,
            turned: !unturned(object),
            hollow: object.hollow.is_hollow(),
            fits,
        };
        let metrics = self.with_earlier(object.id, measure(&model, self.wall_mm));
        let Some(index) = self.wall_at else {
            return metrics;
        };
        match decide(&self.plan, index, &metrics) {
            Ok(Decision::Run { settings, .. }) => match settings.get("wall_mm") {
                Some(Resolved::Number(wall_mm)) => {
                    self.with_earlier(object.id, measure(&model, *wall_mm))
                }
                _ => metrics,
            },
            _ => metrics,
        }
    }

    fn with_earlier(&self, id: ObjectId, mut metrics: Metrics) -> Metrics {
        let earlier = self.measured.iter().find(|(held, _)| *held == id);
        if let Some((_, earlier)) = earlier {
            for metric in Metric::ALL {
                if let Some(value) = earlier.get(metric) {
                    metrics.set(metric, value);
                }
            }
        }
        metrics
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

/// The wall `saves_ml` is reckoned at before a model is reached: the first hollow step's
/// when it names one outright, else Hollow's default; and that step, when it is worked out.
pub(super) fn wall_of(plan: &Plan) -> (Scalar, Option<usize>) {
    let default = HollowSettings::default().thickness_mm;
    let mut steps = plan.recipe().steps.iter().enumerate();
    let first = steps.find(|(_, step)| step.enabled && step.kind == StepKind::Hollow);
    match first.map(|(index, step)| (index, step.set.get("wall_mm"))) {
        Some((_, Some(Setting::Number(wall_mm)))) => (*wall_mm, None),
        Some((index, Some(Setting::Expr(_)))) => (default, Some(index)),
        _ => (default, None),
    }
}

/// The wall `object` is hollowed at, and the resin ADR 0393 reckons it saves there.
pub(super) fn hollowing_at(object: &SceneObject, wall_mm: f32) -> Hollowing {
    let linear = object.transform.linear();
    let saves_mm3 = hollowing_saves_mm3(&object.mesh, linear, wall_mm).unwrap_or_else(|| {
        let (volume_mm3, area_mm2) = placed_volume_and_area(&object.mesh, linear);
        raw_saving_mm3(volume_mm3, area_mm2, wall_mm).max(0.0) as f32
    });
    Hollowing { wall_mm, saves_mm3 }
}
