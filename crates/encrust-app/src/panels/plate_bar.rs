use egui::{Align, Layout, Rect, RichText, Sense, vec2};

use crate::scene::Scene;
use crate::state::Doc;
use crate::ui::{icon, icon_button, theme};

/// Points of quiet either side of a tab's label.
const TAB_PAD: f32 = 11.0;

/// How wide the tab grows while its name is being typed, so a longer one still fits.
const TAB_W_EDITING: f32 = 120.0;

/// One strip under the title holding every plate in the project, and what the tools on
/// the plate in front are aimed at.
///
/// The tabs have a strip of their own rather than a corner of the plate panel, because
/// that panel folds away and a project's plates must not fold away with it.
pub fn ui(ui: &mut egui::Ui, doc: &mut Doc) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let (models, picked) = tally(&doc.scene);
        if let Some(picked) = picked {
            ui.label(
                RichText::new(picked)
                    .font(theme::figures(11.0))
                    .color(theme::colors().picked),
            );
        }
        ui.label(
            RichText::new(models)
                .font(theme::figures(11.0))
                .color(theme::colors().text_low),
        );
        ui.add_space(theme::ITEM_GAP);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            tabs(ui, &mut doc.scene);
        });
    });
}

/// One tab per plate, with the way to add another and to take this one away. A project
/// of one plate still shows its tab: the row is where a second one is reached from.
fn tabs(ui: &mut egui::Ui, scene: &mut Scene) {
    let names: Vec<String> = scene.plates().to_vec();
    let active = scene.active_plate();
    let mut show = None;
    let mut removed = None;
    let mut rename = None;
    let editing: Option<u32> = ui.memory(|memory| memory.data.get_temp(editing_id()));

    // The tabs scroll rather than push the tally off the strip: a project may hold more
    // plates than fit across it.
    egui::ScrollArea::horizontal()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (index, name) in names.iter().enumerate() {
                    let plate = index as u32;
                    match tab(ui, name, plate, plate == active, editing) {
                        Some(Tab::Show) => show = Some(plate),
                        Some(Tab::Rename(name)) => rename = Some((plate, name)),
                        None => {}
                    }
                }
                ui.add_space(theme::ITEM_GAP);
                if icon_button(ui, icon::ADD, "New plate").clicked() {
                    scene.add_plate();
                }
                ui.add_enabled_ui(names.len() > 1, |ui| {
                    if icon_button(ui, icon::REMOVE, "Remove this plate and what is on it")
                        .clicked()
                    {
                        removed = Some(active);
                    }
                });
            });
        });

    if let Some(plate) = show {
        scene.show_plate(plate);
    }
    if let Some((plate, name)) = rename {
        scene.rename_plate(plate, name);
    }
    if let Some(plate) = removed {
        scene.remove_plate(plate);
    }
}

/// Which tab the strip is being typed into, and the text typed so far. Both live in
/// egui's memory rather than in the scene: a half-typed name is not part of the project.
fn editing_id() -> egui::Id {
    egui::Id::new("plate-rename")
}

fn field_id(plate: u32) -> egui::Id {
    egui::Id::new(("plate-name", plate))
}

/// What a press on a tab asked for.
enum Tab {
    Show,
    Rename(String),
}

