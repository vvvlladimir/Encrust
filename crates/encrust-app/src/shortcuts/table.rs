//! The table itself: every action, the group the sheet lists it under, and its keys.

use egui::{Key, KeyboardShortcut, Modifiers};

use crate::i18n::tr;
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

    /// The name the user's own keys are kept under, which no translation or reordering
    /// changes.
    pub fn id(self) -> &'static str {
        match self {
            Self::Pick(tool) => tool_id(tool),
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::SelectAll => "select-all",
            Self::Deselect => "deselect",
            Self::Duplicate => "duplicate",
            Self::Remove => "remove",
            Self::View(step) if step > 0 => "next-view",
            Self::View(_) => "previous-view",
            Self::FrameView => "frame-view",
            Self::Settings => "settings",
            Self::Sheet => "sheet",
            Self::Find => "find",
            Self::Step(1) => "layer-on",
            Self::Step(-1) => "layer-back",
            Self::Step(layers) if layers > 0 => "ten-layers-on",
            Self::Step(_) => "ten-layers-back",
            Self::Play => "play",
            Self::NewPlate => "new-plate",
            Self::NewProject => "new-project",
            Self::OpenProject => "open-project",
            Self::SaveProject => "save-project",
            Self::SaveProjectAs => "save-project-as",
            Self::OpenModel => "import-model",
            Self::Slice => "slice",
        }
    }
}

fn tool_id(tool: Tool) -> &'static str {
    match tool {
        Tool::Position => "tool-select",
        Tool::Measure => "tool-measure",
        Tool::Repair => "tool-repair",
        Tool::Hollow => "tool-hollow",
        Tool::Drain => "tool-drain",
        Tool::Cut => "tool-cut",
        Tool::Relief => "tool-relief",
        Tool::Supports => "tool-supports",
        Tool::Paint => "tool-paint",
        Tool::Shape => "tool-shape",
        Tool::Check => "tool-check",
        Tool::PrintSettings => "tool-print-settings",
    }
}

/// Which block of the sheet a binding is listed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Tools,
    Plate,
    View,
    Layers,
    Files,
    Window,
    /// What the mouse and the trackpad do: listed, never rebound.
    Pointer,
}

impl Group {
    /// Every group, in the order the sheet lists them.
    pub const ALL: [Self; 7] = [
        Self::Tools,
        Self::Plate,
        Self::View,
        Self::Layers,
        Self::Files,
        Self::Window,
        Self::Pointer,
    ];

    pub fn label(self) -> String {
        match self {
            Self::Tools => tr!("shortcut-group-tools"),
            Self::Plate => tr!("shortcut-group-plate"),
            Self::View => tr!("shortcut-group-view"),
            Self::Layers => tr!("shortcut-group-layers"),
            Self::Files => tr!("shortcut-group-files"),
            Self::Window => tr!("shortcut-group-window"),
            Self::Pointer => tr!("shortcut-group-pointer"),
        }
    }
}

/// One action, where it is listed, and every way to press it.
pub struct Binding {
    pub group: Group,
    pub action: Action,
    /// Each chord that fires it unless the user chose others. The first is the one a
    /// tooltip and a menu row show.
    pub keys: &'static [KeyboardShortcut],
}

/// A gesture of the pointer: listed with the keys because that is where the user looks
/// for it, read by nothing.
pub struct Gesture {
    pub group: Group,
    pub label: fn() -> String,
    pub keys: fn() -> String,
}

pub(super) const fn plain(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::NONE, key)
}

pub(super) const fn shift(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::SHIFT, key)
}

pub(super) const fn cmd(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}

pub(super) const fn cmd_shift(key: Key) -> KeyboardShortcut {
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
    // The tools of the rail, in its order.
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
        Action::Pick(Tool::Repair),
        &[plain(Key::Num3)],
    ),
    bind(
        Group::Tools,
        Action::Pick(Tool::Hollow),
        &[plain(Key::Num4)],
    ),
    bind(Group::Tools, Action::Pick(Tool::Drain), &[plain(Key::Num5)]),
    bind(Group::Tools, Action::Pick(Tool::Cut), &[plain(Key::Num6)]),
    bind(
        Group::Tools,
        Action::Pick(Tool::Relief),
        &[plain(Key::Num7)],
    ),
    bind(
        Group::Tools,
        Action::Pick(Tool::Supports),
        &[plain(Key::Num8)],
    ),
    bind(Group::Tools, Action::Pick(Tool::Check), &[plain(Key::Num9)]),
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
    bind(Group::View, Action::View(1), &[plain(Key::M)]),
    bind(Group::View, Action::View(-1), &[shift(Key::M)]),
    bind(Group::View, Action::FrameView, &[plain(Key::F)]),
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
        group: Group::Pointer,
        label: || tr!("gesture-add-selection"),
        keys: || tr!("gesture-add-selection-keys"),
    },
    Gesture {
        group: Group::Pointer,
        label: || tr!("gesture-pick-between"),
        keys: || tr!("gesture-pick-between-keys"),
    },
    Gesture {
        group: Group::Pointer,
        label: || tr!("gesture-orbit"),
        keys: || tr!("gesture-orbit-keys"),
    },
    Gesture {
        group: Group::Pointer,
        label: || tr!("gesture-pan"),
        keys: || tr!("gesture-pan-keys"),
    },
    Gesture {
        group: Group::Pointer,
        label: || tr!("gesture-zoom"),
        keys: || tr!("gesture-zoom-keys"),
    },
    Gesture {
        group: Group::Pointer,
        label: || tr!("gesture-zoom-mask"),
        keys: || tr!("gesture-zoom-mask-keys"),
    },
    Gesture {
        group: Group::Pointer,
        label: || tr!("gesture-move-mask"),
        keys: || tr!("gesture-move-mask-keys"),
    },
    Gesture {
        group: Group::Pointer,
        label: || tr!("gesture-whole-mask"),
        keys: || tr!("gesture-whole-mask-keys"),
    },
];
