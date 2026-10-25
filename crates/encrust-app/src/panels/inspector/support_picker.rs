use egui::{Sense, vec2};

use super::scope;
use crate::i18n::tr;
use crate::panels::Window;
use crate::status::Status;
use crate::ui::{BinLook, bin_button, hairline, theme};
use crate::workspace::Tool;

mod list;
mod rows;
mod save;
use list::{List, removal};
use rows::{Act, Pick, Row, in_hand, presets, rows};
use save::save_button;

/// Height of one line of the list, points.
const LINE_H: f32 = 26.0;
/// How far the shapes of the automatic preset stand in under it, points.
const SHAPE_INDENT: f32 = 16.0;

/// The preset in hand on one line under the card's heading: the picker that is also the
/// list of groups standing, Save, and the bin. See `docs/decisions/0336`.
pub(super) fn line(ui: &mut egui::Ui, window: &mut Window) {
    let mut acts = Vec::new();
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                // The group in hand may not change while a model's own numbers are held.
                scope::editing_supports(window, |window| acts = picker(ui, window));
            });
        });
    hairline(ui);
    apply(window, acts);
}

/// The picker, Save and the bin. Answers what was asked of the groups.
fn picker(ui: &mut egui::Ui, window: &mut Window) -> Vec<Act> {
    let rows = rows(window);
    let mut acts = Vec::new();
    let Some(active) = rows.iter().find(|row| row.active) else {
        return acts;
    };
    let hand = in_hand(window, active);
    let presets = presets(&window.machine.slicing.catalogue);
    ui.spacing_mut().item_spacing.x = 4.0;
    let beside = 2.0 * (theme::ICON_SIZE + 4.0);
    let width = (ui.available_width() - beside).max(80.0);
    let (_, response) = ui.allocate_exact_size(vec2(width, theme::FIELD_H), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, tr!("supports-preset"))
    });
    let label = if hand.edited {
        tr!("shape-edited", name = hand.name.clone())
    } else {
        hand.name.clone()
    };
    closed(ui, &response, active, &label);
    let list = List {
        rows: &rows,
        presets: &presets,
        hand: &hand,
        shaping: *window.tool == Tool::Shape,
    };
    let mut pick = None;
    egui::Popup::menu(&response)
        .width(width + beside)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(width + beside - 12.0);
            ui.spacing_mut().item_spacing.y = 1.0;
            pick = list.show(ui, &mut acts);
        });
    save_button(ui, window, &hand);
    let (act, tip) = removal(rows.len(), active);
    if bin_button(ui, theme::ICON_SIZE, BinLook::Quiet, &tip, active.holds > 0) {
        acts.push(act);
    }
    take(window, pick);
    acts
}

/// The closed picker, drawn as every list of the window is: the group's colour and count
/// while it holds supports, the preset's name, and the caret.
fn closed(ui: &egui::Ui, response: &egui::Response, row: &Row, label: &str) {
    let rect = response.rect;
    let visuals = ui.style().interact(response);
    let painter = ui.painter();
    painter.rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let caret = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, rect.center().y),
        vec2(9.0, 6.0),
    );
    painter.add(egui::Shape::convex_polygon(
        vec![caret.left_top(), caret.right_top(), caret.center_bottom()],
        visuals.fg_stroke.color,
        egui::Stroke::NONE,
    ));
    let mut right = caret.left() - 10.0;
    let mut left = rect.left() + 10.0;
    if row.holds > 0 {
        right = count_at(painter, right, rect.center().y, row.holds) - 8.0;
        swatch(
            painter,
            egui::pos2(rect.left() + 14.0, rect.center().y),
            row,
        );
        left = rect.left() + 26.0;
    }
    let clip = egui::Rect::from_x_y_ranges(left..=right, rect.y_range());
    painter.with_clip_rect(clip).text(
        egui::pos2(left, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        theme::label(),
        visuals.text_color(),
    );
}

/// The count ending at `right`; answers where it begins.
fn count_at(painter: &egui::Painter, right: f32, y: f32, count: usize) -> f32 {
    let low = theme::colors().text_low;
    let galley = painter.layout_no_wrap(count.to_string(), theme::figures(11.5), low);
    let left = right - galley.size().x;
    painter.galley(egui::pos2(left, y - galley.size().y / 2.0), galley, low);
    left
}

/// The group's colour, faded while it is hidden.
fn swatch(painter: &egui::Painter, center: egui::Pos2, row: &Row) {
    let tint = theme::support_tint(row.group as usize);
    let tint = if row.hidden {
        tint.gamma_multiply(0.35)
    } else {
        tint
    };
    painter.rect_filled(
        egui::Rect::from_center_size(center, vec2(8.0, 8.0)),
        2.0,
        tint,
    );
}

/// Takes up the preset chosen: an automatic one switches the sizing on, one by hand off.
fn take(window: &mut Window, pick: Option<Pick>) {
    let catalogue = &window.machine.slicing.catalogue;
    let tool = &mut window.tools.supports;
    let found = match pick {
        Some(Pick::Auto(id)) => {
            if let Some(entry) = catalogue.autos().find(|entry| entry.id == id) {
                tool.set_automatic(true, catalogue);
                tool.auto = Some(entry.profile.clone());
            }
            Ok(())
        }
        Some(Pick::Preset(id)) => catalogue.support(&id).map(|entry| {
            tool.set_automatic(false, catalogue);
            tool.chosen = Some(entry.profile.clone());
        }),
        Some(Pick::Shape(id)) => catalogue.support(&id).map(|entry| {
            tool.chosen = Some(entry.profile.clone());
        }),
        None => Ok(()),
    };
    if let Err(error) = found {
        window.machine.status = Status::failed(&error.into());
    }
}

/// Does what the clicks on the groups asked. A group chosen brings its preset, and with
/// it the sizing that preset belongs to.
fn apply(window: &mut Window, acts: Vec<Act>) {
    let catalogue = &window.machine.slicing.catalogue;
    let tool = &mut window.tools.supports;
    let scene = &mut window.doc.scene;
    for act in acts {
        match act {
            Act::Choose(group) => {
                tool.choose_group(group);
                let name = &tool.profile.name;
                let found = catalogue
                    .supports()
                    .find(|entry| entry.profile.name == *name);
                if let Some(entry) = found {
                    tool.set_automatic(catalogue.is_auto_shape(&entry.id), catalogue);
                }
            }
            Act::Toggle(group) => {
                let entry = &mut tool.groups[group as usize];
                entry.hidden = !entry.hidden;
            }
            Act::Clear(group) => {
                for object in scene.objects_mut() {
                    object.supports.clear_group(group);
                }
                tool.picked.clear();
            }
            Act::Drop(group) => {
                let name = if group == tool.active {
                    tool.profile.name.clone()
                } else {
                    tool.groups[group as usize].profile.name.clone()
                };
                let gone = tool.remove_group(group, scene);
                window.machine.status =
                    Status::Info(tr!("paint-dropped", name = name, count = gone));
            }
        }
    }
}
