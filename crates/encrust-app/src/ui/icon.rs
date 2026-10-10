//! The Phosphor glyphs the window uses, under the names it uses them for.
//!
//! One family, one weight. A screen that wants a new icon names it here rather than
//! pasting a codepoint into a panel.

pub use egui_phosphor::regular::{
    ALIGN_CENTER_HORIZONTAL as ARRANGE, ARROW_CIRCLE_UP as UPDATE,
    ARROW_COUNTER_CLOCKWISE as RESET, ARROW_LEFT as BACK, ARROWS_OUT_CARDINAL as POSITION, BUG,
    CARET_DOWN, CARET_LEFT as PREVIOUS, CARET_LINE_RIGHT as FOLD, CARET_RIGHT, CARET_RIGHT as NEXT,
    CHECK_CIRCLE as CLEAR, CIRCLE_DASHED as HOLLOW, CIRCLE_HALF as XRAY,
    CLIPBOARD_TEXT as CLIPBOARD, COLUMNS as SIDE_BY_SIDE, COPY as DUPLICATE,
    CROSSHAIR_SIMPLE as FRAME, CUBE as MODEL, CUBE_TRANSPARENT as EMPTY_PLATE, CURSOR as SELECT,
    DROP as DRAIN, ERASER as ERASE, EXPORT, EYE as VISIBLE, EYE_SLASH as HIDDEN, FLASK as RESIN,
    FLOPPY_DISK as SAVE, FOLDER_OPEN as OPEN, GEAR as SETTINGS, GRID_FOUR as GRID,
    GRID_NINE as ARRAY, HOUSE as HOME, INFO, KEYBOARD, KNIFE as CUT, LINK as LINKED,
    LIST_CHECKS as CHECK, MAGNIFYING_GLASS as FIND, PAINT_BRUSH as PAINT, PAPER_PLANE_TILT as SEND,
    PAUSE, PERSON_SIMPLE_TAI_CHI as ORIENT, PERSPECTIVE, PLAY, PLUS as ADD, PRINTER, QUESTION,
    RULER as MEASURE, SCISSORS as SECTION, SLIDERS_HORIZONTAL as PARAMETERS,
    SPLIT_HORIZONTAL as SPLIT, SQUARE_HALF as MASK, STACK as SLICE, STAMP as RELIEF,
    TRASH as REMOVE, TREE_STRUCTURE as SUPPORTS, WARNING, WIFI_HIGH as NETWORK, WRENCH as SHAPE,
    X as CANCEL,
};

/// The window buttons the strip draws itself. macOS keeps its own over the content view,
/// and a browser tab has the browser's.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
pub use egui_phosphor::regular::{MINUS as MINIMISE, SQUARE as MAXIMISE};
