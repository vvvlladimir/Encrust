//! The sizes a glyph is drawn at, points: one scale for every icon in the window, each named
//! for where it stands (`docs/design/ui-design-system.md`, Glyphs).

/// The question mark inside a ringed help mark.
pub const HELP: f32 = 10.0;
/// A caret after a section's title or the chip's names.
pub const CARET: f32 = 11.0;
/// Beside small text: a figure's caret, a release's step, a menu row's keys.
pub const MARK: f32 = 12.0;
/// A quiet glyph at a row's end, a small square's at rest, the search box's.
pub const ASIDE: f32 = 13.0;
/// A row's lead in a list, a chooser or a dropdown.
pub const ROW: f32 = 14.0;
/// In a 26 square, a menu row, a window's list and its head.
pub const SQUARE: f32 = 15.0;
/// In a button other than the primary, a notice, a spinner.
pub const BUTTON: f32 = 16.0;
/// In a framed square beside a primary, a printer's row.
pub const FRAMED: f32 = 18.0;
/// In the tool row.
pub const RAIL: f32 = 19.0;
/// A verdict card's and a summary button's lead.
pub const LEAD: f32 = 20.0;
/// A start page tile's.
pub const TILE: f32 = 24.0;
/// The start page's drop zone.
pub const ZONE: f32 = 28.0;
/// An empty state.
pub const EMPTY: f32 = 30.0;
/// A print with no picture of its own, in the printer sheet.
pub const PICTURE: f32 = 36.0;
