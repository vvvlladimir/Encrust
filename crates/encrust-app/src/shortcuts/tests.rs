use super::table::{Binding, cmd, cmd_shift, plain, shift};
use super::*;
use crate::workspace::Tool;

fn every_chord() -> Vec<(&'static Binding, &'static KeyboardShortcut)> {
    BINDINGS
        .iter()
        .flat_map(|binding| binding.keys.iter().map(move |chord| (binding, chord)))
        .collect()
}

#[test]
fn no_two_bindings_share_a_chord() {
    let chords = every_chord();
    for (index, (binding, chord)) in chords.iter().enumerate() {
        for (other, twin) in &chords[index + 1..] {
            assert_ne!(
                (chord.modifiers, chord.logical_key),
                (twin.modifiers, twin.logical_key),
                "{} and {} answer the same keys",
                binding.action.label(),
                other.action.label()
            );
        }
    }
}

/// Ten digits key the first ten tools of the rail, in its order; see
/// `docs/decisions/0218`.
#[test]
fn the_rail_order_is_the_digit_order() {
    let digits = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
        Key::Num0,
    ];
    let rail: Vec<Tool> = Tool::on_rail().collect();
    for (tool, digit) in rail.iter().zip(digits) {
        let chord = chords(Action::Pick(*tool))
            .first()
            .copied()
            .expect("each of the first ten tools is in the table");
        assert_eq!(
            chord.logical_key,
            digit,
            "{} is not on the digit of its place on the rail",
            tool.label()
        );
    }
    for tool in rail.iter().skip(digits.len()) {
        assert!(chords(Action::Pick(*tool)).is_empty());
    }
}

/// egui spells its key names out. A sheet that says "Questionmark" does not tell the
/// user which key to press.
#[test]
fn a_chord_reads_as_the_keycap_does() {
    for (_, chord) in every_chord() {
        let text = chord_text(chord);
        assert!(
            !text.contains("Questionmark") && !text.contains("Backslash"),
            "{text} is a name, not a keycap"
        );
    }
    assert_eq!(chord_text(&plain(Key::Questionmark)), "?");
    assert_eq!(chord_text(&plain(Key::Backslash)), "\\");
}

#[test]
fn the_sheet_shows_every_way_to_press_an_action() {
    assert_eq!(
        chords(Action::Remove).len(),
        2,
        "both delete keys are listed"
    );
    assert_eq!(chords(Action::Sheet).len(), 3);
}

fn key_event(key: Key, modifiers: Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Feeds one key press through the hook, which is what the window does with it.
fn press(key: Key, modifiers: Modifiers) -> Vec<Action> {
    let mut input = egui::RawInput {
        events: vec![key_event(key, modifiers)],
        ..Default::default()
    };
    take(&egui::Context::default(), &mut input)
}

#[test]
fn the_question_mark_asks_for_the_sheet() {
    assert_eq!(
        press(Key::Questionmark, Modifiers::SHIFT),
        vec![Action::Sheet]
    );
    assert_eq!(press(Key::Slash, Modifiers::SHIFT), vec![Action::Sheet]);
    assert_eq!(press(Key::F1, Modifiers::NONE), vec![Action::Sheet]);
}

#[test]
fn m_turns_the_view_and_shift_m_turns_it_back() {
    assert_eq!(press(Key::M, Modifiers::NONE), vec![Action::View(1)]);
    assert_eq!(press(Key::M, Modifiers::SHIFT), vec![Action::View(-1)]);
}

/// Tab walks egui's ring of controls, so the table must never bind it.
#[test]
fn tab_is_left_to_the_ring_of_controls() {
    let mut input = egui::RawInput {
        events: vec![
            key_event(Key::Tab, Modifiers::NONE),
            key_event(Key::Tab, Modifiers::SHIFT),
        ],
        ..Default::default()
    };
    assert!(take(&egui::Context::default(), &mut input).is_empty());
    assert_eq!(input.events.len(), 2);
}

/// A control in focus is pressed with Space and let go with Esc; the rest of the table
/// still answers, so the focus never leaves the window deaf.
#[test]
fn a_control_in_focus_keeps_the_keys_that_drive_it() {
    let ctx = egui::Context::default();
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("a button")));
    let mut input = egui::RawInput {
        events: vec![
            key_event(Key::Space, Modifiers::NONE),
            key_event(Key::Escape, Modifiers::NONE),
            key_event(Key::ArrowUp, Modifiers::NONE),
            key_event(Key::ArrowUp, Modifiers::SHIFT),
            key_event(Key::M, Modifiers::NONE),
        ],
        ..Default::default()
    };
    let fired = take(&ctx, &mut input);
    assert_eq!(fired, vec![Action::Step(10), Action::View(1)]);
    assert_eq!(input.events.len(), 3, "Space, Esc and the bare arrow stay");
}

