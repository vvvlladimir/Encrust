use egui::{Align2, Rect, Sense, vec2};

use core_geometry::Vec3;

use crate::panels::Window;
use crate::profiles;
use crate::scene::{Axis, ImportSummary, ObjectId, Scene, SceneObject};
use crate::shortcuts::{self, Action};
use crate::state::Doc;
use crate::ui::{
    count_row, hairline, heading, hint, icon, icon_button, meta, number_row, picker,
    secondary_button, theme,
};

/// The print block at the foot of the panel: a heading over two pickers.
const PRINT_H: f32 = 128.0;

/// The row of actions at its foot: one line of icon buttons, inset like the inspector's
/// own margin, plus the line above it.
const ACTIONS_H: f32 = theme::ICON_SIZE + 25.0;

/// More copies than this in one go is a slip of the mouse, not an intention.
const ARRAY_MAX: u32 = 20;

/// How wide the import report is allowed to get before it wraps.
const POPUP_W: f32 = 230.0;

/// The plate down the left of the stage: what stands on it, and what it is printed on.
///
/// A panel rather than a card over the viewport, so the list may be as long as the plate
/// is full and a press on it can never orbit the camera; see `docs/decisions/0102`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    egui::Panel::top("plate-print")
        .exact_size(PRINT_H)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(theme::colors().panel))
        .show(ui, |ui| print(ui, window));

    egui::Panel::bottom("plate-actions")
        .exact_size(ACTIONS_H)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(theme::colors().panel))
        .show(ui, |ui| actions(ui, window));

    ui.spacing_mut().item_spacing.y = 0.0;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        egui::Frame::new()
            .inner_margin(theme::CARD_MARGIN)
            .show(ui, |ui| {
                heading(ui, "Plate contents", None);
            });
    });
    hairline(ui);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| rows(ui, &mut window.doc.scene));
}

/// What can be done to what stands on the plate. These belong to the contents rather
/// than to a tool, so they sit under the list and no tool panel repeats them.
fn actions(ui: &mut egui::Ui, window: &mut Window) {
    hairline(ui);
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            let picked: Vec<ObjectId> = window.doc.scene.selection().to_vec();
            let aimed = !picked.is_empty();
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.columns(5, |columns| {
                cell(&mut columns[0], aimed, |ui| duplicate(ui, window, &picked));
                cell(&mut columns[1], aimed, |ui| mirror(ui, window.doc, &picked));
                cell(&mut columns[2], aimed, |ui| array(ui, window, &picked));
                cell(&mut columns[3], true, |ui| {
                    if icon_button(ui, icon::ARRANGE, "Arrange the plate").clicked() {
                        window.machine.status = crate::arrange::arrange_plate(
                            &mut window.doc.scene,
                            &window.doc.plate,
                            window.tools.array.gap_mm,
                        );
                    }
                });
                cell(&mut columns[4], aimed, |ui| {
                    let tooltip = shortcuts::tooltip(Action::Remove);
                    if icon_button(ui, icon::REMOVE, &tooltip).clicked() {
                        window.doc.scene.remove_selected();
                    }
                });
            });
        });
}

/// One action in its share of the row, centred in it.
fn cell(ui: &mut egui::Ui, enabled: bool, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_enabled_ui(enabled, |ui| {
        ui.vertical_centered(add);
    });
}

/// Every copy and every flip works on the whole selection, so a plate of four does not
/// need the same button pressed four times.
fn duplicate(ui: &mut egui::Ui, window: &mut Window, picked: &[ObjectId]) {
    let tooltip = if picked.len() > 1 {
        format!("Duplicate {} models", picked.len())
    } else {
        shortcuts::tooltip(Action::Duplicate)
    };
    if icon_button(ui, icon::DUPLICATE, &tooltip).clicked() {
        duplicate_selection(window);
    }
}

/// A copy of everything picked, laid down beside what it came from and picked in its turn.
pub fn duplicate_selection(window: &mut Window) {
    let picked: Vec<ObjectId> = window.doc.scene.selection().to_vec();
    let copies: Vec<ObjectId> = picked
        .iter()
        .filter_map(|id| {
            let offset = beside(&window.doc.scene, *id, window.tools.array.gap_mm);
            window.doc.scene.duplicate(*id, offset)
        })
        .collect();
    window.doc.scene.select_many(&copies);
}

/// Three axes on one button: a flip is rare enough not to be worth three of them.
fn mirror(ui: &mut egui::Ui, doc: &mut Doc, picked: &[ObjectId]) {
    let response = icon_button(ui, icon::MIRROR, "Mirror");
    egui::Popup::menu(&response).show(|ui| {
        for axis in Axis::ALL {
            if ui.button(axis.label()).clicked() {
                for id in picked {
                    doc.scene.mirror(*id, axis);
                }
            }
        }
    });
}

