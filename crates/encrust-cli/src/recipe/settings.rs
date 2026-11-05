//! A step's resolved settings read by key, and the flags typed for a model laid over them:
//! a flag shaping a setting wins over it in every step of its kind (`docs/design/recipes.md`).

use std::collections::BTreeMap;

use clap::ValueEnum;
use core_geometry::Scalar;
use core_recipe::{Resolved, StepKind};

use super::prepared::profile_name;
use crate::hollowing::Mode;
use crate::stage::Shaping;

pub(super) type Settings = BTreeMap<String, Resolved>;

pub(super) fn number(settings: &Settings, key: &str) -> Option<Scalar> {
    match settings.get(key)? {
        Resolved::Number(number) => Some(*number),
        _ => None,
    }
}

pub(super) fn flag(settings: &Settings, key: &str) -> Option<bool> {
    match settings.get(key)? {
        Resolved::Bool(flag) => Some(*flag),
        _ => None,
    }
}

/// A word or a text, which a setting such as a profile id may be written as.
pub(super) fn word<'a>(settings: &'a Settings, key: &str) -> Option<&'a str> {
    match settings.get(key)? {
        Resolved::Word(text) | Resolved::Text(text) => Some(text),
        _ => None,
    }
}

pub(super) fn points<'a>(settings: &'a Settings, key: &str) -> &'a [[Scalar; 3]] {
    match settings.get(key) {
        Some(Resolved::Points(points)) => points,
        _ => &[],
    }
}

/// The step kinds a flag typed for a model, or a plate file's key, makes run whatever their
/// `when`, each with the flag that asks it.
pub(super) fn forced(shaping: &Shaping) -> impl Iterator<Item = (StepKind, &'static str)> {
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
        (StepKind::Hollow, "--hollow", shaping.hollow.wanted()),
        (StepKind::Supports, "--supports", shaping.supports.is_some()),
    ];
    typed
        .into_iter()
        .filter_map(|(kind, flag, typed)| typed.then_some((kind, flag)))
}

/// `settings` of a step of `kind` with what `shaping` was given by a flag or a plate file
/// laid over them.
pub(super) fn typed_over(kind: StepKind, mut settings: Settings, shaping: &Shaping) -> Settings {
    let mut set = |key: &str, value: Resolved| {
        settings.insert(key.to_owned(), value);
    };
    let word = |name: Option<clap::builder::PossibleValue>| {
        Resolved::Word(
            name.map(|name| name.get_name().to_owned())
                .unwrap_or_default(),
        )
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
        StepKind::Hollow => {
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
        }
        StepKind::Drain => {
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
