//! The window of keys: every action of `crate::shortcuts` by group, each key a button that
//! records another in its place. See `docs/decisions/0386`.

use egui::{KeyboardShortcut, RichText, Ui, vec2};

use super::settings::frame::{child, scrolled, show, with_footer, with_header};
use super::settings::{FIND_GAP, GROUP_GAP, NavAside, find_line, nav_row};
use crate::i18n::tr;
use crate::shortcuts::keymap::{self, MOST_CHORDS, Slot};
use crate::shortcuts::{
    Action, BINDINGS, GESTURES, Group, chord_keys, chord_text, chords, modifier_keys,
};
use crate::ui::{
    BindingPress, BindingRow, Confirm, Recording, binding_row, chooser_group, confirm_button,
    group_name, icon, inline_button, key_row, notice, theme,
};

/// What the window holds between frames while it is open.
#[derive(Clone, Default)]
struct Sheet {
    query: String,
    /// The group shown, or `None` for every one.
    page: Option<Group>,
    taken: Option<Taken>,
}

/// A chord pressed for one action that another already answers, waiting on the user.
#[derive(Clone, Copy)]
struct Taken {
    slot: Slot,
    chord: KeyboardShortcut,
    holder: Action,
}

/// One row: an action that can be rebound, or a gesture listed beside them.
enum Entry {
    Bound(Action),
    Gesture(fn() -> String, fn() -> String),
}

impl Entry {
    fn label(&self) -> String {
        match self {
            Self::Bound(action) => action.label(),
            Self::Gesture(label, _) => label(),
        }
    }
}

/// Shows the window over the plate, and closes it on Escape, the cross or a click outside.
pub fn ui(ctx: &egui::Context, open: &mut bool) {
    let id = egui::Id::new("shortcuts");
    let mut sheet: Sheet = ctx.data(|data| data.get_temp(id)).unwrap_or_default();
    let title = tr!("shortcuts-title");
    let close = show(
        ctx,
        "shortcuts",
        icon::KEYBOARD,
        &title,
        |ui, list, side| {
            groups(&mut child(ui, list.shrink2(vec2(12.0, 14.0))), &mut sheet);
            page(ui, side, &mut sheet);
        },
    );
    if close {
        *open = false;
    }
    if *open {
        ctx.data_mut(|data| data.insert_temp(id, sheet));
    } else {
        keymap::record(None);
        ctx.data_mut(|data| data.remove::<Sheet>(id));
    }
}

/// Stops recording and gives the chord pressed last to its action, or holds it for the
/// user's word when another action answers it already.
fn finish(sheet: &mut Sheet) {
    let Some((slot, chord)) = keymap::finish() else {
        return;
    };
    match keymap::read(|keymap| keymap.holder(chord, slot.action)) {
        Some(holder) => {
            sheet.taken = Some(Taken {
                slot,
                chord,
                holder,
            });
        }
        None => keymap::change(|keymap| keymap.assign(slot.action, slot.index, chord)),
    }
}

/// Every row of `group`, gestures after the actions.
fn entries(group: Group) -> Vec<Entry> {
    let bound = BINDINGS
        .iter()
        .filter(|binding| binding.group == group)
        .map(|binding| Entry::Bound(binding.action));
    let gestures = GESTURES
        .iter()
        .filter(|gesture| gesture.group == group)
        .map(|gesture| Entry::Gesture(gesture.label, gesture.keys));
    bound.chain(gestures).collect()
}

/// The rows of `group` whose name holds what was typed.
fn found(group: Group, query: &str) -> Vec<Entry> {
    let wanted = query.trim().to_lowercase();
    let mut rows = entries(group);
    rows.retain(|entry| entry.label().to_lowercase().contains(&wanted));
    rows
}

fn group_icon(group: Group) -> &'static str {
    match group {
        Group::Tools => icon::TOOLS,
        Group::Plate => icon::MODEL,
        Group::View => icon::CAMERA,
        Group::Layers => icon::SLICE,
        Group::Files => icon::OPEN,
        Group::Window => icon::WINDOW,
        Group::Pointer => icon::POINTER,
    }
}

