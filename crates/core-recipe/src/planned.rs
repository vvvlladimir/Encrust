//! A model measured as a plan stands to it: `saves_ml` at the wall its hollow step would
//! hollow it at, and what the run measured since laid over.

use core_geometry::Scalar;
use core_volume::HollowSettings;

use crate::decide::{Decision, Resolved, decide};
use crate::metrics::{Metric, Metrics, Model, measure};
use crate::recipe::{Setting, StepKind};
use crate::validate::Plan;

#[cfg(test)]
mod tests;

/// The wall `saves_ml` is reckoned at before a model is reached: the first enabled hollow
/// step's when it names one outright, else Hollow's default; and that step, when it works
/// the wall out per model.
pub fn planned_wall(plan: &Plan) -> (Scalar, Option<usize>) {
    let default = HollowSettings::default().thickness_mm;
    let mut steps = plan.recipe().steps.iter().enumerate();
    let first = steps.find(|(_, step)| step.enabled && step.kind == StepKind::Hollow);
    match first.map(|(index, step)| (index, step.set.get("wall_mm"))) {
        Some((_, Some(Setting::Number(wall_mm)))) => (*wall_mm, None),
        Some((index, Some(Setting::Expr(_)))) => (default, Some(index)),
        _ => (default, None),
    }
}

/// Every variable of `model` with `earlier`, what the run measured on it since, laid over;
/// `saves_ml` at `wall_mm` where a front end fixes the wall, else at [`planned_wall`]'s, worked
/// out on the model where the step does. A wall that fails to work out is the default's: the
/// step says why once it is reached.
pub fn measure_planned(
    plan: &Plan,
    model: &Model,
    wall_mm: Option<Scalar>,
    earlier: &Metrics,
) -> Metrics {
    let (wall_mm, worked_out) = match wall_mm {
        Some(wall_mm) => (wall_mm, None),
        None => planned_wall(plan),
    };
    let measured = |wall_mm| {
        let mut metrics = measure(model, wall_mm);
        for metric in Metric::ALL {
            if let Some(value) = earlier.get(metric) {
                metrics.set(metric, value);
            }
        }
        metrics
    };
    let metrics = measured(wall_mm);
    let Some(index) = worked_out else {
        return metrics;
    };
    match decide(plan, index, &metrics) {
        Ok(Decision::Run { settings, .. }) => match settings.get("wall_mm") {
            Some(Resolved::Number(wall_mm)) => measured(*wall_mm),
            _ => metrics,
        },
        _ => metrics,
    }
}
