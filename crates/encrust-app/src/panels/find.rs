//! The palette behind Find: every tool, every action of the key table and every model on
//! the plate, found by typing part of its name. See `docs/decisions/0230`.

use egui::{Align2, Key, Modifiers, Sense, vec2};

use crate::panels::Window;
use crate::scene::ObjectId;
use crate::shortcuts::{self, Action, BINDINGS};
use crate::ui::{dialog_frame, focus_ring, icon, theme};
use crate::workspace::Tool;

const PALETTE_W: f32 = 520.0;
const ROW_H: f32 = 32.0;
/// The most rows shown at once: past it the list scrolls.
const LIST_H: f32 = ROW_H * 10.0;

/// What a row of the palette does once chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Found {
    Open(Tool),
    Act(Action),
    Pick(ObjectId),
}

struct Entry {
    found: Found,
    glyph: &'static str,
    name: String,
    keys: String,
}

/// Every entry the palette can find, tools first, then actions, then the models here.
fn entries(window: &Window) -> Vec<Entry> {
    let tools = Tool::RAIL
        .into_iter()
        .flatten()
        .copied()
        .chain([Tool::PrintSettings])
        .map(|tool| Entry {
            found: Found::Open(tool),
            glyph: tool.glyph(),
            name: tool.label().to_owned(),
            keys: shortcuts::text(Action::Pick(tool)),
        });
    let actions = BINDINGS
        .iter()
        .filter(|binding| !matches!(binding.action, Action::Pick(_) | Action::Find))
        .map(|binding| Entry {
            found: Found::Act(binding.action),
            glyph: icon::KEYBOARD,
            name: binding.action.label().to_owned(),
            keys: shortcuts::text(binding.action),
        });
    let models = window.doc.scene.here().map(|object| Entry {
        found: Found::Pick(object.id),
        glyph: icon::MODEL,
        name: object.name.clone(),
        keys: String::new(),
    });
    tools.chain(actions).chain(models).collect()
}

/// Whether every word of `query` is somewhere in `name`, regardless of case.
fn matches(name: &str, query: &str) -> bool {
    let name = name.to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| name.contains(word))
}

/// The entries `query` finds, those whose name starts with it first.
fn found(mut entries: Vec<Entry>, query: &str) -> Vec<Entry> {
    entries.retain(|entry| matches(&entry.name, query));
    let lead = query.trim().to_lowercase();
    entries.sort_by_key(|entry| !entry.name.to_lowercase().starts_with(&lead));
    entries
}

/// The palette over the window while it is open. Up and Down move along the list, Enter
/// does what is lit, and Esc or a click outside closes it.
pub fn ui(ctx: &egui::Context, window: &mut Window) {
    let id = egui::Id::new("find");
    let mut query: String = ctx.data(|data| data.get_temp(id)).unwrap_or_default();
    let mut lit: usize = ctx.data(|data| data.get_temp(id.with("lit"))).unwrap_or(0);
    let list = found(entries(window), &query);
    let mut chosen = None;
    let modal = egui::Modal::new(id)
        .frame(dialog_frame().inner_margin(egui::Margin::same(10)))
        .show(ctx, |ui| {
            ui.set_width(PALETTE_W);
            lit = steer(ui, lit, list.len());
            if field(ui, id, &mut query) {
                lit = 0;
            }
            if ui.input(|input| input.key_pressed(Key::Enter)) {
                chosen = list.get(lit).map(|entry| entry.found);
            }
            chosen = chosen.or(rows(ui, &list, lit));
        });
    let closing = modal.should_close() || chosen.is_some();
    if closing {
        ctx.data_mut(|data| {
            data.remove::<String>(id);
            data.remove::<usize>(id.with("lit"));
        });
        window.view.options.find = false;
    } else {
        ctx.data_mut(|data| {
            data.insert_temp(id, query);
            data.insert_temp(id.with("lit"), lit);
        });
    }
    if let Some(found) = chosen {
        run(window, found);
    }
}

/// Moves the lit row with the arrows, taken before the field sees them.
fn steer(ui: &mut egui::Ui, lit: usize, count: usize) -> usize {
    let (down, up) = ui.input_mut(|input| {
        let mut bare = |key| input.count_and_consume_key(Modifiers::NONE, key);
        (bare(Key::ArrowDown), bare(Key::ArrowUp))
    });
    (lit + down).saturating_sub(up).min(count.saturating_sub(1))
}

/// The field the name is typed in, which holds the focus while the palette is up.
/// Answers whether what is typed changed.
fn field(ui: &mut egui::Ui, id: egui::Id, query: &mut String) -> bool {
    let colors = theme::colors();
    let response = ui.add(
        egui::TextEdit::singleline(query)
            .id(id.with("field"))
            .hint_text("Find a tool, an action or a model")
            .font(theme::body())
            .margin(egui::Margin::symmetric(10, 8))
            .background_color(colors.sunken)
            .desired_width(f32::INFINITY),
    );
    if !response.has_focus() {
        response.request_focus();
    }
    ui.add_space(6.0);
    response.changed()
}

/// The entries found, the lit one washed. Answers the one clicked.
fn rows(ui: &mut egui::Ui, list: &[Entry], lit: usize) -> Option<Found> {
    if list.is_empty() {
        ui.label(
            egui::RichText::new("Nothing goes by that name")
                .font(theme::small())
                .color(theme::colors().text_low),
        );
        return None;
    }
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .max_height(LIST_H)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (index, entry) in list.iter().enumerate() {
                let response = row(ui, entry, index == lit);
                if index == lit {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    clicked = Some(entry.found);
                }
            }
        });
    clicked
}

fn row(ui: &mut egui::Ui, entry: &Entry, lit: bool) -> egui::Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, lit, &entry.name)
    });
    let (fill, ink) = match (lit, response.hovered()) {
        (true, _) => (colors.accent_wash, colors.accent_soft),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (egui::Color32::TRANSPARENT, colors.text_high),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let left = rect.left_center() + vec2(14.0, 0.0);
    painter.text(
        left,
        Align2::CENTER_CENTER,
        entry.glyph,
        theme::icon(14.0),
        ink,
    );
    painter.text(
        left + vec2(16.0, 0.0),
        Align2::LEFT_CENTER,
        &entry.name,
        theme::label(),
        ink,
    );
    painter.text(
        rect.right_center() - vec2(12.0, 0.0),
        Align2::RIGHT_CENTER,
        &entry.keys,
        theme::figures(11.0),
        colors.text_low,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// Does what the chosen entry stands for. A tool is opened rather than toggled: finding
/// the open tool must not fold it away.
fn run(window: &mut Window, found: Found) {
    match found {
        Found::Open(tool) => {
            *window.tool = tool;
            window.view.options.inspector = true;
        }
        Found::Act(action) => shortcuts::act(window, action),
        Found::Pick(id) => window.doc.scene.select(Some(id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_typed_has_to_be_in_the_name() {
        assert!(matches("Save the project as", "save as"));
        assert!(matches("Save the project as", "PROJ"));
        assert!(!matches("Save the project", "save as"));
        assert!(matches("anything", "  "), "nothing typed finds everything");
    }

    #[test]
    fn a_name_that_starts_with_the_query_comes_first() {
        let entry = |name: &str| Entry {
            found: Found::Act(Action::Undo),
            glyph: "",
            name: name.to_owned(),
            keys: String::new(),
        };
        let list = found(vec![entry("Frame the view"), entry("View cube")], "view");
        assert_eq!(list[0].name, "View cube");
    }
}
