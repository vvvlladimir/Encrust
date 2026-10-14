//! One printer over the window, everything about it at once: what it is printing with a
//! picture of the plate, the buttons that pause or stop it, its readings and camera, and
//! the prints it remembers. Opened from the ring in the top bar; see
//! `docs/decisions/0264`.

use egui::RichText;

use super::{colour_of, controls, face, history, job_lines, picture_of, trouble_lines};
use crate::network::{Errand, Network, Reading, Target};
use crate::panels::Window;
use crate::ui::{dialog_frame, dialog_head, hairline, icon, subheading, theme, tone};

/// How wide the sheet is, and the picture of the plate at its left.
const SHEET_W: f32 = 620.0;
const PICTURE: egui::Vec2 = egui::vec2(240.0, 180.0);
const COLUMN_GAP: f32 = 18.0;
/// What the sheet leaves of the window's height for its margin, and the least of the
/// history it shows before that scrolls.
const CHROME_H: f32 = 120.0;
const HISTORY_MIN_H: f32 = 96.0;

/// Shows the printer the ring opened, while one is open, and carries out what it is
/// pressed for.
pub fn ui(ctx: &egui::Context, window: &mut Window) {
    let network = &mut window.machine.network;
    let Some(key) = network.open.clone() else {
        return;
    };
    let Some(target) = network.targets().into_iter().find(|t| t.key() == key) else {
        network.open = None;
        return;
    };
    let mut close = false;
    let mut errand = None;
    let modal = egui::Modal::new(egui::Id::new("printer-sheet"))
        .frame(dialog_frame().inner_margin(egui::Margin::same(18)))
        .show(ctx, |ui| {
            ui.set_width(SHEET_W);
            close = dialog_head(ui, icon::PRINTER, &target.name());
            tone(ui, &about(&target), theme::colors().text_low);
            ui.add_space(8.0);
            hairline(ui);
            ui.add_space(12.0);
            errand = body(ui, network, &target);
        });
    if close || modal.should_close() {
        network.open = None;
    }
    if let Some(errand) = errand {
        network.ask(&key, errand);
    }
}

/// The picture and the camera at the left, the print and its buttons at the right, the
/// prints it remembers under both.
fn body(ui: &mut egui::Ui, network: &Network, target: &Target<'_>) -> Option<Errand> {
    let key = target.key();
    let reading = network.reading(target);
    let extras = network.extras(&key);
    let face = face(network.reach(target), reading);
    let state = match reading {
        Some(Reading::State(state)) => Some(state),
        _ => None,
    };
    let mut errand = None;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(PICTURE.x);
            let texture = state
                .and_then(|state| state.job_id.as_ref())
                .and_then(|id| extras?.pictures.get(id)?.texture());
            match texture {
                Some(texture) => picture_of(ui, texture, PICTURE),
                None => empty_picture(ui),
            }
            if let Some(state) = state {
                errand = errand.take().or(controls::camera(ui, state, extras));
            }
        });
        ui.add_space(COLUMN_GAP);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(&face.word)
                    .font(theme::dialog_title())
                    .color(colour_of(face.light)),
            );
            if let Some(job) = &face.job {
                job_lines(ui, job);
            }
            if let Some(state) = state {
                ui.add_space(4.0);
                errand = errand.take().or(controls::buttons(ui, &key, state, extras));
                ui.add_space(4.0);
                if let Some(warmth) = &face.warmth {
                    tone(ui, warmth, theme::colors().text_mid);
                }
                controls::film(ui, state);
            }
            if let Some(Reading::Failed(trouble)) = reading {
                trouble_lines(ui, trouble);
            }
            if let Some(trouble) = extras.and_then(|extras| extras.trouble.as_ref()) {
                trouble_lines(ui, trouble);
            }
        });
    });
    if state.is_some() && matches!(target, Target::Sdcp(_)) {
        ui.add_space(14.0);
        hairline(ui);
        ui.add_space(6.0);
        subheading(ui, "Recent prints");
        // As tall as the tiles, up to what the window has left under them: a short window
        // scrolls the history rather than pushing the sheet off the screen.
        let prints = extras
            .and_then(|extras| extras.history.as_ref())
            .map_or(1, Vec::len);
        let wanted = history::height(ui.available_width(), prints) + HISTORY_MIN_H / 2.0;
        let left = ui.ctx().content_rect().bottom() - CHROME_H / 2.0 - ui.cursor().top();
        let height = wanted.min(left.max(HISTORY_MIN_H));
        let listed = egui::ScrollArea::vertical()
            .max_height(height)
            .min_scrolled_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| history::list(ui, extras))
            .inner;
        errand = errand.take().or(listed);
    }
    errand
}

/// Where the picture of the plate goes before the printer has sent one.
fn empty_picture(ui: &mut egui::Ui) {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(PICTURE, egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, theme::R_CONTROL, colors.sunken);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon::PRINTER,
        theme::icon(36.0),
        colors.text_low,
    );
}

/// The model, where it is and what it runs, on one line under the name.
fn about(target: &Target<'_>) -> String {
    match target {
        Target::Sdcp(printer) => {
            let mut parts = vec![printer.model.clone(), printer.address.to_string()];
            if !printer.firmware.is_empty() {
                parts.push(format!("firmware {}", printer.firmware));
            }
            parts.retain(|part| !part.is_empty());
            parts.join(" · ")
        }
        Target::Prusa(_) => target.detail(),
    }
}
