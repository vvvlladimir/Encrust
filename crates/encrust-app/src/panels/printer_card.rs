//! A printer on the network as the window shows it: a row in the plate's card, the ring in the
//! top bar and the sheet the ring opens. See `docs/decisions/0264`.

mod controls;
mod history;
pub mod ring;
pub mod sheet;

use egui::{Align2, Color32, RichText, Sense, StrokeKind, pos2, vec2};
use printer_link::State;

use crate::i18n::tr;
use crate::network::{Network, Reach, Reading, Trouble};
use crate::ui::{focus_ring, hint, icon, icon_button, progress_bar_of, theme, tone};

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

/// What the plate card's printer row was pressed for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen {
    /// Send this profile's files to the listed printer at that address from now on.
    Pick(String),
    /// Ask the network again, for a printer switched on since the last scan.
    Scan,
    /// Open the sheet of the printer under that key.
    Open(String),
}

/// Where the printer in hand sends its files, as one row: its name and state, and its
/// sheet beside it; a press opens the printers the profile lists and a new scan.
/// Failures wait behind the status word.
pub fn destination(
    ui: &mut egui::Ui,
    network: &Network,
    warning: Option<String>,
) -> Option<Chosen> {
    let chosen_machine = network.chosen_machine()?;
    let target = network.resolve(chosen_machine);
    let mut notes: Vec<Note> = warning.into_iter().map(Note::plain).collect();
    notes.extend(machine_notes(network, chosen_machine));
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::BUTTON_GAP;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(target) = &target
                && icon_button(ui, icon::OPEN_SHEET, &tr!("card-open-printer")).clicked()
            {
                chosen = Some(Chosen::Open(target.key()));
            }
            let face = machine_face(network, chosen_machine);
            let row = destination_row(ui, chosen_machine.label(), &face, &notes);
            egui::Popup::menu(&row).show(|ui| {
                for machine in network.listed() {
                    let face = machine_face(network, machine);
                    let line = format!("{}  ·  {}", machine.label(), face.word);
                    let on = machine.address == chosen_machine.address;
                    if ui.selectable_label(on, line).clicked() {
                        chosen = Some(Chosen::Pick(machine.address.clone()));
                    }
                }
                ui.separator();
                let scan = egui::Button::new(format!(
                    "{}  {}",
                    icon::RESCAN,
                    tr!("connection-scan-again")
                ));
                if ui.add_enabled(!network.is_scanning(), scan).clicked() {
                    chosen = Some(Chosen::Scan);
                }
            });
        });
    });
    chosen
}

/// One thing that went wrong, and what the user can do about it when there is something.
#[derive(Debug, Clone, PartialEq)]
struct Note {
    line: String,
    advice: Option<String>,
}

impl Note {
    fn plain(line: String) -> Self {
        Self { line, advice: None }
    }

    fn of(trouble: &Trouble) -> Self {
        Self {
            line: trouble.message.clone(),
            advice: trouble.advice(),
        }
    }
}

/// What went wrong reaching one listed printer: its own failure, the broadcast that went
/// nowhere, and the scan finding nothing at its address.
fn machine_notes(network: &Network, machine: &printer_profiles::NetworkMachine) -> Vec<Note> {
    let mut notes = Vec::new();
    if let Some(target) = network.resolve(machine)
        && let Some(Reading::Failed(trouble)) = network.reading(&target)
    {
        notes.push(Note::of(trouble));
    }
    if let Some(trouble) = &network.trouble {
        notes.push(Note::plain(tr!("card-search-failed")));
        notes.push(Note::of(trouble));
    }
    let address = machine.address.trim();
    for (silent, trouble) in &network.silent {
        if silent.to_string() != address {
            continue;
        }
        notes.push(Note::plain(tr!(
            "card-nothing-answered",
            address = silent.to_string()
        )));
        notes.extend(trouble.as_ref().map(Note::of));
    }
    notes
}

/// What a listed printer is doing, or that it was not found.
fn machine_face(network: &Network, machine: &printer_profiles::NetworkMachine) -> Face {
    match network.resolve(machine) {
        Some(target) => face(network.reach(&target), network.reading(&target)),
        None => face(network.reach_of(machine), None),
    }
}

