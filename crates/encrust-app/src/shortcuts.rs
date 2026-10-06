//! Every key the window answers, in one table.
//!
//! The handler, the rail's tooltips, the menus and the sheet all read `BINDINGS`, so a
//! shortcut cannot exist unlisted; see `docs/decisions/0106`.

use egui::{Key, KeyboardShortcut, Modifiers};

use crate::panels::{Window, duplicate_selection, frame_view, slice_this_plate, toggle_settings};
use crate::status::Status;
use crate::workspace::{Mode, Tool};

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
    ToggleMode,
    PlatePanel,
    FrameView,
    Settings,
    Sheet,
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Pick(tool) => tool.label(),
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::SelectAll => "Select all",
            Self::Deselect => "Deselect",
            Self::Duplicate => "Duplicate",
            Self::Remove => "Remove from the plate",
            Self::ToggleMode => "Prepare or Preview",
            Self::PlatePanel => "Plate contents",
            Self::FrameView => "Frame the view",
            Self::Settings => "Settings",
            Self::Sheet => "This sheet",
            Self::Step(1) => "One layer on",
            Self::Step(-1) => "One layer back",
            Self::Step(layers) if layers > 0 => "Ten layers on",
            Self::Step(_) => "Ten layers back",
            Self::Play => "Play the stack",
            Self::NewPlate => "New plate",
            Self::NewProject => "New project",
            Self::OpenProject => "Open a project",
            Self::SaveProject => "Save the project",
            Self::SaveProjectAs => "Save the project as",
            Self::OpenModel => "Import a model",
            Self::Slice => "Slice",
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Tools => "Tools, by their place on the rail",
            Self::Plate => "The plate",
            Self::Window => "The window",
            Self::Layers => "The layers",
            Self::Files => "Files",
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
    pub label: &'static str,
    pub keys: &'static str,
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
    bind(
        Group::Tools,
        Action::Pick(Tool::Select),
        &[plain(Key::Num1)],
    ),
    bind(
        Group::Tools,
        Action::Pick(Tool::Supports),
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
        Action::Pick(Tool::Layers),
        &[plain(Key::Num7)],
    ),
    bind(
        Group::Tools,
        Action::Pick(Tool::Measure),
        &[plain(Key::Num8)],
    ),
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
    bind(Group::Window, Action::ToggleMode, &[plain(Key::Tab)]),
    bind(Group::Window, Action::PlatePanel, &[plain(Key::Backslash)]),
    bind(Group::Window, Action::FrameView, &[plain(Key::F)]),
    bind(Group::Window, Action::Settings, &[cmd(Key::Comma)]),
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
        group: Group::Plate,
        label: "Add to the selection",
        keys: "Shift click",
    },
    Gesture {
        group: Group::Window,
        label: "Orbit",
        keys: "Drag",
    },
    Gesture {
        group: Group::Window,
        label: "Pan",
        keys: "Right drag",
    },
    Gesture {
        group: Group::Window,
        label: "Zoom",
        keys: "Scroll",
    },
    Gesture {
        group: Group::Layers,
        label: "Zoom into the mask",
        keys: "Scroll or pinch",
    },
    Gesture {
        group: Group::Layers,
        label: "Move the mask",
        keys: "Drag",
    },
    Gesture {
        group: Group::Layers,
        label: "Show the whole mask",
        keys: "Double click",
    },
];

/// How many modifiers the busiest chord in the table carries.
const MOST_MODIFIERS: u32 = 2;

/// The actions whose keys were pressed this frame.
///
/// egui ignores an extra Shift or Alt when matching a chord, so the busier chord has to be
/// offered first: Cmd Shift S before Cmd S.
pub fn pressed(ctx: &egui::Context) -> Vec<Action> {
    // A field being typed into owns the keyboard: S is an S, not Slice.
    if ctx.egui_wants_keyboard_input() {
        return Vec::new();
    }
    let mut fired = Vec::new();
    ctx.input_mut(|input| {
        for count in (0..=MOST_MODIFIERS).rev() {
            for binding in BINDINGS {
                let hit = binding
                    .keys
                    .iter()
                    .filter(|chord| modifiers(chord) == count)
                    .any(|chord| input.consume_shortcut(chord));
                if hit {
                    fired.push(binding.action);
                }
            }
        }
    });
    fired
}

/// Every chord that fires an action, in the order the sheet shows them.
pub fn chords(action: Action) -> &'static [KeyboardShortcut] {
    BINDINGS
        .iter()
        .find(|binding| binding.action == action)
        .map_or(&[], |binding| binding.keys)
}

