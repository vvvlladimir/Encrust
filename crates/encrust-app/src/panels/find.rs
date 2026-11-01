//! The palette behind Find: every tool, every action of the key table and every model on
//! the plate, found by typing part of its name. See `docs/decisions/0230`.

use egui::{Key, Modifiers};

use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::ObjectId;
use crate::shortcuts::{self, Action, BINDINGS};
use crate::ui::theme::layout::find::LIST_H;
use crate::ui::{
    chooser_group, hint, icon, key_foot, palette_dialog, palette_field, palette_row, theme,
};
use crate::workspace::Tool;

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
    let top = theme::TOP_BAR_H + theme::PALETTE_TOP;
    let modal = palette_dialog(ctx, id, top).show(ctx, |ui| {
        ui.set_width(theme::PALETTE_W);
        ui.spacing_mut().item_spacing.y = 0.0;
        let steered = steer(ui, lit, list.len());
        let mut moved = steered != lit;
        lit = steered;
        if field(ui, id, &mut query) {
            lit = 0;
            moved = true;
        }
        if ui.input(|input| input.key_pressed(Key::Enter)) {
            chosen = list.get(lit).map(|entry| entry.found);
        }
        chosen = chosen.or(rows(ui, &list, lit, moved));
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
    let (hint, close) = (tr!("find-tooltip"), tr!("hint-esc"));
    let size = (theme::PALETTE_W, theme::PALETTE_FIELD_H);
    palette_field(ui, id.with("field"), query, size, (&hint, &close))
}

/// The keys that work the palette, on a strip of its own at its foot.
fn foot(ui: &mut egui::Ui) {
    let (move_to, open) = (tr!("find-move"), tr!("find-open"));
    let keys: [(&[&str], &str); 2] = [(&["↑", "↓"], &move_to), (&[icon::KEY_RETURN], &open)];
    key_foot(ui, &keys);
}

/// The entries found, the lit one washed and scrolled to on the frame it `moved`, so the wheel
/// can leave it. Answers the one clicked.
fn rows(ui: &mut egui::Ui, list: &[Entry], lit: usize, moved: bool) -> Option<Found> {
    if list.is_empty() {
        ui.add_space(theme::ITEM_GAP);
        ui.horizontal(|ui| {
            ui.add_space(theme::PALETTE_INSET);
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
            ui.add_space(theme::NOTE_GAP);
            let mut group = None;
            for (index, entry) in list.iter().enumerate() {
                if group != Some(entry.found.group()) {
                    group = Some(entry.found.group());
                    ui.horizontal(|ui| {
                        ui.add_space(theme::ITEM_GAP);
                        chooser_group(ui, &entry.found.group_label());
                    });
                }
                let response = row(ui, entry, index == lit);
                if moved && index == lit {
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
    palette_row(ui, (entry.glyph, &entry.name, &entry.keys), lit)
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

    /// How far the list has scrolled after a few frames with `lit` on its last row.
    fn scrolled_after(moved: bool) -> f32 {
        let list: Vec<Entry> = (0..40)
            .map(|index| Entry {
                found: Found::Act(Action::Undo),
                glyph: "",
                name: format!("Entry {index}"),
                keys: String::new(),
            })
            .collect();
        let ctx = egui::Context::default();
        let mut offset = 0.0;
        for _ in 0..3 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.push_id("find-test", |ui| {
                    rows(ui, &list, list.len() - 1, moved);
                    let id = ui.make_persistent_id(egui::IdSalt::new("scroll_area"));
                    let state = egui::containers::scroll_area::State::load(ui.ctx(), id);
                    offset = state.map_or(0.0, |state| state.offset.y);
                });
            });
            output.textures_delta.clear();
        }
        offset
    }

    /// BUG: the lit row was scrolled to on every frame, so the wheel could not leave it.
    #[test]
    fn the_list_follows_the_lit_row_only_on_the_frame_it_moved() {
        assert!(
            scrolled_after(true) > 0.0,
            "a lit row that moved is scrolled to"
        );
        assert_eq!(scrolled_after(false), 0.0, "the wheel is left to the list");
    }
}
