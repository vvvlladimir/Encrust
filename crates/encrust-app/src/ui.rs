//! The Encrust design layer: the tokens the window is painted with and the widgets built
//! from them. See `docs/design/ui-design-system.md`.
//!
//! No colour, radius, gap or font size is written anywhere else in the crate.

pub(crate) mod fonts;
pub mod icon;
pub mod kept;
pub mod theme;
mod widgets;

pub use widgets::{
    BinLook, Carried, Held, NoticeReply, RailButton, Segment, Segmented, address_field, ago,
    axis_label, bare_switch, bin_button, blocked_hover, body_and_foot, busy, card, card_foot,
    carried_row, compact_button, companion_button, companion_square, confirm_bin, count_row,
    describe, described, describing, dialog_frame, dialog_head, duration, empty_state, field_label,
    filter_chip, focus_ring, hairline, heading, help_mark, hint, hint_bar, icon_button,
    icon_toggle, inline_button, inset, issue_row, job_notice, limit_verdict, limited_row, list,
    list_row, meta, mode_group, nested, notice, notice_card, number_field, number_row, picker,
    picker_row, preset_picker, primary_button, progress_bar, quiet_button, rail_button, readings,
    secondary_button, section, section_with_action, section_with_actions, section_with_switch,
    stats, stepped_row, subheading, subheading_with_switch, summary_button, swatch, switch,
    text_box, text_row, toggle_button, tone, two_lines, unit_field, vertical_hairline, zone_button,
};
#[expect(
    unused_imports,
    reason = "TODO(step-7, step-10): the panels these are built for"
)]
pub use widgets::{Figure, confirm_button, details_fold, figure_grid};
