//! The Phosphor glyphs the window uses, under the names it uses them for.
//!
//! One family, one weight. A screen that wants a new icon names it here rather than
//! pasting a codepoint into a panel.

pub use egui_phosphor::regular::{
    ALIGN_CENTER_HORIZONTAL as ARRANGE, ARROW_CIRCLE_UP as UPDATE,
    ARROW_COUNTER_CLOCKWISE as RESET, ARROW_LEFT as BACK, ARROW_SQUARE_OUT as OPEN_SHEET,
    ARROWS_CLOCKWISE as RESCAN, BANDAIDS as REPAIR, BUG, CARET_DOWN, CARET_DOWN as LAYER_DOWN,
    CARET_RIGHT, CARET_UP as LAYER_UP, CHECK as IN_HAND, CHECK_CIRCLE as CLEAR,
    CIRCLE_DASHED as HOLLOW, CIRCLE_HALF as XRAY, CIRCLE_NOTCH as JOB, CLIPBOARD_TEXT as CLIPBOARD,
    CLOCK_COUNTER_CLOCKWISE as RECENT, COLUMNS as SIDE_BY_SIDE, COPY as DUPLICATE, CUBE as MODEL,
    CUBE_FOCUS as EMPTY_PLATE, CURSOR as SELECT, DROP_HALF_BOTTOM as DRAIN, ERASER as ERASE,
    EXPORT, EYE as VISIBLE, EYE_SLASH as HIDDEN, FLASK as RESIN, FLOPPY_DISK as SAVE,
    FOLDER_OPEN as OPEN, FOLDER_SIMPLE as TEST_PRINTS, FRAME_CORNERS as FRAME, GEAR as SETTINGS,
    GRID_FOUR as GRID, GRID_NINE as ARRAY, HOUSE as HOME, INFO, KEYBOARD, KNIFE as CUT,
    LINK as LINKED, LIST_CHECKS as CHECK, MAGIC_WAND as AUTO_ORIENT, MAGNIFYING_GLASS as FIND,
    PAINT_BRUSH as PAINT, PAUSE, PENCIL_SIMPLE as EDIT, PERSPECTIVE, PLAY, PLUS as ADD, PRINTER,
    QUESTION as HELP, QUESTION as ARMED, RULER as MEASURE, SELECTION_INVERSE as INVERT,
    SELECTION_PLUS as PICK_SMALL, SLIDERS as GENERAL, SLIDERS_HORIZONTAL as PARAMETERS,
    SPLIT_HORIZONTAL as SPLIT, SQUARE_HALF as MASK, STACK as SLICE, STAMP as RELIEF, STOP,
    SWAP as FLIP, TRASH as REMOVE, TRAY_ARROW_DOWN as IMPORT, TREE_STRUCTURE as SUPPORTS,
    UPLOAD_SIMPLE as DROP_HERE, VIDEO_CAMERA as CAMERA, WARNING, WIFI_HIGH as NETWORK,
    WRENCH as SHAPE, X as CANCEL,
};

/// Glyphs of the glyph table no panel draws yet.
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "TODO(step-5, step-6): the panels that draw them"
    )
)]
pub use egui_phosphor::regular::{
    ALIGN_BOTTOM as DROP_TO_PLATE, BOOKMARK_SIMPLE as PRESET, CHECKERBOARD as TEXTURE,
    LINE_SEGMENT as POINTS, PARALLELOGRAM as PLANES, SHIELD as BLOCKER, WARNING_OCTAGON as ERROR,
};

/// The faces of the keys the plain fonts have no glyph for: Shift, Option, Return and
/// the rest of a Mac keyboard's symbols.
pub use egui_phosphor::regular::{
    ARROW_FAT_UP as KEY_SHIFT, BACKSPACE as KEY_BACKSPACE, COMMAND as KEY_COMMAND,
    CONTROL as KEY_CONTROL, KEY_RETURN, OPTION as KEY_OPTION,
};

