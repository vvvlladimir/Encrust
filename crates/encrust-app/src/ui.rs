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
    Aside, ChooserRow, Lead, chooser_foot, chooser_frame, chooser_group, chooser_row, group_name,
};
pub use widgets::{
    BinLook, Carried, Held, NoticeReply, RailButton, Segment, Segmented, address_field, ago,
    axis_label, bare_switch, bin_button, blocked_hover, body_and_foot, busy, card, card_foot,
    card_title, carried_row, compact_button, companion_button, companion_square, confirm_bin,
    count_row, describe, described, describing, dialog, dialog_head, dialog_head_searching,
    duration, empty_state, field_label, field_note, filter_chip, focus_ring, found_row, hairline,
    heading, help_mark, hint, hint_bar, hint_bar_names, hover_mix, icon_button, icon_toggle,
    inline_button, inset, issue_list, issue_row, job_notice, limit_verdict, limited_row, list,
    list_row, meta, mode_column, mode_group, nested, notice, notice_card, number_field, number_row,
    palette_dialog, picker_row, primary_button, primary_with_companion, progress_bar,
    progress_bar_of, quiet_button, quiet_square, rail_button, readings, row_name, secondary_button,
    section, section_with_action, section_with_actions, section_with_switch, spinner, stats,
    stepped_row, subheading, subheading_with_switch, summary_button, swatch, switch, text_box,
    text_row, toggle_button, tone, two_lines, unit_field, vertical_hairline, zone_button,
};
pub use widgets::{Confirm, any_armed, confirm_button, key_row, keycap, keycap_width};
pub use widgets::{DropdownFace, DropdownRow, dropdown, dropdown_menu, dropdown_row};
pub use widgets::{
    Figure, ZoomPress, details_fold, figure_grid, small_icon_button, split_readings, tag, tag_w,
    zoom_bar,
};