/// The grid and the gap it leaves, behind the button that lays it out.
///
/// The popup stays open on a click inside it, unlike a menu of actions: a click on one of
/// its fields opens that field for typing, which closing would take away.
fn array(ui: &mut egui::Ui, window: &mut Window, picked: &[ObjectId]) {
    let response = icon_button(ui, icon::ARRAY, "Array");
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_width(POPUP_W);
            let array = &mut window.tools.array;
            count_row(ui, "Columns", &mut array.columns, "", 0.05, 1..=ARRAY_MAX);
            count_row(ui, "Rows", &mut array.rows, "", 0.05, 1..=ARRAY_MAX);
            number_row(ui, "Gap", &mut array.gap_mm, "mm", 0.1, 0.0..=50.0, 1);
            ui.add_space(4.0);
            if secondary_button(ui, icon::ARRAY, "Lay out").clicked()
                && let Some(id) = picked.first()
            {
                window.doc.scene.array(
                    *id,
                    window.tools.array.columns as usize,
                    window.tools.array.rows as usize,
                    window.tools.array.gap_mm,
                );
            }
        });
}

/// Where a single copy goes: one footprint to the right, with the same gap the array
/// leaves between its cells.
fn beside(scene: &Scene, id: ObjectId, gap_mm: f32) -> Vec3 {
    let width = scene
        .get(id)
        .and_then(SceneObject::world_bounds)
        .map_or(0.0, |bounds| bounds.maxs.x - bounds.mins.x);
    Vec3::new(width + gap_mm, 0.0, 0.0)
}

/// The machine and the resin belong to the plate rather than to whichever tool is open,
/// so they stand over its contents and leave the inspector to the tool.
fn print(ui: &mut egui::Ui, window: &mut Window) {
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            heading(ui, "Print", None);
            let response = picker(ui, icon::PRINTER, window.doc.plate.display_name());
            egui::Popup::menu(&response).show(|ui| profiles::printer_menu(ui, window));

            ui.add_space(theme::ITEM_GAP);
            let response = picker(ui, icon::RESIN, &window.machine.slicing.material.name);
            egui::Popup::menu(&response).show(|ui| profiles::resin_menu(ui, window.machine));

            if window.machine.slicing.printer_id.is_some()
                && !window.machine.slicing.resin_is_tuned()
            {
                meta(ui, &["not measured on this printer".to_owned()]);
            }
        });
    hairline(ui);
}

fn rows(ui: &mut egui::Ui, scene: &mut Scene) {
    // What a click means, as every list on the desktop reads it: plain picks one, cmd
    // adds or drops one, shift takes everything between the last pick and this.
    let (spanning, adding) = ui.input(|input| (input.modifiers.shift, input.modifiers.command));
    let mut select = None;
    let mut toggle = None;

    for object in scene.here() {
        match row(ui, object, scene.is_selected(object.id)) {
            Some(Clicked::Select) => select = Some(object.id),
            Some(Clicked::Visibility) => toggle = Some(object.id),
            None => {}
        }
    }

    if let Some(id) = select {
        match (spanning, adding) {
            (true, _) => scene.select_span_to(id),
            (false, true) => scene.toggle_selected(id),
            (false, false) => scene.select(Some(id)),
        }
    }
    if let Some(id) = toggle
        && let Some(object) = scene.get_mut(id)
    {
        object.visible = !object.visible;
    }
}

enum Clicked {
    Select,
    Visibility,
}

/// One model: whether it is drawn and what it is called, with what import found about it
/// at the end of the row.
fn row(ui: &mut egui::Ui, object: &SceneObject, selected: bool) -> Option<Clicked> {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::click());
    row_background(ui.painter(), rect, selected, response.hovered());

    let (eye, eye_response) = visibility_eye(ui, rect, object);
    let end = rect.right() - 10.0;
    let name_right = match trouble(&object.summary) {
        Some((color, lines)) => import_warning(ui, rect, object, end, color, &lines),
        None => end,
    };

    let colors = theme::colors();
    let painter = ui.painter();
    let name_color = if object.visible {
        colors.text_high
    } else {
        colors.text_low
    };
    let name = painter.layout(
        object.name.clone(),
        theme::label(),
        name_color,
        (name_right - 8.0 - eye.right()).max(0.0),
    );
    painter.galley(
        egui::pos2(eye.right() + 4.0, rect.center().y - name.size().y / 2.0),
        name,
        name_color,
    );

    if eye_response.clicked() {
        return Some(Clicked::Visibility);
    }
    if response.clicked() {
        return Some(Clicked::Select);
    }
    None
}

