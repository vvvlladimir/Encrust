//! The Encrust design layer: the tokens the window is painted with and the widgets built
//! from them. See `docs/design/ui-design-system.md`.
//!
//! No colour, radius, gap or font size is written anywhere else in the crate.

mod fonts;
pub mod icon;
pub mod theme;
mod widgets;

pub use widgets::{
    Carried, Segment, Segmented, axis_label, card, carried_row, compact_button, companion_button,
    count_row, describe, duration, field_label, fold, hairline, heading, hint, icon_button,
    icon_toggle, inline_button, issue_row, list, list_row, meta, nested, notice, number_field,
    number_row, picker, primary_button, readings, secondary_button, section, section_with_action,
    stats, subheading, summary_button, switch, text_row, tone, tool_button,
};
