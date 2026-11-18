//! The sheet that writes a bug report: what the user types, what the window adds, and the
//! clipboard, file or prefilled issue. Recomposed every frame, see `docs/decisions/0212`.

use egui::{Align, Layout};

use crate::i18n::tr;
use crate::report::{FILE_NAME, Facts, Plate, Report};
use crate::scene::Scene;
use crate::state::{Machine, Tools};
use crate::status::Status;
use crate::tool_settings::ToolSettings;
use crate::ui::theme::layout::report::{
    ANSWER_H, BODY_MAX_H, BODY_MIN_H, CHROME_H, SCREEN_MARGIN, SHEET_MIN_W, SHEET_W,
};
use crate::ui::{
    Inline, dialog, dialog_head, hairline, heading, hint, icon, inline_button, notice,
    quiet_button, section, switch, text_area, text_block, theme,
};

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
        printer_shipped: shipped(slicing),
        resin: &slicing.material,
        resin_id: slicing.resin_id.as_deref(),
        format: slicing.format.extension(),
        message: status.is_error().then(|| status.text().to_owned()),
        hosts: &hosts,
        plate: counted(scene),
        settings: &settings,
    };

    // Sized to the window: capped well short of the full height so it stays a sheet, and
    // shrinking on a screen that cannot hold the cap.
    let screen = ctx.viewport_rect();
    let sheet_w = (screen.width() - SCREEN_MARGIN).clamp(SHEET_MIN_W, SHEET_W);
    let body_h = (screen.height() - SCREEN_MARGIN - CHROME_H).clamp(BODY_MIN_H, BODY_MAX_H);
    let sheet = dialog(ctx, egui::Id::new("report"), 18).show(ctx, |ui| {
        ui.set_width(sheet_w);
        let close = dialog_head(ui, icon::BUG, &tr!("report-title"));
        hairline(ui);
        let text = egui::ScrollArea::vertical()
            .max_height(body_h)
            .auto_shrink([false, true])
            .show(ui, |ui| body(ui, report, &facts))
            .inner;
        hairline(ui);
        handed_over(ui, report, &facts, &text, status);
        close
    });

    if sheet.inner || sheet.should_close() {
        report.open = false;
        report.handed = false;
        report.crash = None;
    }
}

/// What the user writes and what goes with it, then the report itself; returns the report.
fn body(ui: &mut egui::Ui, report: &mut Report, facts: &Facts<'_>) -> String {
    ui.add_space(theme::ITEM_GAP);
    // The privacy line first, so it says what does not happen before anything is typed
    // (ADR 0212).
    hint(ui, &tr!("report-stays-here"));
    ui.add_space(theme::ITEM_GAP);
    told_state(ui, report);
    text_area(ui, &tr!("report-what-happened"), &mut report.what, ANSWER_H);
    ui.add_space(theme::LINE_GAP);
    text_area(ui, &tr!("report-steps"), &mut report.steps, ANSWER_H);
    ui.add_space(theme::PANEL_PAD);

    section(ui, &tr!("report-include"), None, |ui| {
        let include = &mut report.include;
        switch(ui, &mut include.profiles, &tr!("report-include-profiles"));
        switch(ui, &mut include.settings, &tr!("report-include-settings"));
        switch(ui, &mut include.plate, &tr!("report-include-plate"));
    });
    // Composed after the fields, so that a key pressed this frame is already in it.
    let text = report.markdown(facts);
    // A heading, not a section: a section's rule under it would double the one over the foot.
    ui.add_space(theme::ITEM_GAP);
    heading(ui, &tr!("report-the-report"), None);
    ui.add_space(theme::NOTE_GAP);
    text_block(ui, &text);
    ui.add_space(theme::ITEM_GAP);
    text
}

/// A verdict over the questions: the report went to the issue form, or the last run
/// crashed and the crash is already in it.
fn told_state(ui: &mut egui::Ui, report: &Report) {
    let colors = theme::colors();
    let (glyph, tint, title, why) = if report.handed {
        (
            icon::CLIPBOARD,
            colors.ok,
            tr!("report-copied"),
            tr!("report-handed-why"),
        )
    } else if report.crash.is_some() {
        (
            icon::WARNING,
            colors.warn,
            tr!("report-crashed"),
            tr!("report-crashed-why"),
        )
    } else {
        return;
    };
    notice(ui, glyph, tint, &title, &why);
    ui.add_space(theme::PANEL_PAD);
}

/// The three ways out: a file at the left, the clipboard and the issue form at the right.
/// Opening an issue copies the report as well, for the form's field when it does not fit.
fn handed_over(
    ui: &mut egui::Ui,
    report: &mut Report,
    facts: &Facts<'_>,
    text: &str,
    status: &mut Status,
) {
    ui.add_space(theme::ITEM_GAP);
    ui.horizontal(|ui| {
        if quiet_button(ui, icon::SAVE, &tr!("report-save"), false, true).clicked() {
            save(text, status);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = tr!("report-open-issue");
            let issue = Inline::new(&label).glyph(icon::ELSEWHERE).primary();
            if inline_button(ui, issue)
                .on_hover_text(tr!("report-open-issue-tooltip"))
                .clicked()
            {
                ui.ctx().copy_text(text.to_owned());
                ui.ctx()
                    .open_url(egui::OpenUrl::new_tab(report.issue_url(facts)));
                report.handed = true;
            }
            if inline_button(ui, Inline::new(&tr!("report-copy")).glyph(icon::CLIPBOARD)).clicked()
            {
                ui.ctx().copy_text(text.to_owned());
                *status = Status::Info(tr!("report-copied"));
            }
        });
    });
}

/// Whether the printer in hand is the one this build ships under its id.
fn shipped(slicing: &crate::slicing::Slicing) -> bool {
    let (Some(printer), Some(id)) = (&slicing.printer, &slicing.printer_id) else {
        return false;
    };
    slicing.catalogue.shipped_printer(id).as_ref() == Some(printer)
}

/// What stands on the plate being edited, as the report counts it: a hollowed model counts
/// the shell it prints as.
fn counted(scene: &Scene) -> Plate {
    let mut plate = Plate::default();
    for object in scene.printable(scene.active_plate()) {
        let mesh = object.printed();
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
        .with_context(|| tr!("report-cannot-write", path = path.display().to_string()));
    let _ = status.report(
        &tr!("status-saved", name = path.display().to_string()),
        written,
    );
}

/// A browser asks where a download goes itself, so the report is only named.
#[cfg(target_arch = "wasm32")]
fn save(text: &str, status: &mut Status) {
    use anyhow::Context as _;

    let handed = crate::web::files::download(FILE_NAME, text.as_bytes())
        .map_err(|error| anyhow::Error::new(crate::web::opfs::failed(error)))
        .context(tr!("report-cannot-hand-over"));
    let _ = status.report(&tr!("status-saved", name = FILE_NAME), handed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_plate_counts_nothing() {
        assert_eq!(counted(&Scene::default()), Plate::default());
    }
}
