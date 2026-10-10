//! The sheet that writes a bug report: what the user types, what the window adds, and the
//! three ways out — the clipboard, a file, or a prefilled issue.
//!
//! The report is composed again every frame the sheet is open, so what is shown is what
//! the buttons hand over; see `docs/decisions/0212`.

use egui::{Align, Layout, RichText};

use crate::report::{FILE_NAME, Facts, Plate, Report};
use crate::scene::Scene;
use crate::state::{Machine, Tools};
use crate::status::Status;
use crate::tool_settings::ToolSettings;
use crate::ui::{hairline, hint, icon, inline_button, switch, theme};

/// The sheet's width where there is room for it, and the narrowest it is still drawn at.
const SHEET_W: f32 = 520.0;
const SHEET_MIN_W: f32 = 300.0;
/// Left around the sheet so it never reaches the window's own edges.
const SCREEN_MARGIN: f32 = 96.0;
/// How tall a typed answer is drawn before it scrolls.
const ANSWER_H: f32 = 56.0;
/// The scrolling body between the header and the buttons: its bounds, and the height those
/// two take out of the window before it is measured.
const BODY_MAX_H: f32 = 420.0;
const BODY_MIN_H: f32 = 160.0;
const CHROME_H: f32 = 110.0;
/// The report pane's share of the body, and the least it is left with on a short window.
const REPORT_SHARE: f32 = 0.45;
const REPORT_MIN_H: f32 = 120.0;

/// What the sheet says above the fields. It is the privacy notice as well, which is why it
/// states what does not happen before it asks for anything; see `docs/decisions/0212`.
const NOTHING_SENT: &str = "Encrust is in alpha, and a report is how a bug gets fixed. Nothing \
                            is sent from here: what you see below stays on this machine until \
                            you hand it over yourself.";

const NO_MODEL: &str = "Your model is never in the report. Attach it to the issue yourself only \
                        if it is yours to share — a small shape that shows the same bug is \
                        better for everyone.";

/// Shows the sheet over the window, and closes it on Escape or a click outside.
pub fn ui(ctx: &egui::Context, scene: &Scene, tools: &Tools, machine: &mut Machine) {
    if !machine.report.open {
        return;
    }
    let Machine {
        slicing,
        network,
        status,
        report,
        ..
    } = machine;

    // Composed from the profiles in hand, which is why they are gathered before the sheet
    // and not inside it.
    let settings = ToolSettings::of(tools, slicing);
    // The addresses are gathered here for the same reason: a failure that quotes one is
    // scrubbed of it before the report states it.
    let hosts = network.hosts();
    let facts = Facts {
        printer: slicing.printer.as_ref(),
        printer_id: slicing.printer_id.as_deref(),
        resin: &slicing.material,
        resin_id: slicing.resin_id.as_deref(),
        format: slicing.format.extension(),
        message: status.is_error().then(|| status.text().to_owned()),
        hosts: &hosts,
        plate: counted(scene),
        settings: &settings,
    };

    // The sheet is sized to the window first: it is capped well short of the full height so
    // it stays a sheet, and shrinks with its panes on a screen that cannot hold the cap.
    let screen = ctx.viewport_rect();
    let sheet_w = (screen.width() - SCREEN_MARGIN).clamp(SHEET_MIN_W, SHEET_W);
    let body_h = (screen.height() - SCREEN_MARGIN - CHROME_H).clamp(BODY_MIN_H, BODY_MAX_H);
    let report_h = (body_h * REPORT_SHARE).max(REPORT_MIN_H);
    let sheet = egui::Modal::new(egui::Id::new("report"))
        .frame(
            egui::Frame::new()
                .fill(theme::colors().panel)
                .corner_radius(theme::R_SURFACE)
                .inner_margin(egui::Margin::same(18))
                .shadow(theme::shadow()),
        )
        .show(ctx, |ui| {
            ui.set_width(sheet_w);
            header(ui, report);
            ui.add_space(10.0);

            let text = egui::ScrollArea::vertical()
                .max_height(body_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    hint(ui, NOTHING_SENT);
                    ui.add_space(12.0);
                    asked(
                        ui,
                        "What happened, and what you expected instead",
                        &mut report.what,
                    );
                    asked(ui, "How to reproduce it", &mut report.steps);
                    carried(ui, report);
                    ui.add_space(8.0);
                    hint(ui, NO_MODEL);
                    ui.add_space(12.0);

                    // Drawn after the fields so that a key pressed this frame is already in it.
                    let text = report.markdown(&facts);
                    shown(ui, &text, report_h);
                    text
                })
                .inner;
            ui.add_space(12.0);
            handed_over(ui, report, &facts, &text, status);
        });

    if sheet.should_close() {
        report.open = false;
    }
}

