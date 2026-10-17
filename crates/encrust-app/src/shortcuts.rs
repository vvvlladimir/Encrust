//! Every key the window answers, in one table.
//!
//! The handler, the rail's tooltips, the menus and the sheet all read `BINDINGS`, so a
//! shortcut cannot exist unlisted; see `docs/decisions/0106`.

use egui::{Key, KeyboardShortcut, Modifiers};

use crate::i18n::tr;
use crate::panels::{
    Window, duplicate_selection, frame_view, section, slice_this_plate, toggle_settings,
};
use crate::status::Status;
use crate::workspace::Tool;

/// What a key asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Pick(Tool),
    Undo,
    Redo,
    SelectAll,
    Deselect,
    Duplicate,
    Remove,
    /// Views to move along the strip's three, wrapping: the next is positive.
    View(i8),
    FrameView,
    Settings,
    Sheet,
    Find,
    /// Layers to move through the stack, signed: up the stack is positive.
    Step(i64),
    Play,
    NewPlate,
    NewProject,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    OpenModel,
    Slice,
}

impl Action {
    /// What the sheet, a tooltip and a menu row call this action.
    pub fn label(self) -> String {
        match self {
            Self::Pick(tool) => tool.label(),
            Self::Undo => tr!("action-undo"),
            Self::Redo => tr!("action-redo"),
            Self::SelectAll => tr!("action-select-all"),
            Self::Deselect => tr!("action-deselect"),
            Self::Duplicate => tr!("action-duplicate"),
            Self::Remove => tr!("action-remove"),
            Self::View(step) if step > 0 => tr!("action-next-view"),
            Self::View(_) => tr!("action-previous-view"),
            Self::FrameView => tr!("action-frame-view"),
            Self::Settings => tr!("action-settings"),
            Self::Sheet => tr!("action-sheet"),
            Self::Find => tr!("action-find"),
            Self::Step(1) => tr!("action-layer-on"),
            Self::Step(-1) => tr!("action-layer-back"),
            Self::Step(layers) if layers > 0 => tr!("action-ten-layers-on"),
            Self::Step(_) => tr!("action-ten-layers-back"),
            Self::Play => tr!("action-play"),
            Self::NewPlate => tr!("action-new-plate"),
            Self::NewProject => tr!("action-new-project"),
            Self::OpenProject => tr!("action-open-project"),
            Self::SaveProject => tr!("action-save-project"),
            Self::SaveProjectAs => tr!("action-save-project-as"),
            Self::OpenModel => tr!("action-import-model"),
            Self::Slice => tr!("action-slice"),
        }
    }
}

/// Which block of the sheet a binding is listed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Tools,
    Plate,
    Window,
    Layers,
    Files,
}

impl Group {
    pub fn label(self) -> String {
        match self {
            Self::Tools => tr!("shortcut-group-tools"),
            Self::Plate => tr!("shortcut-group-plate"),
            Self::Window => tr!("shortcut-group-window"),
            Self::Layers => tr!("shortcut-group-layers"),
            Self::Files => tr!("shortcut-group-files"),
        }
    }
}

/// One action, where it is listed, and every way to press it.
pub struct Binding {
    pub group: Group,
    pub action: Action,
    /// Each chord that fires it. The first is the one a tooltip and a menu row show.
    pub keys: &'static [KeyboardShortcut],
}

/// A gesture of the pointer: listed with the keys because that is where the user looks
/// for it, read by nothing.
pub struct Gesture {
    pub group: Group,
    pub label: fn() -> String,
    pub keys: fn() -> String,
}

const fn plain(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::NONE, key)
}

const fn shift(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::SHIFT, key)
}

const fn cmd(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}

const fn cmd_shift(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), key)
}

const fn bind(group: Group, action: Action, keys: &'static [KeyboardShortcut]) -> Binding {
    Binding {
        group,
        action,
        keys,
    }
}

