use egui::{Align2, Rect, Sense, vec2};

use core_geometry::{Vec2, Vec3};

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

/// What the end of the row of a model that will not slice as it stands reads.
const BROKEN: &str = "broken";

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
        .show(ui, |ui| rows(ui, window));
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
                    Vec2::new(window.doc.plate.x_mm, window.doc.plate.y_mm),
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
            let response = picker(ui, icon::RESIN, window.machine.slicing.resin_name());
            egui::Popup::menu(&response).show(|ui| profiles::resin_menu(ui, window.machine));

            if window.machine.slicing.printer_id.is_some()
                && window.machine.slicing.has_resin()
                && !window.machine.slicing.resin_is_tuned()
            {
                meta(ui, &["not measured on this printer".to_owned()]);
            }
        });
    hairline(ui);
}

fn rows(ui: &mut egui::Ui, window: &mut Window) {
    // What a click means, as every list on the desktop reads it: plain picks one, cmd
    // adds or drops one, shift takes everything between the last pick and this.
    let (spanning, adding) = ui.input(|input| (input.modifiers.shift, input.modifiers.command));
    let plates: Vec<String> = window.doc.scene.plates().to_vec();
    let active = window.doc.scene.active_plate();
    let mut select = None;
    let mut clicked = None;

    let scene = &mut window.doc.scene;
    for object in scene.here() {
        let selected = scene.is_selected(object.id);
        match row(ui, object, selected, &plates, active) {
            Some(Clicked::Select) => select = Some(object.id),
            Some(what) => clicked = Some((object.id, what)),
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
    match clicked {
        Some((id, Clicked::Visibility)) => {
            if let Some(object) = scene.get_mut(id) {
                object.visible = !object.visible;
            }
        }
        // What a row's own menu does is done to everything picked, like every button under
        // the list: a plate of four is not moved one row at a time.
        Some((id, Clicked::MoveTo(plate))) => {
            for id in with_the_selection(scene, id) {
                scene.move_to_plate(id, plate);
            }
        }
        Some((id, Clicked::Repair)) => {
            window
                .doc
                .repairs
                .start(scene, id, &mut window.machine.status);
        }
        Some((_, Clicked::Select)) | None => {}
    }
}

/// The models one row's menu works on: everything picked when the row is one of them, and
/// the row alone when it is not.
fn with_the_selection(scene: &Scene, id: ObjectId) -> Vec<ObjectId> {
    if scene.is_selected(id) {
        scene.selection().to_vec()
    } else {
        vec![id]
    }
}

enum Clicked {
    Select,
    Visibility,
    MoveTo(u32),
    Repair,
}

/// One model: whether it is drawn and what it is called, with what import found about it
/// at the end of the row.
fn row(
    ui: &mut egui::Ui,
    object: &SceneObject,
    selected: bool,
    plates: &[String],
    active: u32,
) -> Option<Clicked> {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::click());
    row_background(ui.painter(), rect, selected, response.hovered());
    let asked = row_menu(&response, object, plates, active);

    let (eye, eye_response) = visibility_eye(ui, rect, object);
    let end = rect.right() - 10.0;
    let name_right = match trouble(&object.summary) {
        Some((mark, lines)) => import_mark(ui, rect, object, end, mark, &lines),
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

    if let Some(asked) = asked {
        return Some(asked);
    }
    if eye_response.clicked() {
        return Some(Clicked::Visibility);
    }
    if response.clicked() {
        return Some(Clicked::Select);
    }
    None
}

/// The row's own menu, where a model is where the hand already is: sending it to another
/// plate, and repairing it when it came in broken.
fn row_menu(
    response: &egui::Response,
    object: &SceneObject,
    plates: &[String],
    active: u32,
) -> Option<Clicked> {
    let mut clicked = None;
    response.context_menu(|ui| {
        if !object.summary.is_sound() && ui.button("Repair").clicked() {
            clicked = Some(Clicked::Repair);
            ui.close();
        }
        ui.add_enabled_ui(plates.len() > 1, |ui| {
            ui.menu_button("Move to plate", |ui| {
                for (index, name) in plates.iter().enumerate() {
                    let plate = index as u32;
                    if ui
                        .add_enabled(plate != active, egui::Button::new(name))
                        .clicked()
                    {
                        clicked = Some(Clicked::MoveTo(plate));
                        ui.close();
                    }
                }
            });
        });
    });
    clicked
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

/// What the end of a row says about a model: a model that will not slice as it stands is
/// named broken in words, a model repair only had to tidy gets the quiet sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Repaired,
    Broken,
}

/// The mark at the end of the row, with what import found behind a click. Returns where
/// it starts.
fn import_mark(
    ui: &mut egui::Ui,
    rect: Rect,
    object: &SceneObject,
    right: f32,
    mark: Mark,
    lines: &[String],
) -> f32 {
    let spot = match mark {
        Mark::Repaired => {
            Rect::from_center_size(egui::pos2(right - 17.0, rect.center().y), vec2(18.0, 18.0))
        }
        Mark::Broken => {
            let width = ui.painter().layout_no_wrap(
                BROKEN.to_owned(),
                theme::small(),
                theme::colors().danger,
            );
            let width = width.size().x + 8.0;
            Rect::from_center_size(
                egui::pos2(right - width / 2.0, rect.center().y),
                vec2(width, 18.0),
            )
        }
    };
    let response = ui.interact(spot, ui.id().with((object.id, "import")), Sense::click());
    paint_mark(ui.painter(), spot, mark, response.hovered());
    egui::Popup::menu(&response).show(|ui| {
        ui.set_max_width(POPUP_W);
        for line in lines {
            hint(ui, line);
        }
    });
    spot.left()
}

fn paint_mark(painter: &egui::Painter, spot: Rect, mark: Mark, hovered: bool) {
    let colors = theme::colors();
    let (text, font, color) = match mark {
        Mark::Repaired => (icon::WARNING, theme::icon(14.0), colors.text_low),
        Mark::Broken => (BROKEN, theme::small(), colors.danger),
    };
    let color = if hovered { colors.text_high } else { color };
    painter.text(spot.center(), Align2::CENTER_CENTER, text, font, color);
}

/// What is wrong with a model and what repair already did, and how loudly to say it.
/// `None` when the mesh came in clean, which is most of them.
fn trouble(summary: &ImportSummary) -> Option<(Mark, Vec<String>)> {
    let defects = summary.defects();
    let mark = if defects.is_empty() {
        Mark::Repaired
    } else {
        Mark::Broken
    };
    let lines = [defects, summary.repairs()].concat();
    (!lines.is_empty()).then_some((mark, lines))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_geometry::{Mesh, MeshDiagnostics, Orientation, Transform, Vec3};

    use super::*;
    use crate::scene::Imported;

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
                unbalanced_edges: boundary_edges,
                ..core_geometry::diagnose(&mesh)
            },
        }
    }

    /// A plate of `count` models, none of them selected.
    fn plate_of(count: usize) -> (Scene, Vec<ObjectId>) {
        let mesh = Arc::new(Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            vec![[0, 1, 2]],
        ));
        let mut scene = Scene::default();
        let ids = (0..count)
            .map(|at| {
                scene.insert(Imported::new(
                    format!("model-{at}.stl"),
                    Arc::clone(&mesh),
                    Transform::default(),
                    summary(true, 0),
                ))
            })
            .collect();
        scene.clear_selection();
        (scene, ids)
    }

    #[test]
    fn a_rows_menu_moves_everything_picked_when_that_row_is_one_of_them() {
        let (mut scene, ids) = plate_of(3);
        scene.select_many(&ids[..2]);

        assert_eq!(with_the_selection(&scene, ids[1]), ids[..2].to_vec());
    }

    #[test]
    fn a_rows_menu_moves_that_row_alone_when_it_is_not_picked() {
        let (mut scene, ids) = plate_of(3);
        scene.select_many(&ids[..2]);

        assert_eq!(with_the_selection(&scene, ids[2]), vec![ids[2]]);
    }

    #[test]
    fn a_clean_import_says_nothing() {
        assert!(trouble(&summary(true, 0)).is_none());
    }

    #[test]
    fn merging_the_vertices_of_an_stl_is_not_trouble() {
        let merged = ImportSummary {
            vertices_merged: 28,
            ..summary(true, 0)
        };
        assert!(
            trouble(&merged).is_none(),
            "every STL merges vertices, so a mark for it would stand on every row"
        );
    }

    #[test]
    fn a_mesh_that_cannot_be_oriented_is_named_broken() {
        let (mark, lines) = trouble(&summary(false, 4)).expect("a torn mesh has something to say");
        assert_eq!(mark, Mark::Broken);
        assert_eq!(lines.len(), 2, "the open edges and the orientation");
    }

    #[test]
    fn a_mesh_repair_only_tidied_gets_the_quiet_mark() {
        let tidied = ImportSummary {
            faces_removed: 2,
            ..summary(true, 0)
        };
        let (mark, lines) = trouble(&tidied).expect("a dropped face is worth a line");
        assert_eq!(mark, Mark::Repaired);
        assert_eq!(lines.len(), 1);
    }
}
