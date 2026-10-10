//! The Encrust design layer: the tokens the window is painted with and the widgets built
//! from them. See `docs/design/ui-design-system.md`.
//!
//! No colour, radius, gap or font size is written anywhere else in the crate.

mod fonts;
pub mod icon;
pub mod theme;
mod widgets;

pub use widgets::{
    Carried, Segment, Segmented, ago, axis_label, card, carried_row, compact_button,
    companion_button, count_row, describe, dialog_frame, dialog_head, duration, field_label,
    filter_chip, hairline, heading, hint, icon_button, icon_toggle, inline_button, issue_row,
    later, list, list_row, meta, nested, notice, number_field, number_row, picker, primary_button,
    progress_bar, quiet_button, rail_button, readings, secondary_button, section,
    section_with_action, stats, subheading, summary_button, switch, text_row, tone, two_lines,
};
