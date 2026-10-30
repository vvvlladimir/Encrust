use egui::{Align2, Rect, Sense, vec2};

use core_geometry::{Vec2, Vec3};

use crate::files::{self, Wanted};
use crate::i18n::tr;
use crate::panels::{Window, plate_strip};
use crate::project;
use crate::scene::{ImportSummary, ObjectId, Scene, SceneObject};
use crate::shortcuts::{self, Action};
use crate::ui::{
    body_and_foot, count_row, empty_state, focus_ring, hairline, heading, hint, icon,
    inline_button, number_row, secondary_button, small_icon_button, tag, theme, tone,
};

/// The heading over the list, as tall as a section's.
const HEADER_H: f32 = theme::SECTION_H;

/// More copies than this in one go is a slip of the mouse, not an intention.
const ARRAY_MAX: u32 = 20;

/// How wide the import report is allowed to get before it wraps.
const POPUP_W: f32 = 230.0;

/// The models card at the right of the stage: which plate, what stands on it, what can be
/// done to it. Scrolls past `max_h` points; see `docs/decisions/0221`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window, max_h: f32) {
    egui::Frame::new()
        .inner_margin(PLATES_MARGIN)
        .show(ui, |ui| {
            let room = ui.available_width();
            if plate_strip::ui(ui, &mut window.doc.scene, room) {
                window.machine.slicing.take_profile_settings();
            }
        });
    hairline(ui);
    header(ui, window);
    hairline(ui);
    body_and_foot(ui, max_h, window, rows, actions);
}

/// Points around the row of plates at the card's head, which stands 42 tall.
const PLATES_MARGIN: egui::Margin = egui::Margin::symmetric(12, 8);

/// The card's heading: *Models*, the open project, how many models the tools are aimed
/// at, and the way to add another model.
fn header(ui: &mut egui::Ui, window: &mut Window) {
    let picked = window.doc.scene.selection().len();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
    let inner = rect
        .shrink2(vec2(theme::PANEL_PAD, 0.0))
        .with_max_x(rect.right() - 6.0);
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            heading(ui, &tr!("models-title"), None);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tooltip = shortcuts::tooltip(Action::OpenModel);
                if small_icon_button(ui, icon::ADD, &tooltip).clicked() {
                    crate::import::open_dialog(window);
                }
                if picked > 0 {
                    tone(
                        ui,
                        &tr!("models-picked", count = picked),
                        theme::colors().accent_soft,
                    );
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    project_name(ui, &window.doc.project);
                });
            });
        },
    );
}

/// What project is open, if one is, cut short to the room the heading leaves it.
fn project_name(ui: &mut egui::Ui, opened: &project::Opened) {
    let Some(name) = opened.name() else {
        return;
    };
    let text = egui::RichText::new(tr!("models-project", name = name.to_owned()))
        .font(theme::label())
        .color(theme::colors().text_low);
    ui.add(egui::Label::new(text).truncate());
}

