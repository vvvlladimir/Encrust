//! The keys the user chose in place of the table's, and the chord the sheet is recording.
//!
//! Both live on the drawing thread, as the palette does, so a tooltip reads its keys with
//! no `Window` in hand; see `docs/decisions/0386`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use egui::{Key, KeyboardShortcut, Modifiers};
use serde::{Deserialize, Serialize};

use super::faces::{parse, spec};
use super::table::{Action, BINDINGS};

/// How many keys the sheet lets one action hold.
pub const MOST_CHORDS: usize = 2;

/// The actions the user gave keys of their own, and those keys; an action left out keeps
/// the table's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    from = "BTreeMap<String, Vec<String>>",
    into = "BTreeMap<String, Vec<String>>"
)]
pub struct Keymap {
    chosen: BTreeMap<&'static str, Vec<KeyboardShortcut>>,
}

impl Keymap {
    /// Every chord that fires `action`, the user's where they chose any.
    pub fn chords(&self, action: Action) -> Vec<KeyboardShortcut> {
        self.chosen
            .get(action.id())
            .map_or_else(|| defaults(action).to_vec(), Clone::clone)
    }

    /// Whether `action` answers keys other than the table's.
    pub fn is_changed(&self, action: Action) -> bool {
        self.chosen.contains_key(action.id())
    }

    /// How many actions answer keys other than the table's.
    pub fn changed(&self) -> usize {
        self.chosen.len()
    }

    /// The action other than `except` that `chord` fires as pressed, if any.
    pub fn holder(&self, chord: KeyboardShortcut, except: Action) -> Option<Action> {
        BINDINGS
            .iter()
            .map(|binding| binding.action)
            .filter(|action| *action != except)
            .find(|action| self.chords(*action).iter().any(|held| same(*held, chord)))
    }

    /// Puts `chord` in place `index` of `action`, taking it from whatever held it. An
    /// action holding [`MOST_CHORDS`] already takes no new place.
    pub fn assign(&mut self, action: Action, index: usize, chord: KeyboardShortcut) {
        let mut keys = self.chords(action);
        if let Some(at) = keys.iter().position(|held| same(*held, chord)) {
            if index < keys.len() {
                keys.swap(at, index);
                self.set(action, keys);
            }
            return;
        }
        if index >= keys.len() && keys.len() >= MOST_CHORDS {
            return;
        }
        self.take_from_others(action, chord);
        match keys.get_mut(index) {
            Some(place) => *place = chord,
            None => keys.push(chord),
        }
        self.set(action, keys);
    }

    /// Takes key `index` off `action`.
    pub fn clear(&mut self, action: Action, index: usize) {
        let mut keys = self.chords(action);
        if index < keys.len() {
            keys.remove(index);
            self.set(action, keys);
        }
    }

    /// Gives `action` back the table's keys, taking them from whatever held them since.
    pub fn reset(&mut self, action: Action) {
        for chord in defaults(action) {
            self.take_from_others(action, *chord);
        }
        self.chosen.remove(action.id());
    }

    fn take_from_others(&mut self, action: Action, chord: KeyboardShortcut) {
        for other in BINDINGS.iter().map(|binding| binding.action) {
            let theirs = self.chords(other);
            if other != action && theirs.iter().any(|held| same(*held, chord)) {
                let kept = theirs.into_iter().filter(|held| !same(*held, chord));
                self.set(other, kept.collect());
            }
        }
    }

    fn set(&mut self, action: Action, keys: Vec<KeyboardShortcut>) {
        if keys == defaults(action) {
            self.chosen.remove(action.id());
        } else {
            self.chosen.insert(action.id(), keys);
        }
    }

    /// A table key the user gave another action, as a later build may add one, is theirs:
    /// the action the table gave it to answers it no more.
    fn free_taken_defaults(&mut self) {
        let taken: Vec<KeyboardShortcut> = self.chosen.values().flatten().copied().collect();
        let is_taken = |key: &KeyboardShortcut| taken.iter().any(|held| same(*held, *key));
        for binding in BINDINGS {
            if !self.chosen.contains_key(binding.action.id()) && binding.keys.iter().any(is_taken) {
                let kept = binding.keys.iter().filter(|key| !is_taken(key)).copied();
                self.chosen.insert(binding.action.id(), kept.collect());
            }
        }
    }
}

