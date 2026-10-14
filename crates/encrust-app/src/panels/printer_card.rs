//! One printer on the network as a card: who it is, whether it answers, what it is
//! printing with a picture of it, the buttons that pause or stop it, its camera, its film
//! and the prints it remembers. The Export tool lists them; the printer settings say the
//! same in a line.

mod controls;
mod history;

use egui::{Color32, RichText};
use printer_link::State;

use crate::network::{Errand, Extras, Network, Reach, Reading, Target, Trouble};
use crate::ui::{hint, icon, progress_bar, quiet_button, theme, tone};

/// How big a picture of the print under way is drawn, in points.
const PICTURE: egui::Vec2 = egui::vec2(72.0, 54.0);

/// What the card says about one printer, worked out before anything is drawn.
#[derive(Debug, Clone, PartialEq)]
struct Face {
    /// The word in the corner, and how it is lit.
    word: String,
    light: Light,
    /// The print under way or the last one, while the printer names it.
    job: Option<Job>,
    /// Temperatures, on one line.
    warmth: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Light {
    Working,
    Ready,
    Held,
    Down,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
struct Job {
    file: String,
    /// Share done, 0 to 1, while it is being printed.
    progress: Option<f32>,
    /// The line under the bar: the layer and the time left, or how it ended.
    detail: String,
}

/// Everything one card is drawn from.
pub struct Card<'a> {
    pub target: &'a Target<'a>,
    pub reach: Reach,
    pub reading: Option<&'a Reading>,
    pub extras: Option<&'a Extras>,
    /// Marks the printer the profile in hand is bound to.
    pub lit: bool,
    /// Offers to bind the profile in hand to this printer.
    pub pickable: bool,
}

/// What a card was pressed for, for the caller to carry out.
#[derive(Debug, Default)]
pub struct Pressed {
    pub picked: bool,
    pub errand: Option<Errand>,
}

/// Draws one printer.
pub fn card(ui: &mut egui::Ui, card: &Card<'_>) -> Pressed {
    let colors = theme::colors();
    let face = face(card.reach, card.reading);
    let (fill, stroke) = if card.lit {
        (colors.accent_wash, colors.accent_soft)
    } else {
        (colors.raised, colors.hairline)
    };
    let mut pressed = Pressed::default();
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(theme::R_CONTROL)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            head(ui, card.target, &face);
            pressed.errand = body(ui, card, &face);
            if card.pickable {
                let pick = quiet_button(
                    ui,
                    icon::SEND,
                    "Send this printer's files here",
                    false,
                    true,
                );
                pressed.picked = pick.clicked();
            }
        });
    pressed
}

/// Everything under the head: the print, its buttons, the readings, the camera, the
/// history, and what went wrong.
fn body(ui: &mut egui::Ui, card: &Card<'_>, face: &Face) -> Option<Errand> {
    let key = card.target.key();
    let mut errand = None;
    if let Some(job) = &face.job {
        ui.add_space(4.0);
        let picture = match card.reading {
            Some(Reading::State(state)) => state
                .job_id
                .as_ref()
                .and_then(|id| card.extras?.pictures.get(id)?.texture()),
            _ => None,
        };
        job_lines(ui, job, picture);
    }
    if let Some(Reading::State(state)) = card.reading {
        errand = errand.or(controls::buttons(ui, &key, state, card.extras));
        if let Some(warmth) = &face.warmth {
            tone(ui, warmth, theme::colors().text_low);
        }
        controls::film(ui, state);
        errand = errand.or(controls::camera(ui, state, card.extras));
        if matches!(card.target, Target::Sdcp(_)) {
            errand = errand.or(history::fold(ui, &key, card.extras));
        }
    }
    if let Some(Reading::Failed(trouble)) = card.reading {
        trouble_lines(ui, trouble);
    }
    if let Some(trouble) = card.extras.and_then(|extras| extras.trouble.as_ref()) {
        trouble_lines(ui, trouble);
    }
    errand
}

/// The icon, the name over the model and address, and the state word at the right.
fn head(ui: &mut egui::Ui, target: &Target<'_>, face: &Face) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(icon::PRINTER)
                .font(theme::icon(18.0))
                .color(colors.text_mid),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            state_word(ui, face);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add(
                    egui::Label::new(
                        RichText::new(target.name())
                            .font(theme::label())
                            .color(colors.text_high),
                    )
                    .truncate(),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(subtitle(target))
                            .font(theme::small())
                            .color(colors.text_low),
                    )
                    .truncate(),
                );
            });
        });
    });
}

/// A dot and a word, in the colour of what the printer is doing.
fn state_word(ui: &mut egui::Ui, face: &Face) {
    let colour = colour_of(face.light);
    ui.label(RichText::new(&face.word).font(theme::small()).color(colour));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, colour);
}

