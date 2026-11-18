//! What the report leaves out so it fits the issue form's URL more often: tool values still
//! at their defaults, and the frames of a backtrace that are not Encrust's own.

use serde_json::{Map, Value};

use crate::tool_settings::ToolSettings;

/// The crates a frame must belong to for the report to keep it; the rest are the standard
/// library and the window's libraries, which say where a panic went, not where it came from.
const OUR_CRATES: [&str; 6] = [
    "encrust",
    "core_",
    "format_",
    "net_",
    "printer_",
    "web_engine",
];

/// The tool values that differ from a fresh window's, as pretty JSON; `{}` when none do.
pub(super) fn changed_settings(settings: &ToolSettings) -> String {
    let fresh = ToolSettings::of(&crate::state::Tools::default(), &Default::default());
    let (Ok(now), Ok(fresh)) = (serde_json::to_value(settings), serde_json::to_value(&fresh))
    else {
        return "\"these settings could not be written out\"".to_owned();
    };
    let changed = changed(&now, &fresh).unwrap_or_else(|| Value::Object(Map::new()));
    serde_json::to_string_pretty(&changed).unwrap_or_default()
}

/// The parts of `now` that differ from `fresh`, objects compared key by key.
fn changed(now: &Value, fresh: &Value) -> Option<Value> {
    match (now, fresh) {
        (Value::Object(now), Value::Object(fresh)) => {
            let kept: Map<String, Value> = now
                .iter()
                .filter_map(|(key, value)| {
                    let differs = match fresh.get(key) {
                        Some(was) => changed(value, was),
                        None => Some(value.clone()),
                    };
                    differs.map(|value| (key.clone(), value))
                })
                .collect();
            (!kept.is_empty()).then_some(Value::Object(kept))
        }
        _ => (now != fresh).then(|| now.clone()),
    }
}

/// A crash with its backtrace cut to Encrust's own frames, and how many others were left out.
pub(super) fn trimmed_crash(crash: &str) -> String {
    let Some((head, backtrace)) = crash.split_once("\n\n") else {
        return crash.to_owned();
    };
    let mut kept = Vec::new();
    let mut dropped = 0;
    let mut keeping = false;
    for line in backtrace.lines() {
        let frame = line.trim_start();
        if let Some((number, name)) = frame.split_once(": ")
            && number.chars().all(|c| c.is_ascii_digit())
        {
            // A trait method is named `<Type as Trait>::method`: its type says whose it is.
            let name = name.trim_start_matches('<');
            keeping = OUR_CRATES.iter().any(|ours| name.starts_with(ours));
            dropped += usize::from(!keeping);
        }
        if keeping {
            kept.push(line);
        }
    }
    let mut text = format!("{head}\n\n{}", kept.join("\n"));
    if dropped > 0 {
        text.push_str(&format!("\n({dropped} frames of other libraries left out)"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_window_has_no_changed_settings() {
        let fresh = ToolSettings::of(&crate::state::Tools::default(), &Default::default());
        assert_eq!(changed_settings(&fresh), "{}");
    }

    #[test]
    fn only_the_differing_leaves_are_kept() {
        let now = serde_json::json!({"hollow": {"wall_mm": 2.0, "on": true}, "cut": {"z": 1}});
        let fresh = serde_json::json!({"hollow": {"wall_mm": 1.5, "on": true}, "cut": {"z": 1}});
        assert_eq!(
            changed(&now, &fresh),
            Some(serde_json::json!({"hollow": {"wall_mm": 2.0}}))
        );
    }

    #[test]
    fn a_backtrace_keeps_only_encrusts_frames() {
        let crash = "Encrust 0.9.0, thread 'main' panicked at src/a.rs:1:2:\nboom\n\n\
                     stack backtrace:\n   0: std::panicking::begin_panic\n             at /rustc/x.rs:1:1\n   \
                     1: core_slicer::slice::layer\n             at crates/x.rs:2:2\n   \
                     2: winit::run";
        let trimmed = trimmed_crash(crash);
        assert!(trimmed.starts_with("Encrust 0.9.0, thread 'main' panicked"));
        assert!(trimmed.contains("1: core_slicer::slice::layer\n             at crates/x.rs:2:2"));
        assert!(!trimmed.contains("begin_panic") && !trimmed.contains("winit"));
        assert!(trimmed.ends_with("(2 frames of other libraries left out)"));
    }

    #[test]
    fn a_trait_method_of_encrust_is_its_own_frame() {
        let crash = "panicked\n\n   0: <encrust_app::app::SlicerApp as eframe::App>::update\n   \
                     1: <winit::Window as core::ops::Drop>::drop";
        let trimmed = trimmed_crash(crash);
        assert!(trimmed.contains("0: <encrust_app::app::SlicerApp as eframe::App>::update"));
        assert!(trimmed.ends_with("(1 frames of other libraries left out)"));
    }
}
