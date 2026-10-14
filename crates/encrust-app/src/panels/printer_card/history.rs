use egui::RichText;
use printer_link::{Ending, Task};

use super::picture_of;
use crate::network::{Errand, Extras};
use crate::ui::{ago, hint, icon, quiet_button, theme, tone};

/// How big a remembered print's picture is drawn, in points.
const THUMB: egui::Vec2 = egui::vec2(48.0, 36.0);

/// The prints the printer remembers, folded away until opened; opening it the first time
/// is what asks for them.
pub(super) fn fold(ui: &mut egui::Ui, key: &str, extras: Option<&Extras>) -> Option<Errand> {
    let mut errand = None;
    let title = RichText::new("Recent prints")
        .font(theme::small())
        .color(theme::colors().text_mid);
    egui::CollapsingHeader::new(title)
        .id_salt(("printer-history", key))
        .show(ui, |ui| {
            let asked = extras.is_some_and(|extras| extras.history_asked);
            let doing = extras.and_then(|extras| extras.doing);
            match extras.and_then(|extras| extras.history.as_deref()) {
                None if !asked => errand = Some(Errand::History),
                None => {
                    if doing.is_some() {
                        ui.spinner();
                    }
                }
                Some([]) => hint(ui, "It remembers no prints."),
                Some(tasks) => {
                    let now_s = crate::updates::now_s();
                    for task in tasks {
                        let picture = extras
                            .and_then(|extras| extras.pictures.get(&task.task_id))
                            .and_then(|picture| picture.texture());
                        row(ui, task, picture, now_s);
                    }
                }
            }
            if asked
                && doing.is_none()
                && quiet_button(ui, icon::RESET, "Read again", false, true).clicked()
            {
                errand = Some(Errand::History);
            }
        });
    errand
}

/// One remembered print: its picture, its file, and how and when it ended.
fn row(ui: &mut egui::Ui, task: &Task, picture: Option<&egui::TextureHandle>, now_s: u64) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        match picture {
            Some(texture) => picture_of(ui, texture, THUMB),
            None => {
                let (rect, _) = ui.allocate_exact_size(THUMB, egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, theme::R_CONTROL, colors.sunken);
            }
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add(
                egui::Label::new(
                    RichText::new(task.file_name())
                        .font(theme::label())
                        .color(colors.text_high),
                )
                .truncate(),
            );
            let tint = match task.ending() {
                Ending::Failed => colors.warn,
                Ending::Finished | Ending::Stopped | Ending::Other => colors.text_low,
            };
            tone(ui, &ended(task, now_s), tint);
        });
    });
}

/// How a remembered print ended, and when, as a person says it.
fn ended(task: &Task, now_s: u64) -> String {
    let when = ago(Some(task.end_time.max(task.begin_time)), now_s);
    let total = task
        .slice_information
        .as_ref()
        .map(|slice| slice.total_layer_numbers)
        .filter(|total| *total > 0);
    let at = match total {
        Some(total) => format!("at layer {} of {total}", task.already_print_layer),
        None => format!("at layer {}", task.already_print_layer),
    };
    match task.ending() {
        Ending::Finished => format!("Finished {when}"),
        Ending::Stopped => format!("Stopped {at}, {when}"),
        Ending::Failed => format!("Failed {at}, {when}"),
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
            begin_time: 1_000,
            end_time: 4_600,
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