/// The groups down the left under the line that narrows them: every key, those of the
/// keyboard by group, then the pointer's.
fn groups(ui: &mut Ui, sheet: &mut Sheet) {
    ui.spacing_mut().item_spacing.y = 2.0;
    find_line(ui, &mut sheet.query, &tr!("shortcuts-find"));
    ui.add_space(FIND_GAP);
    let counts: Vec<(Group, usize)> = Group::ALL
        .into_iter()
        .map(|group| (group, found(group, &sheet.query).len()))
        .collect();
    let every = counts.iter().map(|(_, count)| count).sum();
    let all = tr!("shortcuts-all");
    if nav_row(
        ui,
        icon::KEYBOARD,
        &all,
        sheet.page.is_none(),
        NavAside::Count(every),
    )
    .clicked()
    {
        sheet.page = None;
    }
    for (group, count) in counts.into_iter().filter(|(_, count)| *count > 0) {
        if group == Group::Tools || group == Group::Pointer {
            ui.add_space(GROUP_GAP);
            let heading = match group {
                Group::Pointer => tr!("shortcuts-list-pointer"),
                _ => tr!("shortcuts-list-keyboard"),
            };
            chooser_group(ui, &heading);
        }
        let open = sheet.page == Some(group);
        let aside = NavAside::Count(count);
        if nav_row(ui, group_icon(group), &group.label(), open, aside).clicked() {
            sheet.page = Some(group);
        }
    }
}

/// The page's name and how many actions answer keys of the user's, the rows, and the bar.
fn page(ui: &mut Ui, side: egui::Rect, sheet: &mut Sheet) {
    let (mut head, content) = with_header(ui, side);
    heading(&mut head, sheet.page);
    let (mut body, mut bar) = with_footer(ui, content);
    scrolled(&mut body, |ui| rows(ui, sheet));
    foot(&mut bar, sheet);
}

fn heading(ui: &mut Ui, page: Option<Group>) {
    let colors = theme::colors();
    let changed = keymap::read(keymap::Keymap::changed);
    ui.horizontal_centered(|ui| {
        let name = page.map_or_else(|| tr!("shortcuts-all"), Group::label);
        ui.label(
            RichText::new(name)
                .font(theme::page_title(theme::PAGE_TITLE_PT))
                .color(colors.text_high),
        );
        if changed > 0 {
            ui.label(
                RichText::new(tr!("shortcuts-changed", count = changed))
                    .font(theme::page_title(theme::PAGE_TITLE_PT))
                    .color(colors.text_low),
            );
        }
    });
}

/// The rows of the page, under their group's name when the page shows every group.
fn rows(ui: &mut Ui, sheet: &mut Sheet) {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.add_space(theme::ITEM_GAP);
    let shown: Vec<Group> = sheet.page.map_or(Group::ALL.to_vec(), |group| vec![group]);
    let mut first = true;
    for group in shown {
        let entries = found(group, &sheet.query);
        if entries.is_empty() {
            continue;
        }
        if sheet.page.is_none() {
            if !first {
                ui.add_space(theme::BLOCK_GAP);
            }
            group_name(ui, &group.label());
        }
        first = false;
        for entry in &entries {
            entry_row(ui, entry, sheet);
        }
    }
    if first {
        ui.add_space(theme::ITEM_GAP);
        group_name(ui, &tr!("shortcuts-none-found"));
    }
}

