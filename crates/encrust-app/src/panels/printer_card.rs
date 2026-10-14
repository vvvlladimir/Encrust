//! A printer on the network, in the parts the window shows it in: one row in the Export
//! tool naming where files go, the ring in the top bar, and the sheet the ring opens with
//! everything about it. See `docs/decisions/0264`.

mod controls;
mod history;
pub mod ring;
pub mod sheet;

use egui::{Align2, Color32, RichText, Sense, StrokeKind, pos2, vec2};
use printer_link::State;
use printer_profiles::Connection;

use crate::network::{Network, Reach, Reading, Target, Trouble};
use crate::ui::{focus_ring, hint, icon, icon_button, progress_bar, theme, tone};

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

/// What the Export tool's row was pressed for, by the printer's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen {
    /// Send this profile's files to that printer from now on.
    Bind(String),
    /// Open that printer's sheet.
    Open(String),
}

/// Where the printer in hand sends its files, as one row: the bound printer's name and
/// what it is doing, opening a list of every printer speaking `speaks`, and beside it the
/// way into that printer's sheet.
pub fn destination(ui: &mut egui::Ui, network: &Network, speaks: Connection) -> Option<Chosen> {
    let bound = network.target().filter(|target| target.speaks(speaks));
    let choices: Vec<Target<'_>> = network
        .targets()
        .into_iter()
        .filter(|target| target.speaks(speaks))
        .collect();
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(target) = &bound
                && icon_button(ui, icon::OPEN_SHEET, "Open the printer").clicked()
            {
                chosen = Some(Chosen::Open(target.key()));
            }
            let row = destination_row(ui, network, bound.as_ref());
            let lit = bound.as_ref().map(Target::key);
            egui::Popup::menu(&row).show(|ui| {
                for target in &choices {
                    let line = format!(
                        "{}  ·  {}",
                        target.name(),
                        summary(network.reach(target), network.reading(target))
                    );
                    let on = lit.as_deref() == Some(target.key().as_str());
                    if ui.selectable_label(on, line).clicked() {
                        chosen = Some(Chosen::Bind(target.key()));
                    }
                }
            });
        });
    });
    chosen
}

/// The row itself: the printer glyph, the bound printer's name or an invitation to choose
/// one, what it is doing in its colour, and a caret.
fn destination_row(
    ui: &mut egui::Ui,
    network: &Network,
    bound: Option<&Target<'_>>,
) -> egui::Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::BUTTON_H), Sense::click());
    let border = if response.hovered() {
        colors.line
    } else {
        colors.hairline
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        theme::R_CONTROL,
        colors.sunken,
        egui::Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    painter.text(
        pos2(rect.left() + 16.0, rect.center().y),
        Align2::CENTER_CENTER,
        icon::PRINTER,
        theme::icon(14.0),
        colors.text_low,
    );
    painter.text(
        pos2(rect.right() - 16.0, rect.center().y),
        Align2::CENTER_CENTER,
        icon::CARET_DOWN,
        theme::icon(14.0),
        colors.text_low,
    );
    let (name, tint, state) = match bound {
        Some(target) => {
            let face = face(network.reach(target), network.reading(target));
            (target.name(), colors.text_high, Some(face))
        }
        None => ("Choose a printer".to_owned(), colors.text_low, None),
    };
    let mut right = rect.right() - 30.0;
    if let Some(face) = &state {
        let colour = colour_of(face.light);
        let word = painter.layout_no_wrap(face.word.clone(), theme::small(), colour);
        let at = pos2(right - word.size().x, rect.center().y - word.size().y / 2.0);
        right = at.x - 12.0;
        painter.galley(at, word, colour);
        painter.circle_filled(pos2(right + 6.0, rect.center().y), 3.5, colour);
        right -= 6.0;
    }
    let room = (right - rect.left() - 32.0).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(name.clone(), theme::label(), tint);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let galley = painter.layout_job(job);
    painter.galley(
        pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0),
        galley,
        tint,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    let response = response.on_hover_text("The printer this profile sends to");
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &name));
    response
}

/// The file over its bar and the line under it while it prints; one line for the last
/// print once it is over.
fn job_lines(ui: &mut egui::Ui, job: &Job) {
    let colors = theme::colors();
    let Some(progress) = job.progress else {
        tone(
            ui,
            &format!("{}: {}", job.detail, job.file),
            colors.text_mid,
        );
        return;
    };
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