/// The table, in the order the sheet lists it.
pub const BINDINGS: &[Binding] = &[
    // The first ten tools of the rail, in its order. The rest are reached by the pointer.
    bind(
        Group::Tools,
        Action::Pick(Tool::Position),
        &[plain(Key::Num1)],
    ),
    bind(
        Group::Tools,
        Action::Pick(Tool::Measure),
        &[plain(Key::Num2)],
    ),
    bind(
        Group::Tools,
        Action::Pick(Tool::Hollow),
        &[plain(Key::Num3)],
    ),
    bind(Group::Tools, Action::Pick(Tool::Drain), &[plain(Key::Num4)]),
    bind(Group::Tools, Action::Pick(Tool::Cut), &[plain(Key::Num5)]),
    bind(
        Group::Tools,
        Action::Pick(Tool::Relief),
        &[plain(Key::Num6)],
    ),
    bind(
        Group::Tools,
        Action::Pick(Tool::Supports),
        &[plain(Key::Num7)],
    ),
    bind(Group::Tools, Action::Pick(Tool::Paint), &[plain(Key::Num8)]),
    bind(Group::Tools, Action::Pick(Tool::Shape), &[plain(Key::Num9)]),
    bind(Group::Tools, Action::Pick(Tool::Check), &[plain(Key::Num0)]),
    bind(Group::Plate, Action::SelectAll, &[cmd(Key::A)]),
    bind(Group::Plate, Action::Deselect, &[plain(Key::Escape)]),
    bind(Group::Plate, Action::Duplicate, &[cmd(Key::D)]),
    bind(
        Group::Plate,
        Action::Remove,
        &[plain(Key::Backspace), plain(Key::Delete)],
    ),
    bind(Group::Plate, Action::Undo, &[cmd(Key::Z)]),
    bind(Group::Plate, Action::Redo, &[cmd_shift(Key::Z)]),
    bind(Group::Window, Action::View(1), &[plain(Key::M)]),
    bind(Group::Window, Action::View(-1), &[shift(Key::M)]),
    bind(Group::Window, Action::FrameView, &[plain(Key::F)]),
    bind(Group::Window, Action::Settings, &[cmd(Key::Comma)]),
    bind(Group::Window, Action::Find, &[cmd(Key::K)]),
    bind(
        Group::Window,
        Action::Sheet,
        // A layout that reports Shift and the slash rather than the question mark still
        // asks for the sheet, and F1 works whatever the layout.
        &[plain(Key::Questionmark), shift(Key::Slash), plain(Key::F1)],
    ),
    bind(Group::Layers, Action::Step(1), &[plain(Key::ArrowUp)]),
    bind(Group::Layers, Action::Step(-1), &[plain(Key::ArrowDown)]),
    bind(Group::Layers, Action::Step(10), &[shift(Key::ArrowUp)]),
    bind(Group::Layers, Action::Step(-10), &[shift(Key::ArrowDown)]),
    bind(Group::Layers, Action::Play, &[plain(Key::Space)]),
    bind(Group::Files, Action::NewPlate, &[cmd(Key::N)]),
    bind(Group::Files, Action::NewProject, &[cmd_shift(Key::N)]),
    bind(Group::Files, Action::OpenProject, &[cmd(Key::O)]),
    bind(Group::Files, Action::SaveProject, &[cmd(Key::S)]),
    bind(Group::Files, Action::SaveProjectAs, &[cmd_shift(Key::S)]),
    bind(Group::Files, Action::OpenModel, &[cmd(Key::I)]),
    bind(Group::Files, Action::Slice, &[cmd(Key::Enter)]),
];

pub const GESTURES: &[Gesture] = &[
    Gesture {
        group: Group::Window,
        label: || tr!("gesture-move-focus"),
        keys: || tr!("gesture-move-focus-keys"),
    },
    Gesture {
        group: Group::Window,
        label: || tr!("gesture-press-focus"),
        keys: || tr!("gesture-press-focus-keys"),
    },
    Gesture {
        group: Group::Window,
        label: || tr!("gesture-leave-focus"),
        keys: || tr!("gesture-leave-focus-keys"),
    },
    Gesture {
        group: Group::Plate,
        label: || tr!("gesture-add-selection"),
        keys: || tr!("gesture-add-selection-keys"),
    },
    Gesture {
        group: Group::Plate,
        label: || tr!("gesture-pick-between"),
        keys: || tr!("gesture-pick-between-keys"),
    },
    Gesture {
        group: Group::Window,
        label: || tr!("gesture-orbit"),
        keys: || tr!("gesture-orbit-keys"),
    },
    Gesture {
        group: Group::Window,
        label: || tr!("gesture-pan"),
        keys: || tr!("gesture-pan-keys"),
    },
    Gesture {
        group: Group::Window,
        label: || tr!("gesture-zoom"),
        keys: || tr!("gesture-zoom-keys"),
    },
    Gesture {
        group: Group::Layers,
        label: || tr!("gesture-zoom-mask"),
        keys: || tr!("gesture-zoom-mask-keys"),
    },
    Gesture {
        group: Group::Layers,
        label: || tr!("gesture-move-mask"),
        keys: || tr!("gesture-move-mask-keys"),
    },
    Gesture {
        group: Group::Layers,
        label: || tr!("gesture-whole-mask"),
        keys: || tr!("gesture-whole-mask-keys"),
    },
];