fn header(ui: &mut egui::Ui, report: &mut Report) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Report a problem")
                .font(theme::body())
                .color(theme::colors().text_high),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if crate::ui::icon_button(ui, icon::CANCEL, "Close").clicked() {
                report.open = false;
            }
        });
    });
    ui.add_space(8.0);
    hairline(ui);
}

/// One of the two questions the issue form asks, and the room to answer it in.
fn asked(ui: &mut egui::Ui, question: &str, answer: &mut String) {
    ui.label(
        RichText::new(question)
            .font(theme::small())
            .color(theme::colors().text_mid),
    );
    ui.add_space(4.0);
    ui.add_sized(
        egui::vec2(ui.available_width(), ANSWER_H),
        egui::TextEdit::multiline(answer)
            .font(theme::label())
            .background_color(theme::colors().raised),
    );
    ui.add_space(10.0);
}

/// The three switches over what the window adds of its own.
fn carried(ui: &mut egui::Ui, report: &mut Report) {
    let include = &mut report.include;
    switch(ui, &mut include.profiles, "The printer and resin profiles");
    switch(ui, &mut include.settings, "What the tool panels are set to");
    switch(ui, &mut include.plate, "How much stands on the plate");
}

/// The report itself, whole and scrollable: a user who can read what they are handing
/// over does not have to trust this window about it.
fn shown(ui: &mut egui::Ui, text: &str, height: f32) {
    crate::ui::heading(ui, "The report", None);
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(theme::colors().base)
        .stroke(egui::Stroke::new(1.0, theme::colors().hairline))
        .corner_radius(theme::R_CONTROL)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text)
                            .font(theme::code(11.0))
                            .color(theme::colors().text_mid),
                    );
                });
        });
}

/// The three ways out. Opening an issue copies the report as well: the form has room for
/// what the user typed, and the rest has to be pasted into it.
fn handed_over(
    ui: &mut egui::Ui,
    report: &Report,
    facts: &Facts<'_>,
    text: &str,
    status: &mut Status,
) {
    ui.horizontal(|ui| {
        if inline_button(ui, icon::BUG, "Open an issue", true, true).clicked() {
            ui.ctx().copy_text(text.to_owned());
            ui.ctx()
                .open_url(egui::OpenUrl::new_tab(report.issue_url(facts)));
            *status =
                Status::Info("The report is on your clipboard: paste it into the issue".to_owned());
        }
        ui.add_space(6.0);
        if inline_button(ui, icon::CLIPBOARD, "Copy the report", false, true).clicked() {
            ui.ctx().copy_text(text.to_owned());
            *status = Status::Info("The report is on your clipboard".to_owned());
        }
        if inline_button(ui, icon::SAVE, "Save it as a file", false, true).clicked() {
            save(text, status);
        }
    });
}

/// What stands on the plate being edited, as the report counts it: a hollowed model counts
/// the shell it prints as.
fn counted(scene: &Scene) -> Plate {
    let mut plate = Plate::default();
    for object in scene.printable(scene.active_plate()) {
        let mesh = object.hollow.shell().unwrap_or(&object.mesh);
        plate.models += 1;
        plate.triangles += mesh.faces.len();
        plate.vertices += mesh.vertices.len();
    }
    plate
}

#[cfg(not(target_arch = "wasm32"))]
fn save(text: &str, status: &mut Status) {
    use anyhow::Context as _;

    let Some(path) = rfd::FileDialog::new()
        .add_filter("Markdown", &["md"])
        .set_file_name(FILE_NAME)
        .save_file()
    else {
        return;
    };
    let written = std::fs::write(&path, text)
        .with_context(|| format!("cannot write the report to {}", path.display()));
    let _ = status.report(&format!("Saved {}", path.display()), written);
}

/// A browser asks where a download goes itself, so the report is only named.
#[cfg(target_arch = "wasm32")]
fn save(text: &str, status: &mut Status) {
    use anyhow::Context as _;

    let handed = crate::web::files::download(FILE_NAME, text.as_bytes())
        .map_err(|error| anyhow::Error::new(crate::web::opfs::failed(error)))
        .context("cannot hand over the report");
    let _ = status.report(&format!("Saved {FILE_NAME}"), handed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_plate_counts_nothing() {
        assert_eq!(counted(&Scene::default()), Plate::default());
    }
}