/// The window buttons the strip draws itself. macOS keeps its own over the content view,
/// and a browser tab has the browser's.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
pub use egui_phosphor::regular::{MINUS as MINIMISE, SQUARE as MAXIMISE};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// Every name above, by name.
    const NAMED: &[(&str, &str)] = &[
        ("ARRANGE", ARRANGE),
        ("UPDATE", UPDATE),
        ("RESCAN", RESCAN),
        ("RESET", RESET),
        ("BACK", BACK),
        ("OPEN_SHEET", OPEN_SHEET),
        ("REPAIR", REPAIR),
        ("BUG", BUG),
        ("CARET_DOWN", CARET_DOWN),
        ("LAYER_DOWN", LAYER_DOWN),
        ("CARET_RIGHT", CARET_RIGHT),
        ("LAYER_UP", LAYER_UP),
        ("IN_HAND", IN_HAND),
        ("CLEAR", CLEAR),
        ("HOLLOW", HOLLOW),
        ("XRAY", XRAY),
        ("JOB", JOB),
        ("CLIPBOARD", CLIPBOARD),
        ("RECENT", RECENT),
        ("SIDE_BY_SIDE", SIDE_BY_SIDE),
        ("DUPLICATE", DUPLICATE),
        ("FRAME", FRAME),
        ("MODEL", MODEL),
        ("EMPTY_PLATE", EMPTY_PLATE),
        ("SELECT", SELECT),
        ("DRAIN", DRAIN),
        ("ERASE", ERASE),
        ("EXPORT", EXPORT),
        ("VISIBLE", VISIBLE),
        ("HIDDEN", HIDDEN),
        ("RESIN", RESIN),
        ("SAVE", SAVE),
        ("OPEN", OPEN),
        ("TEST_PRINTS", TEST_PRINTS),
        ("SETTINGS", SETTINGS),
        ("GRID", GRID),
        ("ARRAY", ARRAY),
        ("HOME", HOME),
        ("INFO", INFO),
        ("KEYBOARD", KEYBOARD),
        ("CUT", CUT),
        ("LINKED", LINKED),
        ("CHECK", CHECK),
        ("FIND", FIND),
        ("PAINT", PAINT),
        ("PAUSE", PAUSE),
        ("EDIT", EDIT),
        ("AUTO_ORIENT", AUTO_ORIENT),
        ("PERSPECTIVE", PERSPECTIVE),
        ("PLAY", PLAY),
        ("ADD", ADD),
        ("PRINTER", PRINTER),
        ("HELP", HELP),
        ("ARMED", ARMED),
        ("MEASURE", MEASURE),
        ("INVERT", INVERT),
        ("PICK_SMALL", PICK_SMALL),
        ("GENERAL", GENERAL),
        ("PARAMETERS", PARAMETERS),
        ("SPLIT", SPLIT),
        ("MASK", MASK),
        ("SLICE", SLICE),
        ("RELIEF", RELIEF),
        ("STOP", STOP),
        ("FLIP", FLIP),
        ("REMOVE", REMOVE),
        ("IMPORT", IMPORT),
        ("SUPPORTS", SUPPORTS),
        ("DROP_HERE", DROP_HERE),
        ("CAMERA", CAMERA),
        ("WARNING", WARNING),
        ("NETWORK", NETWORK),
        ("SHAPE", SHAPE),
        ("CANCEL", CANCEL),
        ("DROP_TO_PLATE", DROP_TO_PLATE),
        ("PRESET", PRESET),
        ("TEXTURE", TEXTURE),
        ("POINTS", POINTS),
        ("PLANES", PLANES),
        ("BLOCKER", BLOCKER),
        ("ERROR", ERROR),
        ("KEY_SHIFT", KEY_SHIFT),
        ("KEY_BACKSPACE", KEY_BACKSPACE),
        ("KEY_COMMAND", KEY_COMMAND),
        ("KEY_CONTROL", KEY_CONTROL),
        ("KEY_RETURN", KEY_RETURN),
        ("KEY_OPTION", KEY_OPTION),
    ];

    /// Names the glyph table gives one glyph: Help and Armed are both a question; a caret
    /// folds a block and steps a layer down alike.
    const SHARED: &[[&str; 2]] = &[["HELP", "ARMED"], ["CARET_DOWN", "LAYER_DOWN"]];

    #[test]
    fn no_glyph_stands_for_two_meanings() {
        let mut by_glyph: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (name, glyph) in NAMED {
            by_glyph.entry(glyph).or_default().push(name);
        }
        for names in by_glyph.values().filter(|names| names.len() > 1) {
            assert!(
                SHARED
                    .iter()
                    .any(|pair| names.len() == 2 && pair.iter().all(|n| names.contains(n))),
                "{names:?} share one glyph, and the glyph table gives each its own"
            );
        }
    }
}