/// The actions whose keys were pressed, taken out of the raw input before egui reads it.
///
/// Tab is egui's, and walks its ring of controls. While a control holds the focus the keys
/// that drive it are egui's too, and every other key is still the table's; a field being
/// typed into owns the keyboard whole. See `docs/decisions/0229`.
pub fn take(ctx: &egui::Context, raw_input: &mut egui::RawInput) -> Vec<Action> {
    if ctx.text_edit_focused() {
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
    BINDINGS
        .iter()
        .flat_map(|binding| binding.keys.iter().map(move |chord| (binding, chord)))
        .filter(|(_, chord)| {
            chord.logical_key == key && modifiers.matches_logically(chord.modifiers)
        })
        .max_by_key(|(_, chord)| held(chord))
        .map(|(binding, _)| binding.action)
}

/// How many modifiers a chord carries, which is what makes one busier than another.
fn held(chord: &KeyboardShortcut) -> u32 {
    let modifiers = chord.modifiers;
    u32::from(modifiers.alt)
        + u32::from(modifiers.shift)
        + u32::from(modifiers.ctrl || modifiers.command || modifiers.mac_cmd)
}

/// Every chord that fires an action, in the order the sheet shows them.
pub fn chords(action: Action) -> &'static [KeyboardShortcut] {
    BINDINGS
        .iter()
        .find(|binding| binding.action == action)
        .map_or(&[], |binding| binding.keys)
}

/// What one chord reads as, all its keys run together as a tooltip shows them.
pub fn chord_text(chord: &KeyboardShortcut) -> String {
    let separator = if cfg!(target_os = "macos") { "" } else { "+" };
    chord_keys(chord).join(separator)
}

/// The face of each key a chord holds down, modifiers first, one keycap each.
///
/// The Mac's key symbols are Phosphor glyphs: the text fonts have no ⇧, ⌥ or ⏎, and a
/// missing glyph draws as an empty box.
pub fn chord_keys(chord: &KeyboardShortcut) -> Vec<&'static str> {
    let modifiers = chord.modifiers;
    let held = if cfg!(target_os = "macos") {
        // Control, Option, Shift, Command: the order the platform writes them in.
        vec![
            (modifiers.ctrl, crate::ui::icon::KEY_CONTROL),
            (modifiers.alt, crate::ui::icon::KEY_OPTION),
            (modifiers.shift, crate::ui::icon::KEY_SHIFT),
            (
                modifiers.command || modifiers.mac_cmd,
                crate::ui::icon::KEY_COMMAND,
            ),
        ]
    } else {
        vec![
            (modifiers.ctrl || modifiers.command, "Ctrl"),
            (modifiers.alt, "Alt"),
            (modifiers.shift, "Shift"),
        ]
    };
    held.into_iter()
        .filter_map(|(held, face)| held.then_some(face))
        .chain([key_text(chord.logical_key)])
        .collect()
}

/// The face of a keycap. egui spells its key names out — "Questionmark", "Backslash" —
/// which is not what a keycap says; only the keys the table uses need one of their own.
fn key_text(key: Key) -> &'static str {
    let mac = cfg!(target_os = "macos");
    match key {
        Key::Escape => "Esc",
        Key::Backspace if mac => crate::ui::icon::KEY_BACKSPACE,
        Key::Backspace => "Backspace",
        Key::Delete => "Del",
        Key::Enter if mac => crate::ui::icon::KEY_RETURN,
        Key::Enter => "Enter",
        Key::Space => "Space",
        Key::ArrowUp => "\u{2191}",
        Key::ArrowDown => "\u{2193}",
        Key::Questionmark => "?",
        Key::Backslash => "\\",
        Key::Slash => "/",
        Key::Comma => ",",
        _ => key.name(),
    }
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

/// Esc over the Machine and resin window: the question or the calculator over it first,
/// then the window itself.
fn leave_machines(machine: &mut crate::state::Machine) {
    let settings = &mut machine.settings;
    if settings.confirm.take().is_some() || settings.calculators.open.take().is_some() {
        return;
    }
    settings.close_machines();
}

