//! What the card says about one printer, its word, light, print and temperatures, worked
//! out from what the network last heard before anything is drawn.

use egui::Color32;
use printer_link::State;

use crate::i18n::tr;
use crate::network::{Reach, Reading};
use crate::ui::theme;

/// What the card says about one printer, worked out before anything is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Face {
    /// The word in the corner, and how it is lit.
    pub(super) word: String,
    pub(super) light: Light,
    /// The print under way or the last one, while the printer names it.
    pub(super) job: Option<Job>,
    /// Temperatures, on one line.
    pub(super) warmth: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Light {
    Working,
    Ready,
    Held,
    Down,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Job {
    pub(super) file: String,
    /// Share done, 0 to 1, while it is being printed.
    pub(super) progress: Option<f32>,
    /// The line under the bar: the layer and the time left, or how it ended.
    pub(super) detail: String,
}

/// One line for a printer's state, for the list in the printer settings, and its colour.
pub fn summary(reach: Reach, reading: Option<&Reading>) -> (String, Color32) {
    let face = face(reach, reading);
    let tint = colour_of(face.light);
    match face.job {
        Some(Job {
            file,
            progress: Some(progress),
            ..
        }) => (
            tr!(
                "printer-printing-file",
                state = face.word,
                file = file,
                percent = format!("{:.0}", progress * 100.0)
            ),
            tint,
        ),
        _ => (face.word, tint),
    }
}

pub(super) fn face(reach: Reach, reading: Option<&Reading>) -> Face {
    match reading {
        Some(Reading::State(state)) => of_state(state),
        Some(Reading::Failed(_)) => Face {
            word: tr!("card-not-answering"),
            light: Light::Down,
            job: None,
            warmth: None,
        },
        None => Face {
            word: match reach {
                Reach::Answered => tr!("card-connecting"),
                Reach::Silent => tr!("card-not-answering"),
                Reach::Unasked => tr!("card-not-asked"),
            },
            light: match reach {
                Reach::Silent => Light::Down,
                Reach::Answered | Reach::Unasked => Light::Unknown,
            },
            job: None,
            warmth: None,
        },
    }
}

/// Whether the print is paused or on its way to it, which is what Resume answers.
pub(super) fn paused(state: &State) -> bool {
    matches!(state.stage.as_deref(), Some("pausing" | "paused")) || state.state == "paused"
}

pub(super) fn of_state(state: &State) -> Face {
    let stage = state.stage.as_deref();
    let (word, light) = if paused(state) {
        (tr!("card-paused"), Light::Held)
    } else if state.busy {
        (capitalised(stage.unwrap_or(&state.state)), Light::Working)
    } else {
        (tr!("card-ready"), Light::Ready)
    };
    Face {
        word,
        light,
        job: job(state),
        warmth: warmth(state),
    }
}

pub(super) fn job(state: &State) -> Option<Job> {
    let file = state.file.clone()?;
    if !state.busy {
        let ended = match (&state.error, state.stage.as_deref()) {
            (Some(error), _) => tr!("card-stopped-error", error = error.clone()),
            (None, Some("complete")) => tr!("card-finished"),
            (None, Some("stopped")) => tr!("card-stopped"),
            (None, _) => tr!("card-last-print"),
        };
        return Some(Job {
            file,
            progress: None,
            detail: ended,
        });
    }
    let mut parts = Vec::new();
    if let (Some(layer), Some(count)) = (state.layer, state.layer_count) {
        parts.push(tr!("card-layer-of", layer = layer, count = count));
    }
    if let Some(remaining_s) = state.remaining_s {
        parts.push(left(remaining_s));
    }
    Some(Job {
        file,
        progress: state.progress,
        detail: parts.join(" · "),
    })
}

pub(super) fn warmth(state: &State) -> Option<String> {
    let readings: Vec<String> = [
        (tr!("card-light-source"), state.uv_led_c),
        (tr!("card-hood"), state.chamber_c),
    ]
    .into_iter()
    .filter_map(|(name, celsius)| {
        celsius.map(|c| tr!("card-celsius", name = name, value = format!("{c:.0}")))
    })
    .collect();
    (!readings.is_empty()).then(|| readings.join(" · "))
}

/// The time left as a person says it.
pub(super) fn left(seconds: u64) -> String {
    let minutes = seconds.div_ceil(60);
    match (minutes / 60, minutes % 60) {
        (0, 0) => tr!("card-almost-done"),
        (0, minutes) => tr!("card-minutes-left", minutes = minutes),
        (hours, 0) => tr!("card-hours-left", hours = hours),
        (hours, minutes) => tr!("card-time-left", hours = hours, minutes = minutes),
    }
}

pub(super) fn capitalised(word: &str) -> String {
    let mut letters = word.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

pub(super) fn colour_of(light: Light) -> Color32 {
    let colors = theme::colors();
    match light {
        Light::Working => colors.accent,
        Light::Ready => colors.ok,
        Light::Held => colors.warn,
        Light::Down => colors.danger,
        Light::Unknown => colors.text_low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::Trouble;

    fn printing() -> State {
        State {
            state: "printing".to_owned(),
            busy: true,
            stage: Some("exposing".to_owned()),
            file: Some("dragon.goo".to_owned()),
            progress: Some(0.15),
            layer: Some(120),
            layer_count: Some(800),
            remaining_s: Some(4800),
            uv_led_c: Some(34.4),
            ..State::default()
        }
    }

    #[test]
    fn a_print_under_way_names_its_layer_and_the_time_left() {
        let face = of_state(&printing());
        assert_eq!(face.word, "Exposing");
        assert_eq!(face.light, Light::Working);
        let job = face.job.expect("a print is under way");
        assert_eq!(job.file, "dragon.goo");
        assert_eq!(job.detail, "Layer 120 of 800 · 1 h 20 min left");
        assert_eq!(face.warmth.as_deref(), Some("Light source 34 °C"));
    }

    #[test]
    fn a_finished_print_is_shown_as_the_last_one_on_a_ready_printer() {
        let state = State {
            state: "idle".to_owned(),
            stage: Some("complete".to_owned()),
            file: Some("dragon.goo".to_owned()),
            progress: Some(1.0),
            ..State::default()
        };
        let face = of_state(&state);
        assert_eq!(face.word, "Ready");
        let job = face.job.expect("the last file is still named");
        assert_eq!(job.progress, None, "no bar for a print that is over");
        assert_eq!(job.detail, "Finished");
    }

    #[test]
    fn a_paused_print_is_lit_apart_from_one_running() {
        let state = State {
            stage: Some("paused".to_owned()),
            ..printing()
        };
        assert_eq!(of_state(&state).light, Light::Held);
        assert_ne!(colour_of(Light::Held), colour_of(Light::Working));
    }

    #[test]
    fn a_printer_not_heard_from_says_what_the_scan_knows() {
        assert_eq!(face(Reach::Answered, None).word, "Connecting");
        assert_eq!(face(Reach::Silent, None).light, Light::Down);
        let trouble = Reading::Failed(Trouble {
            message: "network call failed".to_owned(),
            unreachable: true,
        });
        assert_eq!(face(Reach::Answered, Some(&trouble)).word, "Not answering");
    }

    #[test]
    fn the_settings_line_carries_the_share_done() {
        let reading = Reading::State(printing());
        let (line, tint) = summary(Reach::Answered, Some(&reading));
        assert_eq!(line, "Exposing, dragon.goo, 15%");
        assert_eq!(
            tint,
            theme::colors().accent,
            "a print running is in the accent"
        );
    }

    #[test]
    fn the_time_left_rounds_up_to_the_minute() {
        assert_eq!(left(0), "Almost done");
        assert_eq!(left(61), "2 min left");
        assert_eq!(left(3600), "1 h left");
        assert_eq!(left(3660), "1 h 1 min left");
    }
}
