//! A printer on the network as the window shows it: a row in the plate's card, the ring in the
//! top bar and the sheet the ring opens. See `docs/decisions/0264`.

mod controls;
mod face;
mod history;
pub mod ring;
pub mod sheet;

use egui::{Rect, pos2, vec2};

pub use face::summary;
use face::{Face, Job, Light, colour_of, face, left, paused};

use crate::i18n::tr;
use crate::network::{Network, Reading, Trouble};
use crate::ui::{DEVICE_OPEN_INSET, DeviceRow, device_row, share_figure, title_line};
use crate::ui::{
    DropdownRow, Lead, chooser_foot, dropdown_menu, dropdown_row, hairline, hint, icon,
    icon_button, progress_bar_of, theme, tone,
};

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

/// Where the printer in hand sends its files, as one row with its sheet beside it; a press
/// lists the profile's printers and scans, failures wait behind the status word.
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
            let face = machine_face(network, chosen_machine);
            let opens = target.is_some();
            let row = destination_row(ui, chosen_machine.label(), &face, &notes, opens);
            // The sheet's square stands inside the row, over it, so it takes its own press.
            let square = Rect::from_center_size(
                pos2(row.rect.right() - DEVICE_OPEN_INSET, row.rect.center().y),
                vec2(theme::ICON_SIZE, theme::ICON_SIZE),
            );
            if let Some(target) = &target {
                let open = ui.scope_builder(egui::UiBuilder::new().max_rect(square), |ui| {
                    icon_button(ui, icon::OPEN_SHEET, &tr!("card-open-printer")).clicked()
                });
                if open.inner {
                    chosen = Some(Chosen::Open(target.key()));
                }
            }
            dropdown_menu(&row, |ui| {
                for machine in network.listed() {
                    let face = machine_face(network, machine);
                    let row = DropdownRow {
                        lead: Some(Lead::Dot(colour_of(face.light))),
                        name: machine.label(),
                        aside: Some(&face.word),
                        chosen: machine.address == chosen_machine.address,
                    };
                    if dropdown_row(ui, &row).clicked() {
                        chosen = Some(Chosen::Pick(machine.address.clone()));
                    }
                }
                hairline(ui);
                let scanning = network.is_scanning();
                let scan = ui.add_enabled_ui(!scanning, |ui| {
                    chooser_foot(ui, icon::RESCAN, &tr!("connection-scan-again")).clicked()
                });
                if scan.inner {
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

/// The row on `raised`, its state from `face`; the status word's hover shows `notes`.
fn destination_row(
    ui: &mut egui::Ui,
    name: &str,
    face: &Face,
    notes: &[Note],
    opens: bool,
) -> egui::Response {
    let row = DeviceRow {
        name,
        state: &face.word,
        tint: colour_of(face.light),
        busy: face.light == Light::Working,
        opens,
    };
    let (response, status) = device_row(ui, &row);
    if !notes.is_empty() && ui.rect_contains_pointer(status) {
        response.on_hover_ui(|ui| note_lines(ui, notes))
    } else {
        response.on_hover_text(tr!("card-destination-several"))
    }
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

/// The file, the share done and its bar, and the line under them while it prints; one line for the last
/// print once it is over.
fn job_lines(ui: &mut egui::Ui, job: &Job) {
    let colors = theme::colors();
    let Some(progress) = job.progress else {
        let line = tr!(
            "printer-last-print",
            how = job.detail.clone(),
            file = job.file.clone()
        );
        tone(ui, &line, colors.text_mid);
        return;
    };
    title_line(ui, &job.file);
    share_figure(
        ui,
        &tr!("value-percent", value = format!("{:.0}", progress * 100.0)),
    );
    progress_bar_of(ui, Some(progress), theme::SHEET_BAR_H);
    tone(ui, &job.detail, colors.text_mid);
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
