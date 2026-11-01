//! The palette behind Find: every tool, every action of the key table and every model on
//! the plate, found by typing part of its name. See `docs/decisions/0230`.

use egui::{Align2, Key, Modifiers, Sense, vec2};

use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::ObjectId;
use crate::shortcuts::{self, Action, BINDINGS};
use crate::ui::{
    chooser_group, focus_ring, hint, hover_mix, icon, key_row, keycap, palette_dialog, theme,
};
use crate::workspace::Tool;

const PALETTE_W: f32 = 500.0;
/// How far under the stage's top the palette hangs, and the field it opens with.
const TOP_GAP: f32 = 70.0;
const FIELD_H: f32 = 48.0;
const ROW_H: f32 = 32.0;
/// Room either side of the field and the rows.
const PAD: f32 = 8.0;
/// The most rows shown at once: past it the list scrolls.
const LIST_H: f32 = ROW_H * 10.0;

/// What a row of the palette does once chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Found {
    Open(Tool),
    Act(Action),
    Pick(ObjectId),
}

impl Found {
    /// The group the row is listed under, in the order the groups stand.
    fn group(self) -> usize {
        match self {
            Found::Open(_) => 0,
            Found::Act(_) => 1,
            Found::Pick(_) => 2,
        }
    }

    fn group_label(self) -> String {
        match self {
            Found::Open(_) => tr!("find-group-tools"),
            Found::Act(_) => tr!("find-group-actions"),
            Found::Pick(_) => tr!("find-group-models"),
        }
    }
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

/// The entries `query` finds by group, in each those whose name starts with it first.
fn found(mut entries: Vec<Entry>, query: &str) -> Vec<Entry> {
    entries.retain(|entry| matches(&entry.name, query));
    let lead = query.trim().to_lowercase();
    entries.sort_by_key(|entry| {
        let starts = entry.name.to_lowercase().starts_with(&lead);
        (entry.found.group(), !starts)
    });
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
    let top = theme::TOP_BAR_H + TOP_GAP;
    let modal = palette_dialog(ctx, id, top).show(ctx, |ui| {
        ui.set_width(PALETTE_W);
        ui.spacing_mut().item_spacing.y = 0.0;
        lit = steer(ui, lit, list.len());
        if field(ui, id, &mut query) {
            lit = 0;
        }
        if ui.input(|input| input.key_pressed(Key::Enter)) {
            chosen = list.get(lit).map(|entry| entry.found);
        }
        chosen = chosen.or(rows(ui, &list, lit));
        foot(ui);
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

/// The field the name is typed in, 48 tall over a hairline, which holds the focus while
/// the palette is up. Answers whether what is typed changed.
fn field(ui: &mut egui::Ui, id: egui::Id, query: &mut String) -> bool {
    let colors = theme::colors();
    let edit = egui::TextEdit::singleline(query)
        .id(id.with("field"))
        .hint_text(format!("{}  {}", icon::FIND, tr!("find-tooltip")))
        .font(theme::body())
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 0)))
        .vertical_align(egui::Align::Center)
        .desired_width(f32::INFINITY);
    let response = ui.add_sized(vec2(PALETTE_W, FIELD_H), edit);
    if !response.has_focus() {
        response.request_focus();
    }
    let esc = tr!("hint-esc");
    let rect = response.rect;
    keycap(ui.painter(), &esc, rect.right() - 16.0, rect.center().y);
    let line = response.rect.bottom();
    ui.painter().hline(
        response.rect.x_range(),
        line,
        egui::Stroke::new(1.0, colors.hairline),
    );
    response.changed()
}

/// The keys that work the palette, on a strip of its own at its foot.
fn foot(ui: &mut egui::Ui) {
    let (move_to, open) = (tr!("find-move"), tr!("find-open"));
    let keys: [(&[&str], &str); 2] = [(&["↑", "↓"], &move_to), (&[icon::KEY_RETURN], &open)];
    let top = ui.cursor().top();
    ui.painter().hline(
        ui.max_rect().x_range(),
        top,
        egui::Stroke::new(1.0, theme::colors().hairline),
    );
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(16, 6))
        .show(ui, |ui| key_row(ui, &keys, FOOT_H));
}

/// The foot's row, points.
const FOOT_H: f32 = 22.0;

/// The entries found, the lit one washed. Answers the one clicked.
fn rows(ui: &mut egui::Ui, list: &[Entry], lit: usize) -> Option<Found> {
    if list.is_empty() {
        ui.add_space(PAD);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            hint(ui, &tr!("find-nothing"));
        });
        return None;
    }
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .max_height(LIST_H)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(PAD / 2.0);
            let mut group = None;
            for (index, entry) in list.iter().enumerate() {
                if group != Some(entry.found.group()) {
                    group = Some(entry.found.group());
                    ui.horizontal(|ui| {
                        ui.add_space(PAD);
                        chooser_group(ui, &entry.found.group_label());
                    });
                }
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
    let rect = rect.shrink2(vec2(PAD, 0.0));
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, lit, &entry.name)
    });
    let (fill, ink) = match lit {
        true => (colors.picked_wash, colors.text_high),
        false => (
            hover_mix(ui, &response, egui::Color32::TRANSPARENT, colors.hover),
            colors.text_high,
        ),
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