/// The row itself, on `raised`: the printer glyph, its name, what it is doing in its colour
/// and the caret of the list it opens. Hovering the status word shows `notes`.
fn destination_row(ui: &mut egui::Ui, name: &str, face: &Face, notes: &[Note]) -> egui::Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), theme::PRINTER_ROW_H),
        Sense::click(),
    );
    let fill = if response.hovered() {
        colors.hover
    } else {
        colors.raised
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        theme::R_CONTROL,
        fill,
        egui::Stroke::new(1.0, colors.hairline),
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
    let status = paint_status(painter, face, rect, rect.right() - 30.0);
    let ink = colors.text_high;
    let room = (status.left() - 8.0 - rect.left() - 30.0).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(name.to_owned(), theme::label(), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let galley = painter.layout_job(job);
    painter.galley(
        pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0),
        galley,
        ink,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    let response = if !notes.is_empty() && ui.rect_contains_pointer(status) {
        response.on_hover_ui(|ui| note_lines(ui, notes))
    } else {
        response.on_hover_text(tr!("card-destination-several"))
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, name));
    response
}

/// How dim a busy printer's dot breathes down to.
const BREATH_LOW: f32 = 0.35;

/// The status word and its dot, right-aligned on `right` in the row; answers where they
/// stand, the full height of the row.
fn paint_status(painter: &egui::Painter, face: &Face, rect: egui::Rect, right: f32) -> egui::Rect {
    let colour = colour_of(face.light);
    let word = painter.layout_no_wrap(face.word.clone(), theme::small(), colour);
    let at = pos2(right - word.size().x, rect.center().y - word.size().y / 2.0);
    painter.galley(at, word, colour);
    let dot = pos2(at.x - 8.0, rect.center().y);
    let dot_colour = match face.light {
        Light::Working => {
            let breath = crate::ui::theme::motion::breathe(painter.ctx());
            colour.gamma_multiply(BREATH_LOW + (1.0 - BREATH_LOW) * breath)
        }
        _ => colour,
    };
    painter.circle_filled(dot, 3.5, dot_colour);
    egui::Rect::from_x_y_ranges(dot.x - 4.0..=right, rect.y_range())
}

fn note_lines(ui: &mut egui::Ui, notes: &[Note]) {
    ui.set_max_width(280.0);
    for note in notes {
        tone(ui, &note.line, theme::colors().warn);
        if let Some(advice) = &note.advice {
            hint(ui, advice);
        }
    }
}

/// The share done of a print in the sheet, pt.
const SHARE_PT: f32 = 22.0;

/// The file, the share done and its bar, and the line under them while it prints; one line for the last
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
    ui.add(
        egui::Label::new(
            RichText::new(&job.file)
                .font(theme::label())
                .color(colors.text_high),
        )
        .truncate(),
    );
    ui.label(
        RichText::new(format!("{:.0}%", progress * 100.0))
            .font(theme::figures(SHARE_PT))
            .color(colors.text_high),
    );
    progress_bar_of(ui, Some(progress), theme::SHEET_BAR_H);
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
        hint(ui, &advice);
    }
}

/// What the last scan could not do: a broadcast that went nowhere, an address that kept
/// quiet. Drawn under a list of printers in the printer settings.
pub fn scan_notes(ui: &mut egui::Ui, network: &Network) {
    if let Some(trouble) = &network.trouble {
        tone(ui, &tr!("card-search-failed"), theme::colors().warn);
        trouble_lines(ui, trouble);
    }
    for (address, trouble) in &network.silent {
        tone(
            ui,
            &tr!("card-nothing-answered", address = address.to_string()),
            theme::colors().warn,
        );
        if let Some(trouble) = trouble {
            trouble_lines(ui, trouble);
        }
    }
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
            format!("{}, {file}, {:.0}%", face.word, progress * 100.0),
            tint,
        ),
        _ => (face.word, tint),
    }
}

fn face(reach: Reach, reading: Option<&Reading>) -> Face {
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
fn paused(state: &State) -> bool {
    matches!(state.stage.as_deref(), Some("pausing" | "paused")) || state.state == "paused"
}

fn of_state(state: &State) -> Face {
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

fn job(state: &State) -> Option<Job> {
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

fn warmth(state: &State) -> Option<String> {
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
fn left(seconds: u64) -> String {
    let minutes = seconds.div_ceil(60);
    match (minutes / 60, minutes % 60) {
        (0, 0) => tr!("card-almost-done"),
        (0, minutes) => tr!("card-minutes-left", minutes = minutes),
        (hours, 0) => tr!("card-hours-left", hours = hours),
        (hours, minutes) => tr!("card-time-left", hours = hours, minutes = minutes),
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