/// What can be done to what stands on the plate. These belong to the contents rather
/// than to a tool, so they sit under the list and no tool panel repeats them.
fn actions(ui: &mut egui::Ui, window: &mut Window) {
    hairline(ui);
    egui::Frame::new()
        .inner_margin(theme::TOOLBAR_PAD)
        .show(ui, |ui| {
            let picked: Vec<ObjectId> = window.doc.scene.selection().to_vec();
            let aimed = !picked.is_empty();
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.columns(4, |columns| {
                cell(&mut columns[0], aimed, |ui| duplicate(ui, window, &picked));
                cell(&mut columns[1], aimed, |ui| array(ui, window, &picked));
                cell(&mut columns[2], true, |ui| {
                    if small_icon_button(ui, icon::ARRANGE, &tr!("models-arrange")).clicked() {
                        window.machine.status = crate::arrange::arrange_plate(
                            &mut window.doc.scene,
                            &window.doc.plate,
                            window.tools.array.gap_mm,
                        );
                    }
                });
                cell(&mut columns[3], aimed, |ui| {
                    let tooltip = shortcuts::tooltip(Action::Remove);
                    if small_icon_button(ui, icon::REMOVE, &tooltip).clicked() {
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

/// Every copy works on the whole selection, so a plate of four does not
/// need the same button pressed four times.
fn duplicate(ui: &mut egui::Ui, window: &mut Window, picked: &[ObjectId]) {
    let tooltip = if picked.len() > 1 {
        tr!("models-duplicate", count = picked.len())
    } else {
        shortcuts::tooltip(Action::Duplicate)
    };
    if small_icon_button(ui, icon::DUPLICATE, &tooltip).clicked() {
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

/// The grid and the gap it leaves, behind the button that lays it out.
/// The popup stays open on a click inside, which a field needs to open for typing.
fn array(ui: &mut egui::Ui, window: &mut Window, picked: &[ObjectId]) {
    let response = small_icon_button(ui, icon::ARRAY, &tr!("models-array"));
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_width(POPUP_W);
            let array = &mut window.tools.array;
            let (columns, rows) = (tr!("models-columns"), tr!("models-rows"));
            count_row(ui, &columns, &mut array.columns, "", 0.05, 1..=ARRAY_MAX);
            count_row(ui, &rows, &mut array.rows, "", 0.05, 1..=ARRAY_MAX);
            let (gap, mm) = (tr!("measure-gap"), tr!("unit-mm"));
            number_row(ui, &gap, &mut array.gap_mm, &mm, 0.1, 0.0..=50.0, 1);
            ui.add_space(4.0);
            if secondary_button(ui, icon::ARRAY, &tr!("models-lay-out")).clicked()
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

/// No model on this plate: what is missing, and the way to put one there.
fn empty_plate(ui: &mut egui::Ui, window: &mut Window) {
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            empty_state(
                ui,
                icon::EMPTY_PLATE,
                &tr!("stage-empty"),
                &tr!("stage-empty-how"),
            );
            let open = ui.vertical_centered(|ui| {
                inline_button(ui, icon::OPEN, &tr!("stage-open"), false, true).clicked()
            });
            if open.inner
                && let Some(file) = files::pick(Wanted::ModelOrProject)
            {
                crate::app::open_by_what_it_is(window, file);
            }
            ui.add_space(theme::ITEM_GAP);
        });
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
    if scene.here().next().is_none() {
        empty_plate(ui, window);
        return;
    }
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
            let state = window.tools.repair.state.plain();
            window
                .doc
                .repairs
                .start(scene, id, state, &mut window.machine.status);
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
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), theme::MODEL_ROW_H),
        Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            &object.name,
        )
    });
    row_background(ui.painter(), rect, selected, response.hovered());
    focus_ring(ui, &response, 0.0);
    let asked = row_menu(&response, object, plates, active);

    let (eye, eye_response) = visibility_eye(ui, rect, object);
    let end = rect.right() - 8.0;
    let (name_right, repair) = match trouble(&object.summary) {
        Some((mark, lines)) => import_mark(ui, rect, object, end, mark, &lines),
        None => (end, false),
    };

    let colors = theme::colors();
    let painter = ui.painter();
    let name_color = if object.visible {
        colors.text_high
    } else {
        colors.text_low
    };
    // A row is one line tall, so a name longer than it is cut rather than wrapped over
    // the row below it; the whole of it is on the tooltip.
    let mut job =
        egui::text::LayoutJob::simple_singleline(object.name.clone(), theme::label(), name_color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(
        (name_right - 8.0 - eye.right() - 4.0).max(0.0),
    );
    let name = painter.layout_job(job);
    let elided = name.elided;
    painter.galley(
        egui::pos2(eye.right() + 4.0, rect.center().y - name.size().y / 2.0),
        name,
        name_color,
    );
    if elided {
        response.clone().on_hover_text(&object.name);
    }

    if let Some(asked) = asked {
        return Some(asked);
    }
    if repair {
        return Some(Clicked::Repair);
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
        if !object.summary.is_sound() && ui.button(tr!("notice-repair")).clicked() {
            clicked = Some(Clicked::Repair);
            ui.close();
        }
        ui.add_enabled_ui(plates.len() > 1, |ui| {
            ui.menu_button(tr!("menu-move-to-plate"), |ui| {
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
    response.widget_info(|| {
        let shown = tr!("models-show", name = object.name.clone());
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, object.visible, shown)
    });
    focus_ring(ui, &response, theme::R_CONTROL);
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

/// The mark at the end of the row, with what import found behind a click, and the repair on a
/// broken model. Returns where it starts, and whether repair was asked for.
fn import_mark(
    ui: &mut egui::Ui,
    rect: Rect,
    object: &SceneObject,
    right: f32,
    mark: Mark,
    lines: &[String],
) -> (f32, bool) {
    let colors = theme::colors();
    let id = ui.id().with((object.id, "import"));
    let response = match mark {
        Mark::Repaired => quiet_mark(ui, rect, id, right),
        Mark::Broken => tag(
            ui,
            egui::pos2(right, rect.center().y),
            id,
            &tr!("models-broken"),
            (colors.danger, colors.danger_wash),
        ),
    };
    let mut repair = false;
    egui::Popup::menu(&response).show(|ui| {
        ui.set_max_width(POPUP_W);
        for line in lines {
            hint(ui, line);
        }
        if mark == Mark::Broken {
            ui.add_space(4.0);
            repair = ui.button(tr!("notice-repair")).clicked();
        }
    });
    (response.rect.left(), repair)
}

/// The quiet sign of a model repair only had to tidy, `right` its right end.
fn quiet_mark(ui: &mut egui::Ui, rect: Rect, id: egui::Id, right: f32) -> egui::Response {
    let colors = theme::colors();
    let spot = Rect::from_center_size(egui::pos2(right - 9.0, rect.center().y), vec2(18.0, 18.0));
    let response = ui.interact(spot, id, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr!("models-import-found"))
    });
    let ink = if response.hovered() {
        colors.text_high
    } else {
        colors.text_low
    };
    ui.painter().text(
        spot.center(),
        Align2::CENTER_CENTER,
        icon::WARNING,
        theme::icon(14.0),
        ink,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// What is wrong with a model, what is odd about it and what repair did, and how loudly.
/// `None` when the mesh came in clean.
fn trouble(summary: &ImportSummary) -> Option<(Mark, Vec<String>)> {
    let defects = summary.defects();
    let mark = if defects.is_empty() {
        Mark::Repaired
    } else {
        Mark::Broken
    };
    let lines = [defects, summary.notes(), summary.repairs()].concat();
    (!lines.is_empty()).then_some((mark, lines))
}

#[cfg(test)]
mod tests;
