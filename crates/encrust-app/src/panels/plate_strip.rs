use egui::{Align2, Sense, Stroke, vec2};

use crate::scene::{ObjectId, Scene};
use crate::shortcuts::{self, Action};
use crate::ui::{icon, icon_button, theme};

/// How big a plate's square is.
const PLATE_SIZE: f32 = 32.0;

/// The row at the head of the plate panel: one square per plate, numbered, and the way to
/// add another, scrolling past `max_w` points. A project of one plate still
/// shows it: the strip is where a second one is reached from.
pub fn ui(ui: &mut egui::Ui, scene: &mut Scene, max_w: f32) {
    let names: Vec<String> = scene.plates().to_vec();
    let active = scene.active_plate();
    let picked = !scene.selection().is_empty();
    let mut asked = None;

    egui::ScrollArea::horizontal()
        .max_width(max_w)
        .auto_shrink([true, true])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (index, name) in names.iter().enumerate() {
                    let plate = index as u32;
                    let place = Place {
                        plate,
                        active: plate == active,
                        many: names.len() > 1,
                        picked,
                    };
                    let count = scene.on_plate(plate).count();
                    if let Some(tab) = square(ui, name, count, place) {
                        asked = Some((plate, tab));
                    }
                }
                if icon_button(ui, icon::ADD, &shortcuts::tooltip(Action::NewPlate)).clicked() {
                    asked = Some((active, Asked::Add));
                }
            });
        });

    if let Some((plate, asked)) = asked {
        apply(scene, plate, asked);
    }
}

fn apply(scene: &mut Scene, plate: u32, asked: Asked) {
    match asked {
        Asked::Show => scene.show_plate(plate),
        Asked::Add => {
            scene.add_plate();
        }
        Asked::Duplicate => {
            scene.duplicate_plate(plate);
        }
        Asked::Remove => scene.remove_plate(plate),
        // What is picked stands on the plate in front, so it is sent from there.
        Asked::Gather => {
            let picked: Vec<ObjectId> = scene.selection().to_vec();
            for id in picked {
                scene.move_to_plate(id, plate);
            }
            scene.show_plate(plate);
        }
    }
}

/// Where a plate stands among the others, which decides what its menu offers.
#[derive(Clone, Copy)]
struct Place {
    plate: u32,
    active: bool,
    /// Whether there is a plate besides this one, so this one may go.
    many: bool,
    /// Whether anything is picked on the plate in front, to be sent here.
    picked: bool,
}

/// What a press on a plate, or its menu, asked for.
enum Asked {
    Show,
    Add,
    Duplicate,
    Remove,
    Gather,
}

/// One plate: its number, lit while it is the plate in front, with its name and what
/// stands on it on hover, and the rest on a right click.
fn square(ui: &mut egui::Ui, name: &str, count: usize, place: Place) -> Option<Asked> {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(vec2(PLATE_SIZE, PLATE_SIZE), Sense::click());
    let (fill, ink) = match (place.active, response.hovered()) {
        (true, _) => (colors.raised, colors.accent_soft),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (egui::Color32::TRANSPARENT, colors.text_mid),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    if place.active {
        painter.rect_stroke(
            rect,
            theme::R_CONTROL,
            Stroke::new(1.0, colors.line),
            egui::StrokeKind::Inside,
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        (place.plate + 1).to_string(),
        theme::figures(12.5),
        ink,
    );

    let models = if count == 1 {
        "1 model".to_owned()
    } else {
        format!("{count} models")
    };
    let response = response.on_hover_text(format!("{name}, {models}"));
    let asked = menu(&response, place);
    asked.or_else(|| response.clicked().then_some(Asked::Show))
}

/// What a right click on a plate offers.
fn menu(response: &egui::Response, place: Place) -> Option<Asked> {
    let mut asked = None;
    response.context_menu(|ui| {
        if ui.button("Duplicate").clicked() {
            asked = Some(Asked::Duplicate);
            ui.close();
        }
        let gather = place.picked && !place.active;
        if ui
            .add_enabled(gather, egui::Button::new("Move the selection here"))
            .clicked()
        {
            asked = Some(Asked::Gather);
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(place.many, egui::Button::new("Remove with what is on it"))
            .clicked()
        {
            asked = Some(Asked::Remove);
            ui.close();
        }
    });
    asked
}
