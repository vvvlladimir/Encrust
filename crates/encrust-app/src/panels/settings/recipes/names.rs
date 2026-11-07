//! The names a condition of the recipe can read, as the list beside its box offers them:
//! what is measured on the model, the recipe's own variables, and the functions.

use core_recipe::Metric;

use crate::i18n::tr;
use crate::ui::ConditionName;

/// Every name a condition of a recipe with `vars` can read, under its heading.
pub(super) fn condition_names(vars: &[(String, f32)]) -> Vec<ConditionName> {
    let (from_start, measured): (Vec<Metric>, Vec<Metric>) = Metric::ALL
        .into_iter()
        .partition(|metric| metric.known_after().is_empty());
    let model = tr!("recipes-names-model");
    let later = tr!("recipes-names-measured");
    let mut names: Vec<ConditionName> = from_start
        .into_iter()
        .map(|metric| entry(metric.name(), meaning(metric), &model))
        .chain(
            measured
                .into_iter()
                .map(|m| entry(m.name(), meaning(m), &later)),
        )
        .collect();
    let yours = tr!("recipes-names-yours");
    names.extend(
        vars.iter()
            .filter(|(name, _)| !name.trim().is_empty())
            .map(|(name, value)| {
                let meaning = tr!("recipes-names-your-value", value = format!("{value}"));
                entry(name.trim(), meaning, &yours)
            }),
    );
    let functions = tr!("recipes-names-functions");
    names.extend([
        entry("min(", tr!("recipes-names-min"), &functions),
        entry("max(", tr!("recipes-names-max"), &functions),
        entry("abs(", tr!("recipes-names-abs"), &functions),
        entry("clamp(", tr!("recipes-names-clamp"), &functions),
    ]);
    names
}

fn entry(name: &str, meaning: String, group: &str) -> ConditionName {
    ConditionName {
        name: name.to_owned(),
        meaning,
        group: group.to_owned(),
    }
}

/// What a measured name means, in a few words with its unit.
fn meaning(metric: Metric) -> String {
    match metric {
        Metric::HeightMm => tr!("recipes-names-height"),
        Metric::WidthMm => tr!("recipes-names-width"),
        Metric::DepthMm => tr!("recipes-names-depth"),
        Metric::VolumeMl => tr!("recipes-names-volume"),
        Metric::SurfaceCm2 => tr!("recipes-names-surface"),
        Metric::Faces => tr!("recipes-names-faces"),
        Metric::Sound => tr!("recipes-names-sound"),
        Metric::Defects => tr!("recipes-names-defects"),
        Metric::Turned => tr!("recipes-names-turned"),
        Metric::Hollow => tr!("recipes-names-hollow"),
        Metric::SavesMl => tr!("recipes-names-saves-ml"),
        Metric::SavesShare => tr!("recipes-names-saves-share"),
        Metric::WorthHollowing => tr!("recipes-names-worth-hollowing"),
        Metric::Fits => tr!("recipes-names-fits"),
        Metric::Trapped => tr!("recipes-names-trapped"),
        Metric::Islands => tr!("recipes-names-islands"),
        Metric::PeelN => tr!("recipes-names-peel"),
        Metric::SupportMl => tr!("recipes-names-support-ml"),
    }
}