/// One plate: what it is called, underlined while it is the plate in front. A double
/// click on the name opens it for typing; the strip carries no count, the plate panel
/// lists what stands there.
fn tab(
    ui: &mut egui::Ui,
    name: &str,
    plate: u32,
    active: bool,
    editing: Option<u32>,
) -> Option<Tab> {
    let colors = theme::colors();
    if editing == Some(plate) {
        return rename_field(ui, name, plate);
    }

    let label = ui
        .painter()
        .layout_no_wrap(name.to_owned(), theme::label(), colors.text_mid);
    let width = label.size().x + TAB_PAD * 2.0;

    let (rect, response) =
        ui.allocate_exact_size(vec2(width, ui.available_height()), Sense::click());
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, 0.0, colors.panel);
        let underline = Rect::from_min_size(
            egui::pos2(rect.left(), rect.bottom() - 2.0),
            vec2(rect.width(), 2.0),
        );
        painter.rect_filled(underline, 0.0, colors.accent);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, colors.raised);
    }

    let foreground = if active || response.hovered() {
        colors.text_high
    } else {
        colors.text_mid
    };
    painter.galley(
        egui::pos2(
            rect.left() + TAB_PAD,
            rect.center().y - label.size().y / 2.0,
        ),
        label,
        foreground,
    );

    if response.double_clicked() {
        ui.memory_mut(|memory| {
            memory.data.insert_temp(editing_id(), plate);
            memory.data.insert_temp(field_id(plate), name.to_owned());
            memory.request_focus(field_id(plate));
        });
        return None;
    }
    response.clicked().then_some(Tab::Show)
}

/// The tab while its name is being typed. The buffer lives in memory so a keystroke does
/// not rename the plate; the name is taken on Enter or on losing focus, and an empty one
/// is refused rather than cancelled, because a retype is the only undo it needs.
fn rename_field(ui: &mut egui::Ui, name: &str, plate: u32) -> Option<Tab> {
    let id = field_id(plate);
    let mut text = ui.memory(|memory| {
        memory
            .data
            .get_temp::<String>(id)
            .unwrap_or_else(|| name.to_owned())
    });
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .font(theme::label())
            .desired_width(TAB_W_EDITING),
    );
    ui.memory_mut(|memory| memory.data.insert_temp(id, text.clone()));

    if !response.lost_focus() {
        return None;
    }
    ui.memory_mut(|memory| {
        memory.data.remove::<u32>(editing_id());
        memory.data.remove::<String>(id);
    });
    let trimmed = text.trim();
    (!trimmed.is_empty() && trimmed != name).then(|| Tab::Rename(trimmed.to_owned()))
}

/// What the plate holds, and how much of it the tools are aimed at. Two parts, because
/// the second is drawn in the selection colour and the first is not.
fn tally(scene: &Scene) -> (String, Option<String>) {
    let count = scene.here().count();
    let models = if count == 1 {
        "1 model".to_owned()
    } else {
        format!("{count} models")
    };
    match scene.selection().len() {
        0 => (models, None),
        picked => (format!("{models},"), Some(format!("{picked} selected"))),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_geometry::{Mesh, Orientation, Transform, Vec3};

    use super::*;
    use crate::scene::{ImportSummary, Imported};

    fn cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ];
        Mesh::new(vertices, vec![[0, 1, 2], [0, 1, 3], [1, 2, 3], [2, 0, 3]])
    }

    fn scene_of(count: usize) -> Scene {
        let mut scene = Scene::default();
        for index in 0..count {
            scene.insert(Imported::new(
                format!("part{index}.stl"),
                Arc::new(cube()),
                Transform::default(),
                ImportSummary {
                    vertices_merged: 0,
                    faces_removed: 0,
                    orientation: Orientation {
                        flipped_faces: 0,
                        inverted_shells: 0,
                        orientable: true,
                    },
                    diagnostics: core_geometry::diagnose(&cube()),
                },
            ));
        }
        scene
    }

    #[test]
    fn the_tally_reads_as_a_sentence() {
        assert_eq!(tally(&Scene::default()), ("0 models".to_owned(), None));

        let mut one = scene_of(1);
        one.clear_selection();
        assert_eq!(tally(&one), ("1 model".to_owned(), None));
    }

    #[test]
    fn the_tally_names_what_the_tools_are_aimed_at() {
        let mut scene = scene_of(4);
        scene.select_here();
        assert_eq!(
            tally(&scene),
            ("4 models,".to_owned(), Some("4 selected".to_owned()))
        );

        scene.clear_selection();
        assert_eq!(tally(&scene), ("4 models".to_owned(), None));
    }
}
