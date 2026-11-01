use egui::RichText;
use printer_link::{Ending, Task};

use super::picture_of;
use crate::i18n::tr;
use crate::network::{Errand, Extras};
use crate::ui::{ago, hint, icon, quiet_button, theme};

/// Tiles to a row, the gap between two, and a tile's height and picture, points.
const COLUMNS: usize = 3;
const TILE_GAP: f32 = 10.0;
const TILE_H: f32 = 60.0;
const PICTURE: f32 = 44.0;
const TILE_PAD: i8 = 8;

/// How wide each of a row's tiles is drawn in a row of `width`.
pub(super) fn tile_w(width: f32) -> f32 {
    ((width - TILE_GAP * (COLUMNS - 1) as f32) / COLUMNS as f32).max(0.0)
}

/// How tall the tiles for `prints` stand.
pub(super) fn height(prints: usize) -> f32 {
    prints.div_ceil(COLUMNS).max(1) as f32 * (TILE_H + TILE_GAP)
}

/// The prints the printer remembers as tiles, several to a row, each with its picture;
/// drawing it the first time is what asks for them.
pub(super) fn list(ui: &mut egui::Ui, extras: Option<&Extras>) -> Option<Errand> {
    let mut errand = None;
    let asked = extras.is_some_and(|extras| extras.history_asked);
    let reading = extras.is_some_and(|extras| extras.reading_history);
    match extras.and_then(|extras| extras.history.as_deref()) {
        None if !asked => errand = Some(Errand::History),
        None => {
            if reading {
                crate::ui::spinner(ui);
            }
        }
        Some([]) => hint(ui, &tr!("history-none")),
        Some(tasks) => tiles(ui, tasks, extras),
    }
    if asked
        && !reading
        && quiet_button(ui, icon::RESET, &tr!("history-read-again"), false, true).clicked()
    {
        errand = Some(Errand::History);
    }
    errand
}

fn tiles(ui: &mut egui::Ui, tasks: &[Task], extras: Option<&Extras>) {
    let width = tile_w(ui.available_width());
    let now_s = crate::updates::now_s();
    ui.spacing_mut().item_spacing = egui::vec2(TILE_GAP, TILE_GAP);
    for row in tasks.chunks(COLUMNS) {
        ui.horizontal_top(|ui| {
            for task in row {
                let picture = extras
                    .and_then(|extras| extras.pictures.get(&task.task_id))
                    .and_then(|picture| picture.texture());
                tile(ui, task, picture, width, now_s);
            }
        });
    }
}

/// One remembered print: its picture beside its file and how and when it ended, in the
/// colour of how it ended.
fn tile(
    ui: &mut egui::Ui,
    task: &Task,
    picture: Option<&egui::TextureHandle>,
    width: f32,
    now_s: u64,
) {
    let colors = theme::colors();
    let frame = egui::Frame::new()
        .fill(colors.panel)
        .stroke(egui::Stroke::new(1.0, colors.hairline))
        .corner_radius(theme::R_PANEL)
        .inner_margin(TILE_PAD);
    let inner_w = (width - 2.0 * f32::from(TILE_PAD)).max(0.0);
    frame.show(ui, |ui| {
        ui.set_width(inner_w);
        ui.set_height(TILE_H - 2.0 * f32::from(TILE_PAD));
        ui.horizontal_centered(|ui| {
            let size = egui::vec2(PICTURE, PICTURE);
            match picture {
                Some(texture) => picture_of(ui, texture, size),
                None => {
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, theme::R_CONTROL, colors.sunken);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icon::MODEL,
                        theme::icon(18.0),
                        colors.text_low,
                    );
                }
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let name = ui.add(
                    egui::Label::new(
                        RichText::new(task.file_name())
                            .font(theme::label())
                            .color(colors.text_high),
                    )
                    .truncate(),
                );
                name.on_hover_text(task.file_name());
                let tint = match task.ending() {
                    Ending::Finished => colors.ok,
                    Ending::Stopped => colors.warn,
                    Ending::Failed => colors.danger,
                    Ending::Other => colors.text_low,
                };
                let ending = egui::Label::new(
                    RichText::new(ended(task, now_s))
                        .font(theme::small())
                        .color(tint),
                )
                .truncate();
                ui.add(ending).on_hover_text(ended(task, now_s));
            });
        });
    });
}

/// How a remembered print ended, and when, as a person says it.
fn ended(task: &Task, now_s: u64) -> String {
    let when = ago(Some(task.end_s.max(task.begin_s)), now_s);
    let total = task
        .slice_information
        .as_ref()
        .map(|slice| slice.total_layer_numbers)
        .filter(|total| *total > 0);
    let at = match total {
        Some(total) => tr!(
            "history-at-layer-of",
            layer = task.already_print_layer,
            total = total
        ),
        None => tr!("history-at-layer", layer = task.already_print_layer),
    };
    match task.ending() {
        Ending::Finished => tr!("history-finished", when = when),
        Ending::Stopped => tr!("history-stopped", at = at, when = when),
        Ending::Failed => tr!("history-failed", at = at, when = when),
        Ending::Other => when,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use printer_link::Task;

    fn a_task(status: u32) -> Task {
        Task {
            task_name: "/media/sda1/print-a.goo".to_owned(),
            begin_s: 1_000,
            end_s: 4_600,
            task_status: status,
            already_print_layer: 240,
            slice_information: Some(net_sdcp::SliceInformation {
                total_layer_numbers: 1710,
                ..net_sdcp::SliceInformation::default()
            }),
            ..Task::default()
        }
    }

    #[test]
    fn three_tiles_share_a_row_and_a_fourth_starts_the_next() {
        assert!((tile_w(620.0) * 3.0 + TILE_GAP * 2.0 - 620.0).abs() < 1e-3);
        assert_eq!(tile_w(0.0), 0.0, "a row with no room draws nothing");
        assert_eq!(height(3), height(1));
        assert!(height(4) > height(3), "a fourth print starts a second row");
    }

    #[test]
    fn a_remembered_print_says_how_and_when_it_ended() {
        let now_s = 4_600 + 2 * 86_400;
        assert_eq!(ended(&a_task(1), now_s), "Finished 2 days ago");
        assert_eq!(
            ended(&a_task(3), now_s),
            "Stopped at layer 240 of 1710, 2 days ago"
        );
        assert_eq!(
            ended(&a_task(2), now_s),
            "Failed at layer 240 of 1710, 2 days ago"
        );
    }
}