/// The file over its bar and the line under it, while it prints, beside the picture of
/// it once the printer has sent one; one line for the last print once it is over.
fn job_lines(ui: &mut egui::Ui, job: &Job, picture: Option<&egui::TextureHandle>) {
    let colors = theme::colors();
    let Some(progress) = job.progress else {
        tone(
            ui,
            &format!("{}: {}", job.detail, job.file),
            colors.text_mid,
        );
        return;
    };
    let lines = |ui: &mut egui::Ui| {
        ui.horizontal(|ui| {
            let share = RichText::new(format!("{:.0}%", progress * 100.0))
                .font(theme::figures(12.0))
                .color(colors.text_high);
            let share_w = 40.0;
            let file = egui::Label::new(
                RichText::new(&job.file)
                    .font(theme::label())
                    .color(colors.text_high),
            )
            .truncate();
            let file_w = (ui.available_width() - share_w).max(0.0);
            ui.allocate_ui(egui::vec2(file_w, theme::ROW_H), |ui| ui.add(file));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(share);
            });
        });
        progress_bar(ui, Some(progress));
        tone(ui, &job.detail, colors.text_mid);
    };
    match picture {
        Some(texture) => {
            ui.horizontal(|ui| {
                picture_of(ui, texture, PICTURE);
                ui.vertical(lines);
            });
        }
        None => lines(ui),
    }
}

/// A printer's picture of a print, fitted into `size` and rounded like the card.
fn picture_of(ui: &mut egui::Ui, texture: &egui::TextureHandle, size: egui::Vec2) {
    ui.add(
        egui::Image::new(texture)
            .fit_to_exact_size(size)
            .corner_radius(theme::R_CONTROL),
    );
}

fn trouble_lines(ui: &mut egui::Ui, trouble: &Trouble) {
    tone(ui, &trouble.message, theme::colors().warn);
    if let Some(advice) = trouble.advice() {
        hint(ui, advice);
    }
}

/// What the last scan could not do: a broadcast that went nowhere, an address that kept
/// quiet. Drawn under a list of printers, in the Export tool and the printer settings.
pub fn scan_notes(ui: &mut egui::Ui, network: &Network) {
    if let Some(trouble) = &network.trouble {
        tone(
            ui,
            "The search could not go out on any network",
            theme::colors().warn,
        );
        trouble_lines(ui, trouble);
    }
    for (address, trouble) in &network.silent {
        tone(
            ui,
            &format!("Nothing answered at {address}"),
            theme::colors().warn,
        );
        if let Some(trouble) = trouble {
            trouble_lines(ui, trouble);
        }
    }
}

/// One line for a printer's state, for the list in the printer settings.
pub fn summary(reach: Reach, reading: Option<&Reading>) -> String {
    let face = face(reach, reading);
    match face.job {
        Some(Job {
            file,
            progress: Some(progress),
            ..
        }) => format!("{}, {file}, {:.0}%", face.word, progress * 100.0),
        _ => face.word,
    }
}

/// The model and where it is, or what a Prusa machine said it is.
fn subtitle(target: &Target<'_>) -> String {
    match target {
        Target::Sdcp(printer) if !printer.model.is_empty() => {
            format!("{} · {}", printer.model, printer.address)
        }
        _ => target.detail(),
    }
}

fn face(reach: Reach, reading: Option<&Reading>) -> Face {
    match reading {
        Some(Reading::State(state)) => of_state(state),
        Some(Reading::Failed(_)) => Face {
            word: "Not answering".to_owned(),
            light: Light::Down,
            job: None,
            warmth: None,
        },
        None => Face {
            word: match reach {
                Reach::Answered => "Connecting",
                Reach::Silent => "Not answering",
                Reach::Unasked => "Not asked yet",
            }
            .to_owned(),
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
fn paused(state: &State) -> bool {
    matches!(state.stage.as_deref(), Some("pausing" | "paused")) || state.state == "paused"
}

fn of_state(state: &State) -> Face {
    let stage = state.stage.as_deref();
    let (word, light) = if paused(state) {
        ("Paused".to_owned(), Light::Held)
    } else if state.busy {
        (capitalised(stage.unwrap_or(&state.state)), Light::Working)
    } else {
        ("Ready".to_owned(), Light::Ready)
    };
    Face {
        word,
        light,
        job: job(state),
        warmth: warmth(state),
    }
}

fn job(state: &State) -> Option<Job> {
    let file = state.file.clone()?;
    if !state.busy {
        let ended = match (&state.error, state.stage.as_deref()) {
            (Some(error), _) => format!("Stopped: {error}"),
            (None, Some("complete")) => "Finished".to_owned(),
            (None, Some("stopped")) => "Stopped".to_owned(),
            (None, _) => "Last print".to_owned(),
        };
        return Some(Job {
            file,
            progress: None,
            detail: ended,
        });
    }
    let mut parts = Vec::new();
    if let (Some(layer), Some(count)) = (state.layer, state.layer_count) {
        parts.push(format!("Layer {layer} of {count}"));
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

fn warmth(state: &State) -> Option<String> {
    let readings: Vec<String> = [("Light source", state.uv_led_c), ("Hood", state.chamber_c)]
        .into_iter()
        .filter_map(|(name, celsius)| celsius.map(|c| format!("{name} {c:.0} °C")))
        .collect();
    (!readings.is_empty()).then(|| readings.join(" · "))
}

/// The time left as a person says it.
fn left(seconds: u64) -> String {
    let minutes = seconds.div_ceil(60);
    match (minutes / 60, minutes % 60) {
        (0, 0) => "Almost done".to_owned(),
        (0, minutes) => format!("{minutes} min left"),
        (hours, 0) => format!("{hours} h left"),
        (hours, minutes) => format!("{hours} h {minutes} min left"),
    }
}

fn capitalised(word: &str) -> String {
    let mut letters = word.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

fn colour_of(light: Light) -> Color32 {
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
        assert_eq!(
            summary(Reach::Answered, Some(&reading)),
            "Exposing, dragon.goo, 15%"
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