fn entry_row(ui: &mut Ui, entry: &Entry, sheet: &mut Sheet) {
    let label = entry.label();
    let action = match entry {
        Entry::Gesture(_, keys) => {
            let keys = keys();
            let row = BindingRow {
                name: &label,
                chords: &[vec![keys.as_str()]],
                recording: None,
                room: false,
                changed: false,
                fixed: true,
            };
            binding_row(ui, &row);
            return;
        }
        Entry::Bound(action) => *action,
    };
    let keys = chords(action);
    let caps: Vec<Vec<&str>> = keys.iter().map(chord_keys).collect();
    let row = BindingRow {
        name: &label,
        chords: &caps,
        recording: keymap::recording()
            .filter(|slot| slot.action == action)
            .map(|slot| recording(ui, slot.index)),
        room: keys.len() < MOST_CHORDS,
        changed: keymap::read(|keymap| keymap.is_changed(action)),
        fixed: false,
    };
    if let Some(press) = binding_row(ui, &row) {
        sheet.taken = None;
        answer(sheet, action, press, keys.len());
    }
    if let Some(taken) = sheet.taken.filter(|taken| taken.slot.action == action) {
        ask_to_take(ui, sheet, taken);
    }
}

/// What the key recording shows: the chord pressed last, or the modifiers held while
/// none has been.
fn recording(ui: &Ui, index: usize) -> Recording<'static> {
    let pressed = keymap::pressed();
    let caps = match pressed {
        Some(chord) => chord_keys(&chord),
        None => modifier_keys(ui.input(|input| input.modifiers)),
    };
    Recording {
        index,
        caps,
        complete: pressed.is_some(),
    }
}

/// Does what a press in an action's row asked for.
fn answer(sheet: &mut Sheet, action: Action, press: BindingPress, held: usize) {
    let recording = keymap::recording();
    let listen = |index| {
        let slot = Slot { action, index };
        keymap::record((recording != Some(slot)).then_some(slot));
    };
    match press {
        BindingPress::Key(index) => listen(index),
        BindingPress::Add => listen(held),
        BindingPress::Done => finish(sheet),
        BindingPress::Clear(index) => {
            keymap::record(None);
            keymap::change(|keymap| keymap.clear(action, index));
        }
        BindingPress::Reset => {
            keymap::record(None);
            keymap::change(|keymap| keymap.reset(action));
        }
    }
}

/// Under the row: which action the chord would be taken from, and whether to take it.
fn ask_to_take(ui: &mut Ui, sheet: &mut Sheet, taken: Taken) {
    ui.add_space(theme::ROW_GAP);
    let keys = chord_text(&taken.chord);
    notice(
        ui,
        icon::WARNING,
        theme::colors().warn,
        &tr!(
            "shortcuts-taken",
            keys = keys,
            action = taken.holder.label()
        ),
        &tr!("shortcuts-taken-why", action = taken.holder.label()),
    );
    ui.add_space(theme::ROW_GAP);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::BUTTON_GAP;
        if inline_button(ui, "", &tr!("shortcuts-replace"), true, true).clicked() {
            let Taken { slot, chord, .. } = taken;
            keymap::change(|keymap| keymap.assign(slot.action, slot.index, chord));
            sheet.taken = None;
        }
        if inline_button(ui, "", &tr!("shortcuts-keep"), false, true).clicked() {
            sheet.taken = None;
        }
    });
    ui.add_space(theme::ROW_GAP);
}

/// Reset every key, asked by a second press; while a key records, the key that stops it.
fn foot(ui: &mut Ui, sheet: &mut Sheet) {
    let changed = keymap::read(keymap::Keymap::changed);
    let reset = Confirm {
        glyph: icon::RESET,
        text: &tr!("shortcuts-reset-all"),
        asked: &tr!("shortcuts-reset-all-asked"),
        tooltip: &tr!("shortcuts-reset-all-tooltip"),
        enabled: changed > 0,
    };
    if confirm_button(ui, &reset) {
        keymap::record(None);
        keymap::install(keymap::Keymap::default());
        sheet.taken = None;
    }
    if keymap::recording().is_some() {
        ui.add_space(theme::BLOCK_GAP);
        let stop = tr!("shortcuts-stop");
        key_row(ui, &[(&["Esc"], &stop)], theme::BUTTON_H);
    }
}

#[cfg(test)]
mod tests;
