//! The flags typed for a model laid over a step's resolved settings:
//! a flag shaping a setting wins over it in every step of its kind (`docs/design/recipes.md`).

use clap::ValueEnum;
use core_recipe::{Resolved, StepKind};

use super::prepared::profile_name;
use crate::hollowing::{HollowArgs, Mode};
use crate::stage::Shaping;

pub(super) type Settings = core_recipe::ResolvedSettings;

/// The step kinds a flag typed for a model, or a plate file's key, makes run whatever their
/// `when`, each with what asks it as the user wrote it: the flag, or the plate file's key.
pub(super) fn forced(shaping: &Shaping) -> impl Iterator<Item = (StepKind, &'static str)> {
    let keyed = shaping.keyed;
    let typed = [
        (
            StepKind::Orient,
            "--orient",
            shaping.import.transform.orient,
        ),
        (
            StepKind::Repair,
            "--repair",
            shaping.import.repair.is_some(),
        ),
        (
            StepKind::Hollow,
            asker(keyed.hollow, "--hollow", "the plate file's `hollow`"),
            shaping.hollow.wanted(),
        ),
        (
            StepKind::Supports,
            asker(keyed.supports, "--supports", "the plate file's `supports`"),
            shaping.supports.is_some(),
        ),
    ];
    typed
        .into_iter()
        .filter_map(|(kind, asker, typed)| typed.then_some((kind, asker)))
}

/// The plate file's `key` where it gave the setting, else `flag`.
fn asker(keyed: bool, flag: &'static str, key: &'static str) -> &'static str {
    if keyed { key } else { flag }
}

/// `settings` of a step of `kind` with what `shaping` was given by a flag or a plate file
/// laid over them.
pub(super) fn typed_over(kind: StepKind, mut settings: Settings, shaping: &Shaping) -> Settings {
    let mut set = |key: &str, value: Resolved| {
        settings.insert(key.to_owned(), value);
    };
    let (import, hollow) = (&shaping.import, &shaping.hollow);
    match kind {
        StepKind::Orient if import.transform.orient => {
            set(
                "goal",
                word(import.transform.orient_goal.to_possible_value()),
            );
            if let Some(tilt) = import.transform.max_tilt {
                set("max_tilt_deg", Resolved::Number(tilt));
            }
        }
        StepKind::Repair => {
            if let Some(method) = import.repair {
                set("method", word(method.to_possible_value()));
            }
        }
        StepKind::Hollow => hollow_over(hollow, &mut set),
        StepKind::Drain => drain_over(hollow, &mut set),
        StepKind::Supports => {
            if let Some(plan) = &shaping.supports {
                set("profile", Resolved::Text(profile_name(plan)));
                if plan.cavity {
                    set("cavity", Resolved::Bool(true));
                }
            }
        }
        _ => {}
    }
    settings
}

/// A flag's value as the word a recipe names it by.
fn word(name: Option<clap::builder::PossibleValue>) -> Resolved {
    Resolved::Word(
        name.map(|name| name.get_name().to_owned())
            .unwrap_or_default(),
    )
}

/// What the hollow flags shape of a hollow step.
fn hollow_over(hollow: &HollowArgs, set: &mut impl FnMut(&str, Resolved)) {
    if let Some(wall_mm) = hollow.thickness_mm {
        set("wall_mm", Resolved::Number(wall_mm));
    }
    if matches!(hollow.mode, Mode::External) {
        set("mode", Resolved::Word("outside".to_owned()));
    } else if hollow.open_floor {
        set("mode", Resolved::Word("open-floor".to_owned()));
    }
    if hollow.infill.is_some() {
        set("infill", Resolved::Bool(true));
    }
    if let Some(under_mm) = hollow.keep_solid.under_mm {
        set("keep_solid_under_mm", Resolved::Number(under_mm));
    }
}

/// What the drain flags shape of a drain step.
fn drain_over(hollow: &HollowArgs, set: &mut impl FnMut(&str, Resolved)) {
    if hollow.no_vent {
        set("vent", Resolved::Bool(false));
    }
    if hollow.auto() {
        set("at", Resolved::Word("auto".to_owned()));
    }
    if hollow.plugs {
        set("plugs", Resolved::Bool(true));
    }
}