/// What one chord reads as. egui spells its key names out — "Questionmark", "Backslash" —
/// which is not what a keycap says.
pub fn chord_text(chord: &KeyboardShortcut) -> String {
    let modifiers = chord.modifiers;
    let mut text = String::new();
    if cfg!(target_os = "macos") {
        // Apple's order, and no separator: the glyphs are the separator.
        for (held, glyph) in [
            (modifiers.ctrl, "Ctrl "),
            (modifiers.alt, "Alt "),
            (modifiers.shift, "\u{21e7}"),
            (modifiers.command || modifiers.mac_cmd, "\u{2318}"),
        ] {
            if held {
                text.push_str(glyph);
            }
        }
    } else {
        for (held, name) in [
            (modifiers.ctrl || modifiers.command, "Ctrl+"),
            (modifiers.alt, "Alt+"),
            (modifiers.shift, "Shift+"),
        ] {
            if held {
                text.push_str(name);
            }
        }
    }
    text.push_str(key_text(chord.logical_key));
    text
}

/// The face of a keycap. Only the keys the table uses need one of their own; everything
/// else is already what egui calls it.
fn key_text(key: Key) -> &'static str {
    match key {
        Key::Escape => "Esc",
        Key::Backspace => "\u{232b}",
        Key::Delete => {
            if cfg!(target_os = "macos") {
                "\u{2326}"
            } else {
                "Del"
            }
        }
        Key::Tab => "\u{21e5}",
        Key::Enter => "\u{23ce}",
        Key::Space => "\u{2423}",
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
        return action.label().to_owned();
    }
    format!("{}  {keys}", action.label())
}

fn modifiers(keys: &KeyboardShortcut) -> u32 {
    let modifiers = keys.modifiers;
    u32::from(modifiers.alt)
        + u32::from(modifiers.shift)
        + u32::from(modifiers.ctrl || modifiers.command || modifiers.mac_cmd)
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
        Action::Deselect => {
            window.tools.orient.stop_picking();
            window.doc.scene.clear_selection();
        }
        Action::Duplicate => duplicate_selection(window),
        Action::Remove => remove(window),
        Action::ToggleMode => *window.mode = other_mode(*window.mode),
        Action::PlatePanel => window.view.options.plate_panel = !window.view.options.plate_panel,
        Action::FrameView => frame_view(
            &window.doc.scene,
            &window.doc.plate,
            &mut window.view.camera,
        ),
        Action::Settings => toggle_settings(window.machine),
        Action::Sheet => window.view.options.sheet = true,
        Action::Step(layers) => step(window, layers),
        Action::Play => play(window),
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
        Action::OpenModel => window
            .doc
            .imports
            .open_dialog(&window.doc.plate, &mut window.machine.status),
        Action::Slice => slice_this_plate(window),
    }
}

/// Reaching for a tool is an editing act, so it brings the plate back into view.
fn pick(window: &mut Window, tool: Tool) {
    *window.mode = Mode::Prepare;
    *window.tool = tool;
}

fn other_mode(mode: Mode) -> Mode {
    match mode {
        Mode::Prepare => Mode::Preview,
        Mode::Preview => Mode::Prepare,
    }
}

fn remove(window: &mut Window) {
    let gone = window.doc.scene.remove_selected();
    if gone > 0 {
        window.machine.status = Status::Info(format!("Removed {gone} model(s)"));
    }
}

/// The transport only means anything against a stack, which is what Preview shows.
fn step(window: &mut Window, layers: i64) {
    if *window.mode == Mode::Preview {
        window.machine.preview.step(layers);
    }
}

fn play(window: &mut Window) {
    if *window.mode != Mode::Preview {
        return;
    }
    let playing = window.machine.preview.is_playing();
    window.machine.preview.set_playing(!playing);
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

    /// A tool with no key would be reachable by the pointer alone.
    #[test]
    fn every_tool_has_a_key() {
        for tool in Tool::ALL {
            assert!(
                !chords(Action::Pick(tool)).is_empty(),
                "{} is bound to nothing",
                tool.label()
            );
        }
    }

    #[test]
    fn the_rail_order_is_the_digit_order() {
        let rail: Vec<Tool> = Tool::PLACING
            .into_iter()
            .chain(Tool::SHAPING)
            .chain(Tool::PRINTING)
            .collect();
        let digits = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
        ];
        for (tool, digit) in rail.iter().zip(digits) {
            let chord = chords(Action::Pick(*tool))
                .first()
                .copied()
                .expect("every tool is in the table");
            assert_eq!(
                chord.logical_key,
                digit,
                "{} is not on the digit of its place on the rail",
                tool.label()
            );
        }
    }

    /// The handler walks the modifier counts it knows about, so a busier chord than that
    /// would never be offered first, and its looser twin would fire instead.
    #[test]
    fn the_handler_looks_for_every_chord_the_table_carries() {
        let busiest = every_chord()
            .iter()
            .map(|(_, chord)| modifiers(chord))
            .max()
            .expect("the table is not empty");
        assert_eq!(busiest, MOST_MODIFIERS);
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

    /// Feeds one key press through a context, which is what the window does with it.
    fn press(key: Key, modifiers: Modifiers) -> Vec<Action> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        ctx.begin_pass(input);
        pressed(&ctx)
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
    fn tab_asks_for_the_other_mode() {
        assert_eq!(press(Key::Tab, Modifiers::NONE), vec![Action::ToggleMode]);
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
