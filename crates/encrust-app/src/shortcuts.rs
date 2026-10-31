//! Every key the window answers, in one table, and the keys the user put in its place.
//!
//! The handler, the rail's tooltips, the menus and the sheet all read the table through
//! [`chords`], so a shortcut cannot exist unlisted; see `docs/decisions/0106` and `0386`.

mod act;
mod faces;
pub mod keymap;
mod table;

use egui::{Key, KeyboardShortcut, Modifiers};

pub use act::{act, pick};
pub use faces::{chord_keys, chord_text, modifier_keys};
pub use table::{Action, BINDINGS, GESTURES, Group};

/// The actions whose keys were pressed, taken out of the raw input before egui reads it.
///
/// Tab is egui's, and walks its ring of controls. While a control holds the focus the keys
/// that drive it are egui's too, and every other key is still the table's; a field being
/// typed into owns the keyboard whole, and a key the sheet is recording owns it before
/// that. See `docs/decisions/0229`.
pub fn take(ctx: &egui::Context, raw_input: &mut egui::RawInput) -> Vec<Action> {
    if keymap::catch(raw_input) || ctx.text_edit_focused() {
        return Vec::new();
    }
    let focused = ctx.memory(|memory| memory.focused().is_some());
    let mut fired = Vec::new();
    raw_input.events.retain(|event| {
        let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            return true;
        };
        if focused && drives_the_focus(*key, *modifiers) {
            return true;
        }
        // An armed `confirm_button` takes Esc before the window it stands in.
        if *key == egui::Key::Escape && !modifiers.any() && crate::ui::any_armed(ctx) {
            return true;
        }
        match action_of(*key, *modifiers) {
            Some(action) => {
                fired.push(action);
                false
            }
            None => true,
        }
    });
    fired
}

/// Whether egui answers this key on the control in focus: Enter and Space press it, Esc
/// lets go of it, and the bare arrows move the focus or the value under it.
fn drives_the_focus(key: Key, modifiers: Modifiers) -> bool {
    !modifiers.any()
        && matches!(
            key,
            Key::Enter
                | Key::Space
                | Key::Escape
                | Key::ArrowUp
                | Key::ArrowDown
                | Key::ArrowLeft
                | Key::ArrowRight
        )
}

/// What this chord asks for, or `None` when the table does not bind it.
///
/// An extra Shift or Alt does not stop a chord matching, because a layout may need one to
/// reach the key at all — '?' is Shift and a slash on most of them. The busiest match
/// therefore wins: Cmd Shift S is Save as, not Save.
fn action_of(key: Key, modifiers: Modifiers) -> Option<Action> {
    keymap::read(|keymap| {
        BINDINGS
            .iter()
            .flat_map(|binding| {
                let chords = keymap.chords(binding.action).into_iter();
                chords.map(|chord| (binding.action, chord))
            })
            .filter(|(_, chord)| {
                chord.logical_key == key && modifiers.matches_logically(chord.modifiers)
            })
            .max_by_key(|(_, chord)| held(chord))
            .map(|(action, _)| action)
    })
}

/// Whether a browser must not act on this chord as well as the window: Safari reopens a
/// closed tab on Cmd Z, F1 opens help. A plain key stays the browser's, so typing still works.
#[cfg(any(target_arch = "wasm32", test))]
pub fn kept_from_browser(key: Key, modifiers: Modifiers) -> bool {
    let typed = !modifiers.command
        && !matches!(
            key,
            Key::F1
                | Key::F2
                | Key::F3
                | Key::F4
                | Key::F5
                | Key::F6
                | Key::F7
                | Key::F8
                | Key::F9
                | Key::F10
                | Key::F11
                | Key::F12
        );
    !typed && action_of(key, modifiers).is_some()
}

/// How many modifiers a chord carries, which is what makes one busier than another.
fn held(chord: &KeyboardShortcut) -> u32 {
    let modifiers = chord.modifiers;
    u32::from(modifiers.alt)
        + u32::from(modifiers.shift)
        + u32::from(modifiers.ctrl || modifiers.command || modifiers.mac_cmd)
}

/// Every chord that fires an action, the user's where they chose any, in the order the
/// sheet shows them.
pub fn chords(action: Action) -> Vec<KeyboardShortcut> {
    keymap::read(|keymap| keymap.chords(action))
}

/// What the keys of an action read as, or nothing if it has none.
pub fn text(action: Action) -> String {
    chords(action).first().map(chord_text).unwrap_or_default()
}

/// A label with its keys beside it: what a tooltip says.
pub fn tooltip(action: Action) -> String {
    let keys = text(action);
    if keys.is_empty() {
        return action.label();
    }
    format!("{}  {keys}", action.label())
}

#[cfg(test)]
mod tests;
