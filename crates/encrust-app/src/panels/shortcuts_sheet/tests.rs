use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::*;
use crate::workspace::Tool;

/// Puts the table's own keys back when the test ends.
struct TableKeys;

impl Drop for TableKeys {
    fn drop(&mut self) {
        keymap::install(keymap::Keymap::default());
        keymap::record(None);
    }
}

fn sheet() -> Harness<'static> {
    let mut dressed = false;
    let mut open = true;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .build_ui(move |ui| {
            // The fonts are bound a frame after they are handed over.
            if dressed {
                super::ui(ui.ctx(), &mut open);
            } else {
                theme::apply(ui.ctx());
                dressed = true;
            }
        });
    harness.run_steps(3);
    harness
}

/// What the window's input hook does with a key before egui reads it.
fn press(harness: &mut Harness<'_>, key: Key, modifiers: Modifiers) {
    let mut input = egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    let fired = crate::shortcuts::take(&harness.ctx, &mut input);
    assert!(fired.is_empty(), "a key recording reaches no action");
    harness.run_steps(2);
}

#[test]
fn a_chord_recorded_in_the_sheet_is_kept_once_done_is_pressed() {
    let _restore = TableKeys;
    let mut harness = sheet();
    harness
        .query_by_label_contains("Measure ")
        .expect("Measure lists its key")
        .click();
    harness.run_steps(2);
    assert!(keymap::recording().is_some(), "a press on a key records it");

    press(&mut harness, Key::U, Modifiers::COMMAND);
    press(
        &mut harness,
        Key::U,
        Modifiers::COMMAND.plus(Modifiers::SHIFT),
    );
    let table = KeyboardShortcut::new(Modifiers::NONE, Key::Num2);
    assert_eq!(
        chords(Action::Pick(Tool::Measure)),
        vec![table],
        "nothing is kept before Done"
    );

    harness.get_by_label("Done").click();
    harness.run_steps(2);
    let chord = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::U);
    assert_eq!(
        chords(Action::Pick(Tool::Measure)),
        vec![chord],
        "the chord pressed last is kept"
    );
    assert_eq!(keymap::recording(), None);
}

#[test]
fn a_chord_another_action_holds_is_moved_only_on_replace() {
    let _restore = TableKeys;
    let mut harness = sheet();
    harness
        .query_by_label_contains("Measure ")
        .expect("Measure lists its key")
        .click();
    harness.run_steps(2);
    press(&mut harness, Key::D, Modifiers::COMMAND);
    harness.get_by_label("Done").click();
    harness.run_steps(2);
    assert_eq!(chords(Action::Duplicate).len(), 1, "asked, not yet moved");

    harness.get_by_label("Replace").click();
    harness.run_steps(2);
    assert!(chords(Action::Duplicate).is_empty());
    let chord = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
    assert_eq!(chords(Action::Pick(Tool::Measure)), vec![chord]);
}
