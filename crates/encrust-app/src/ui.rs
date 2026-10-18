//! The Encrust design layer: the tokens the window is painted with and the widgets built
//! from them. See `docs/design/ui-design-system.md`.
//!
//! No colour, radius, gap or font size is written anywhere else in the crate.

pub(crate) mod fonts;
pub mod icon;
pub mod theme;
mod widgets;

pub use widgets::{
    Carried, Segment, Segmented, address_field, ago, axis_label, body_and_foot, card, card_foot,
    carried_row, compact_button, companion_button, count_row, describe, described, describing,
    dialog_frame, dialog_head, duration, field_label, filter_chip, focus_ring, hairline, heading,
    help_mark, hint, icon_button, icon_toggle, inline_button, issue_row, list, list_row, meta,
    nested, notice, number_field, number_row, picker, primary_button, progress_bar, quiet_button,
    rail_button, readings, secondary_button, section, section_with_action, section_with_switch,
    stats, stepped_row, subheading, subheading_with_switch, summary_button, switch, text_box,
    text_row, tone, two_lines, unit_field, zone_button,
};