fn leave_settings(machine: &mut crate::state::Machine) {
    if machine.settings.confirm.take().is_none() {
        machine.settings.close();
    }
}

/// Does what a key asked for. Every action of the table is answered here, so a binding
/// cannot be added without the work behind it.
pub fn act(window: &mut Window, action: Action) {
    // The sheet is a modal: it holds the keyboard until it is closed.
    if window.view.options.sheet {
        if matches!(action, Action::Sheet | Action::Deselect) {
            window.view.options.sheet = false;
        }
        return;
    }
    // The report sheet is modal as well, and Esc has to reach it here: egui never sees
    // the key, because this is read before its pass.
    if window.machine.report.open {
        if matches!(action, Action::Deselect) {
            window.machine.report.open = false;
        }
        return;
    }
    // The Machine and resin window is modal as well: it holds the keyboard, Esc takes
    // down whatever stands over it first, and undo reaches the form in view.
    if window.machine.settings.machines {
        match action {
            Action::Deselect => leave_machines(window.machine),
            Action::Undo => _ = window.machine.settings.undo(),
            Action::Redo => _ = window.machine.settings.redo(),
            _ => {}
        }
        return;
    }
    // So is the Settings window, and Esc takes down the question over it first.
    if window.machine.settings.open {
        match action {
            Action::Deselect => leave_settings(window.machine),
            Action::Undo => _ = window.machine.settings.undo(),
            Action::Redo => _ = window.machine.settings.redo(),
            _ => {}
        }
        return;
    }
    match action {
        Action::Pick(tool) => pick(window, tool),
        Action::Undo => {
            window.doc.history.undo(
                &mut window.doc.scene,
                window.tools,
                &mut window.machine.slicing,
            );
        }
        Action::Redo => {
            window.doc.history.redo(
                &mut window.doc.scene,
                window.tools,
                &mut window.machine.slicing,
            );
        }
        Action::SelectAll => window.doc.scene.select_here(),
        // A measurement in hand goes first, so Esc on the ruler does not drop the selection.
        Action::Deselect if *window.tool == Tool::Measure && !window.tools.measure.is_empty() => {
            window.tools.measure.clear();
        }
        Action::Deselect => {
            window.tools.orient.stop_picking();
            window.doc.scene.clear_selection();
        }
        Action::Duplicate => duplicate_selection(window),
        Action::Remove => remove(window),
        Action::View(step) => section::turn_view(window, step),
        Action::FrameView => frame_view(
            &window.doc.scene,
            &window.doc.plate,
            &mut window.view.camera,
        ),
        Action::Settings => toggle_settings(window.machine),
        Action::Sheet => window.view.options.sheet = true,
        Action::Find => window.view.options.find = true,
        Action::Step(layers) => section::step(window, layers),
        Action::Play => section::play(window),
        Action::NewPlate => {
            window.doc.scene.add_plate();
        }
        Action::NewProject => crate::project::new_project(window),
        Action::OpenProject => crate::project::open_dialog(window),
        Action::SaveProject => {
            crate::project::save_open(window);
        }
        Action::SaveProjectAs => {
            crate::project::save_dialog(window);
        }
        Action::OpenModel => crate::import::open_dialog(window),
        Action::Slice => slice_this_plate(window),
    }
}

/// Opens a tool in the inspector, or folds the inspector away when the tool is open
/// already. The view stays as it is: the layer strip alone switches it; see
/// `docs/decisions/0218`.
pub(crate) fn pick(window: &mut Window, tool: Tool) {
    let options = &mut window.view.options;
    options.inspector = *window.tool != tool || !options.inspector;
    *window.tool = tool;
}

fn remove(window: &mut Window) {
    let gone = window.doc.scene.remove_selected();
    if gone > 0 {
        window.machine.status = Status::Info(tr!("status-removed-models", count = gone));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for tool in &rail[digits.len()..] {
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
    fn every_group_of_the_sheet_lists_something() {
        for group in [
            Group::Tools,
            Group::Plate,
            Group::Window,
            Group::Layers,
            Group::Files,
        ] {
            assert!(
                BINDINGS.iter().any(|binding| binding.group == group)
                    || GESTURES.iter().any(|gesture| gesture.group == group),
                "{} is an empty heading",
                group.label()
            );
        }
    }
}
