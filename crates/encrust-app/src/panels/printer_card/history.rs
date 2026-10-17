use egui::RichText;
use printer_link::{Ending, Task};

use super::picture_of;
use crate::i18n::tr;
use crate::network::{Errand, Extras};
use crate::ui::{ago, hint, icon, quiet_button, theme, tone};

/// The least width a tile takes, and the gap between two, in points.
const TILE_MIN_W: f32 = 130.0;
const TILE_GAP: f32 = 10.0;
/// A board's thumbnail is 400 by 300.
const PICTURE_ASPECT: f32 = 0.75;
/// What a tile holds under its picture: the file, and how it ended over up to two lines.
const CAPTION_H: f32 = 54.0;

/// How many tiles a row of `width` holds, and how wide each is drawn.
pub(super) fn columns(width: f32) -> (usize, f32) {
    let count = (((width + TILE_GAP) / (TILE_MIN_W + TILE_GAP)).floor() as usize).max(1);
    let tile_w = (width - TILE_GAP * (count - 1) as f32) / count as f32;
    (count, tile_w)
}

/// How tall the tiles for `prints` stand in a row of `width`.
pub(super) fn height(width: f32, prints: usize) -> f32 {
    let (count, tile_w) = columns(width);
    let rows = prints.div_ceil(count).max(1);
    rows as f32 * (tile_w * PICTURE_ASPECT + CAPTION_H + TILE_GAP)
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
                ui.spinner();
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
    let (count, tile_w) = columns(ui.available_width());
    let now_s = crate::updates::now_s();
    ui.spacing_mut().item_spacing = egui::vec2(TILE_GAP, TILE_GAP);
    for row in tasks.chunks(count) {
        ui.horizontal_top(|ui| {
            for task in row {
                let picture = extras
                    .and_then(|extras| extras.pictures.get(&task.task_id))
                    .and_then(|picture| picture.texture());
                tile(ui, task, picture, tile_w, now_s);
            }
        });
    }
}

/// One remembered print: its picture, its file under it, and how and when it ended.
fn tile(
    ui: &mut egui::Ui,
    task: &Task,
    picture: Option<&egui::TextureHandle>,
    width: f32,
    now_s: u64,
) {
    let colors = theme::colors();
    let size = egui::vec2(width, width * PICTURE_ASPECT);
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 2.0;
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
                    theme::icon(22.0),
                    colors.text_low,
                );
            }
        }
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
            Ending::Failed => colors.warn,
            Ending::Finished | Ending::Stopped | Ending::Other => colors.text_low,
        };
        tone(ui, &ended(task, now_s), tint);
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
    fn a_row_holds_as_many_tiles_as_fit_and_shares_what_is_left() {
        let (count, tile_w) = columns(584.0);
        assert_eq!(count, 4, "four of 130 with three gaps of 10 fit in 584");
        assert!((tile_w * 4.0 + TILE_GAP * 3.0 - 584.0).abs() < 1e-3);
        assert_eq!(columns(90.0).0, 1, "a narrow row still holds one");
        assert!(
            height(584.0, 5) > height(584.0, 4),
            "a fifth print starts a second row"
        );
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