/// What a file keeps; an action this build does not know, or a chord it cannot read, is
/// dropped.
impl From<BTreeMap<String, Vec<String>>> for Keymap {
    fn from(stored: BTreeMap<String, Vec<String>>) -> Self {
        let chosen = BINDINGS
            .iter()
            .filter_map(|binding| {
                let lines = stored.get(binding.action.id())?;
                let keys = lines.iter().filter_map(|line| parse(line));
                Some((binding.action.id(), keys.take(MOST_CHORDS).collect()))
            })
            .collect();
        let mut keymap = Self { chosen };
        keymap.free_taken_defaults();
        keymap
    }
}

impl From<Keymap> for BTreeMap<String, Vec<String>> {
    fn from(keymap: Keymap) -> Self {
        keymap
            .chosen
            .into_iter()
            .map(|(id, keys)| (id.to_owned(), keys.iter().map(spec).collect()))
            .collect()
    }
}

fn defaults(action: Action) -> &'static [KeyboardShortcut] {
    BINDINGS
        .iter()
        .find(|binding| binding.action == action)
        .map_or(&[], |binding| binding.keys)
}

/// Whether two chords are pressed the same way, whichever of egui's spellings of Cmd each
/// carries.
fn same(one: KeyboardShortcut, other: KeyboardShortcut) -> bool {
    one.logical_key == other.logical_key && normal(one.modifiers) == normal(other.modifiers)
}

/// The modifiers as the table writes them: Cmd as `command` alone, and Ctrl only on the Mac,
/// where it is a key of its own.
fn normal(modifiers: Modifiers) -> Modifiers {
    let mac = cfg!(target_os = "macos");
    Modifiers {
        alt: modifiers.alt,
        ctrl: mac && modifiers.ctrl,
        shift: modifiers.shift,
        mac_cmd: false,
        command: modifiers.command || modifiers.mac_cmd || (!mac && modifiers.ctrl),
    }
}

/// One place for a key on one action: the first of its keys, or the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub action: Action,
    pub index: usize,
}

thread_local! {
    static KEYMAP: RefCell<Keymap> = RefCell::new(Keymap::default());
    static RECORDING: Cell<Option<Slot>> = const { Cell::new(None) };
    static PRESSED: Cell<Option<KeyboardShortcut>> = const { Cell::new(None) };
}

/// The user's keys now.
pub fn current() -> Keymap {
    KEYMAP.with_borrow(Clone::clone)
}

/// Makes `keymap` the one every key and tooltip answers to.
pub fn install(keymap: Keymap) {
    KEYMAP.set(keymap);
}

/// Reads the user's keys without a copy.
pub fn read<R>(look: impl FnOnce(&Keymap) -> R) -> R {
    KEYMAP.with_borrow(look)
}

/// Edits the user's keys in place.
pub fn change(edit: impl FnOnce(&mut Keymap)) {
    KEYMAP.with_borrow_mut(edit);
}

/// Sends the chords pressed from now to `slot` rather than to the table, or stops listening
/// and forgets what was pressed.
pub fn record(slot: Option<Slot>) {
    RECORDING.set(slot);
    PRESSED.set(None);
}

/// The slot the sheet is recording a chord for, if any.
pub fn recording() -> Option<Slot> {
    RECORDING.get()
}

/// The chord pressed last while recording: the one [`finish`] keeps.
pub fn pressed() -> Option<KeyboardShortcut> {
    PRESSED.get()
}

/// Stops recording, handing over the chord pressed last and the slot it is for.
pub fn finish() -> Option<(Slot, KeyboardShortcut)> {
    let slot = RECORDING.take()?;
    PRESSED.take().map(|chord| (slot, chord))
}

/// While a slot records, takes every key out of the input and keeps the chord pressed
/// last, so a slip is put right by pressing again; Esc stops recording and keeps nothing.
/// Answers whether a slot was recording.
pub(super) fn catch(raw_input: &mut egui::RawInput) -> bool {
    if recording().is_none() {
        return false;
    }
    let mut chords = Vec::new();
    raw_input.events.retain(|event| {
        let chord = match event {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => Some(KeyboardShortcut::new(*modifiers, *key)),
            // The desktop sends these chords as what they do, never as keys.
            egui::Event::Copy => Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::C)),
            egui::Event::Cut => Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::X)),
            egui::Event::Paste(_) => Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::V)),
            egui::Event::Key { .. } | egui::Event::Text(_) => None,
            _ => return true,
        };
        chords.extend(chord);
        false
    });
    for chord in chords {
        match chord.logical_key {
            Key::Escape if !chord.modifiers.any() => {
                record(None);
                break;
            }
            // Tab walks the controls, and is never the table's.
            Key::Tab => {}
            key => PRESSED.set(Some(KeyboardShortcut::new(normal(chord.modifiers), key))),
        }
    }
    true
}

#[cfg(test)]
mod tests;