fn row_background(painter: &egui::Painter, rect: Rect, selected: bool, hovered: bool) {
    let colors = theme::colors();
    if selected {
        painter.rect_filled(rect, 0.0, colors.picked_wash);
        let marker = Rect::from_min_size(
            egui::pos2(rect.left(), rect.top() + 4.0),
            vec2(2.0, rect.height() - 8.0),
        );
        painter.rect_filled(marker, 0.0, colors.picked);
    } else if hovered {
        painter.rect_filled(rect, 0.0, colors.raised);
    }
}

/// The eye at the head of the row that shows and hides the model.
fn visibility_eye(ui: &mut egui::Ui, rect: Rect, object: &SceneObject) -> (Rect, egui::Response) {
    let colors = theme::colors();
    let eye = Rect::from_center_size(
        egui::pos2(rect.left() + 19.0, rect.center().y),
        vec2(18.0, rect.height()),
    );
    let response = ui.interact(
        eye.shrink2(vec2(0.0, 4.0)),
        ui.id().with(object.id),
        Sense::click(),
    );
    ui.painter().text(
        eye.center(),
        Align2::CENTER_CENTER,
        if object.visible {
            icon::VISIBLE
        } else {
            icon::HIDDEN
        },
        theme::icon(15.0),
        if response.hovered() {
            colors.text_high
        } else {
            colors.text_low
        },
    );
    (eye, response)
}

/// The warning at the end of the row, with what import found behind a click. Returns
/// where it starts.
fn import_warning(
    ui: &mut egui::Ui,
    rect: Rect,
    object: &SceneObject,
    right: f32,
    color: egui::Color32,
    lines: &[String],
) -> f32 {
    let spot = Rect::from_center_size(egui::pos2(right - 17.0, rect.center().y), vec2(18.0, 18.0));
    let warn = ui.interact(spot, ui.id().with((object.id, "import")), Sense::click());
    ui.painter().text(
        spot.center(),
        Align2::CENTER_CENTER,
        icon::WARNING,
        theme::icon(14.0),
        if warn.hovered() {
            theme::colors().text_high
        } else {
            color
        },
    );
    egui::Popup::menu(&warn).show(|ui| {
        ui.set_max_width(POPUP_W);
        for line in lines {
            hint(ui, line);
        }
    });
    spot.left()
}

/// What import had to change and what it could not fix, and how loudly to say so.
/// `None` when the mesh came in clean, which is most of them.
fn trouble(summary: &ImportSummary) -> Option<(egui::Color32, Vec<String>)> {
    let colors = theme::colors();
    let mut color = colors.text_low;
    let mut lines = Vec::new();

    for (label, count) in [
        ("vertices merged", summary.vertices_merged),
        ("faces removed", summary.faces_removed),
        ("faces flipped", summary.orientation.flipped_faces),
        ("shells inverted", summary.orientation.inverted_shells),
    ] {
        if count > 0 {
            lines.push(format!("Repaired {count} {label}"));
        }
    }
    for (label, count) in [
        ("open edges", summary.diagnostics.boundary_edges),
        ("branching edges", summary.diagnostics.non_manifold_edges),
        ("degenerate faces", summary.diagnostics.degenerate_faces),
        ("duplicate faces", summary.diagnostics.duplicate_faces),
    ] {
        if count > 0 {
            lines.push(format!("{count} {label}"));
            color = colors.warn;
        }
    }
    if !summary.orientation.orientable {
        lines.push("No consistent orientation: slicing will be unreliable".to_owned());
        color = colors.danger;
    }

    (!lines.is_empty()).then_some((color, lines))
}

#[cfg(test)]
mod tests {
    use core_geometry::{Mesh, MeshDiagnostics, Orientation, Vec3};

    use super::*;

    fn summary(orientable: bool, boundary_edges: usize) -> ImportSummary {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable,
            },
            diagnostics: MeshDiagnostics {
                boundary_edges,
                ..core_geometry::diagnose(&mesh)
            },
        }
    }

    #[test]
    fn a_clean_import_says_nothing() {
        assert!(trouble(&summary(true, 0)).is_none());
    }

    #[test]
    fn a_mesh_that_cannot_be_oriented_is_the_loudest_thing_in_the_report() {
        let (color, lines) = trouble(&summary(false, 4)).expect("a torn mesh has something to say");
        assert_eq!(color, theme::colors().danger);
        assert_eq!(lines.len(), 2, "the open edges and the orientation");
    }
}
