use egui::{Key, KeyboardShortcut, Modifiers};

use super::*;
use crate::shortcuts::faces::{parse, spec};

fn cmd(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}

#[test]
fn a_key_given_to_one_action_leaves_the_one_that_held_it() {
    let mut keymap = Keymap::default();
    assert_eq!(
        keymap.holder(cmd(Key::D), Action::Undo),
        Some(Action::Duplicate)
    );
    keymap.assign(Action::Undo, 0, cmd(Key::D));
    assert_eq!(keymap.chords(Action::Undo), vec![cmd(Key::D)]);
    assert!(keymap.chords(Action::Duplicate).is_empty());
    assert_eq!(keymap.changed(), 2);
}

#[test]
fn a_table_key_the_user_gave_another_action_stays_theirs_on_load() {
    let stored = BTreeMap::from([(Action::Undo.id().to_owned(), vec![spec(&cmd(Key::D))])]);
    let keymap = Keymap::from(stored);
    assert_eq!(keymap.chords(Action::Undo), vec![cmd(Key::D)]);
    assert!(
        keymap.chords(Action::Duplicate).is_empty(),
        "Cmd+D is Undo's alone"
    );
}

#[test]
fn a_key_put_back_as_the_table_has_it_is_not_kept() {
    let mut keymap = Keymap::default();
    keymap.assign(Action::Undo, 0, cmd(Key::U));
    keymap.assign(Action::Undo, 0, cmd(Key::Z));
    assert_eq!(keymap, Keymap::default());
    keymap.assign(Action::Undo, 1, cmd(Key::U));
    keymap.reset(Action::Undo);
    assert!(!keymap.is_changed(Action::Undo));
}

#[test]
fn an_action_holds_two_keys_at_most() {
    let mut keymap = Keymap::default();
    keymap.assign(Action::Undo, 1, cmd(Key::U));
    keymap.assign(Action::Undo, 2, cmd(Key::Y));
    assert_eq!(keymap.chords(Action::Undo), vec![cmd(Key::Z), cmd(Key::U)]);
}

#[test]
fn a_cleared_key_fires_nothing() {
    let mut keymap = Keymap::default();
    keymap.clear(Action::Remove, 0);
    let delete = KeyboardShortcut::new(Modifiers::NONE, Key::Delete);
    assert_eq!(keymap.chords(Action::Remove), vec![delete]);
}

/// Cmd reaches egui as `mac_cmd` and `command` on the Mac and as `ctrl` and `command`
/// elsewhere; the table writes it as `command` alone.
#[test]
fn cmd_matches_however_egui_spells_it() {
    let keymap = Keymap::default();
    let spelt = Modifiers {
        mac_cmd: cfg!(target_os = "macos"),
        ctrl: !cfg!(target_os = "macos"),
        command: true,
        ..Modifiers::NONE
    };
    let pressed = KeyboardShortcut::new(spelt, Key::D);
    assert_eq!(
        keymap.holder(pressed, Action::Undo),
        Some(Action::Duplicate)
    );
}

#[test]
fn the_users_keys_survive_the_file() {
    let mut keymap = Keymap::default();
    keymap.assign(Action::Undo, 0, cmd(Key::U));
    keymap.assign(
        Action::View(1),
        1,
        KeyboardShortcut::new(Modifiers::SHIFT, Key::Plus),
    );
    let text = serde_json::to_string(&keymap).expect("a keymap is plain strings");
    let read: Keymap = serde_json::from_str(&text).expect("it reads back what it wrote");
    assert_eq!(read, keymap);
}

#[test]
fn an_unknown_action_or_key_in_the_file_is_dropped() {
    let text = r#"{"undo":["Cmd+U","Hyper+Q","Cmd+NoSuchKey"],"teleport":["T"]}"#;
    let read: Keymap = serde_json::from_str(text).expect("unknown entries do not fail it");
    assert_eq!(read.chords(Action::Undo), vec![cmd(Key::U)]);
    assert_eq!(read.changed(), 1);
}

#[test]
fn every_key_of_the_table_reads_back_from_its_line() {
    for binding in BINDINGS {
        for chord in binding.keys {
            assert_eq!(parse(&spec(chord)), Some(*chord), "{}", spec(chord));
        }
    }
}

/// On the Mac Ctrl is a key of its own, so Ctrl Cmd F must not come back as Cmd F.
#[test]
fn a_chord_holding_ctrl_and_cmd_reads_back_from_its_line() {
    let held = normal(Modifiers::CTRL.plus(Modifiers::COMMAND));
    let chord = KeyboardShortcut::new(held, Key::F);
    let read = parse(&spec(&chord)).expect("its own line reads");
    assert_eq!(
        KeyboardShortcut::new(normal(read.modifiers), read.logical_key),
        chord
    );
}