/// The looser chord must not fire in place of the busier one.
#[test]
fn shift_turns_a_save_into_a_save_as() {
    assert_eq!(
        press(Key::S, Modifiers::COMMAND.plus(Modifiers::SHIFT)),
        vec![Action::SaveProjectAs]
    );
    assert_eq!(press(Key::S, Modifiers::COMMAND), vec![Action::SaveProject]);
}

/// With nothing in focus the arrows are the layers', not egui's.
#[test]
fn a_key_the_table_binds_is_taken_out_of_the_input() {
    let mut input = egui::RawInput {
        events: vec![
            key_event(Key::M, Modifiers::NONE),
            key_event(Key::ArrowUp, Modifiers::NONE),
            key_event(Key::G, Modifiers::NONE),
        ],
        ..Default::default()
    };
    let fired = take(&egui::Context::default(), &mut input);
    assert_eq!(fired, vec![Action::View(1), Action::Step(1)]);
    assert_eq!(input.events, vec![key_event(Key::G, Modifiers::NONE)]);
}

#[test]
fn a_key_the_table_does_not_bind_fires_nothing() {
    assert!(press(Key::G, Modifiers::NONE).is_empty());
    assert!(
        press(Key::Tab, Modifiers::COMMAND).is_empty(),
        "Cmd Tab belongs to the window manager"
    );
}

#[test]
fn a_page_keeps_the_chords_a_browser_would_also_act_on() {
    assert!(kept_from_browser(Key::Z, Modifiers::COMMAND));
    assert!(kept_from_browser(
        Key::Z,
        Modifiers::COMMAND.plus(Modifiers::SHIFT)
    ));
    assert!(kept_from_browser(Key::F1, Modifiers::NONE));
    assert!(
        !kept_from_browser(Key::M, Modifiers::NONE),
        "a plain key must still type into a field"
    );
    assert!(
        !kept_from_browser(Key::C, Modifiers::COMMAND),
        "Cmd C is the browser's copy, which the window reads"
    );
}

#[test]
fn every_group_of_the_sheet_lists_something() {
    for group in Group::ALL {
        assert!(
            BINDINGS.iter().any(|binding| binding.group == group)
                || GESTURES.iter().any(|gesture| gesture.group == group),
            "{} is an empty heading",
            group.label()
        );
    }
}

/// Puts the table's own keys back when a test that changed them ends.
struct TableKeys;

impl Drop for TableKeys {
    fn drop(&mut self) {
        keymap::install(keymap::Keymap::default());
        keymap::record(None);
    }
}

#[test]
fn a_key_the_user_chose_fires_in_place_of_the_tables() {
    let _restore = TableKeys;
    keymap::change(|keymap| keymap.assign(Action::Undo, 0, cmd(Key::U)));
    assert_eq!(press(Key::U, Modifiers::COMMAND), vec![Action::Undo]);
    assert!(press(Key::Z, Modifiers::COMMAND).is_empty());
    assert_eq!(text(Action::Undo), chord_text(&cmd(Key::U)));
}

fn recording_play() -> keymap::Slot {
    let slot = keymap::Slot {
        action: Action::Play,
        index: 0,
    };
    keymap::record(Some(slot));
    slot
}

#[test]
fn recording_keeps_the_last_chord_pressed_until_done() {
    let _restore = TableKeys;
    let slot = recording_play();
    assert!(
        press(Key::M, Modifiers::SHIFT).is_empty(),
        "the table hears nothing"
    );
    assert_eq!(keymap::pressed(), Some(shift(Key::M)));
    assert!(press(Key::K, Modifiers::COMMAND.plus(Modifiers::SHIFT)).is_empty());
    assert_eq!(
        keymap::recording(),
        Some(slot),
        "a chord does not end the recording"
    );
    assert_eq!(keymap::finish(), Some((slot, cmd_shift(Key::K))));
    assert_eq!(keymap::recording(), None);
}

#[test]
fn esc_stops_recording_and_keeps_nothing() {
    let _restore = TableKeys;
    recording_play();
    press(Key::M, Modifiers::SHIFT);
    assert!(press(Key::Escape, Modifiers::NONE).is_empty());
    assert_eq!(keymap::recording(), None);
    assert_eq!(keymap::finish(), None);
}

/// The desktop sends Cmd C as a copy and never as a key, so it is recorded from that.
#[test]
fn a_copy_while_recording_is_recorded_as_cmd_c() {
    let _restore = TableKeys;
    recording_play();
    let mut input = egui::RawInput {
        events: vec![egui::Event::Copy],
        ..Default::default()
    };
    assert!(take(&egui::Context::default(), &mut input).is_empty());
    assert!(input.events.is_empty(), "nothing is copied either");
    assert_eq!(keymap::pressed(), Some(cmd(Key::C)));
}
