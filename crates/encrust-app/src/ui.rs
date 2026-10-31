//! The Encrust design layer: the tokens the window is painted with and the widgets built
//! from them. See `docs/design/ui-design-system.md`.
//!
//! No colour, radius, gap or font size is written anywhere else in the crate.

pub(crate) mod fonts;
pub mod icon;
pub mod kept;
pub mod theme;
mod widgets;

pub use widgets::window_frame;
pub use widgets::{
    Aside, ChooserRow, Lead, chooser_foot, chooser_frame, chooser_group, chooser_row, group_name,
};
pub use widgets::{BindingPress, BindingRow, Recording, binding_row};
pub use widgets::{CardLook, StepState, boxed, bullet, release_hero, steps, tag_label, theme_card};
pub use widgets::{
    Carried, Held, NoticeReply, RailButton, Segment, Segmented, SquareLook, address_field, ago,
    axis_label, bar_pill, bare_switch, bin_button, blocked_hover, body_and_foot, busy, card,
    card_foot, card_title, carried_row, compact_button, companion_button, companion_square,
    confirm_bin, count_row, describe, described, describing, dialog, dialog_head, duration,
    empty_state, field_label, field_note, filter_chip, focus_ring, found_row, hairline, heading,
    help_mark, hint, hint_bar, hint_bar_names, hover_mix, icon_button, icon_toggle, inline_button,
    inset, is_renaming, issue_list, issue_row, job_notice, limit_verdict, limited_row, list,
    list_row, meta, mode_column, mode_group, nested, notice, notice_card, number_field, number_row,
    numbered_square, page_trail, palette_dialog, picker_row, primary_button,
    primary_with_companion, progress_bar, progress_bar_of, quiet_button, quiet_square, rail_button,
    readings, rename_box, row_name, search_button, secondary_button, section, section_with_action,
    section_with_actions, section_with_switch, spinner, start_rename, step_bar, subheading,
    subheading_with_switch, summary_button, swatch, switch, text_box, text_row, toggle_button,
    tone, two_lines, unit_field, vertical_hairline, zone_button,
};
pub use widgets::{ChipPart, chip_text_w, name_line, paint_chip, row_mark};
pub use widgets::{Confirm, any_armed, confirm_button, is_armed, key_foot, key_row};
pub use widgets::{
    DEVICE_OPEN_INSET, DeviceRow, device_row, print_picture, share_figure, title_line,
};
pub use widgets::{
    DropdownFace, DropdownRow, MENU_GAP, dropdown, dropdown_menu, dropdown_row,
    dropdown_search_menu,
};
pub use widgets::{
    Figure, ZoomPress, details_fold, figure_grid, small_icon_button, split_readings, tag, zoom_bar,
};
pub use widgets::{
    LAYER_HANDLE, LAYER_RISK_HOT, LAYER_RISK_REACH, LayerMark, LayerMarks, PROFILE_PITCH,
    paint_layer_track, span_fraction, up,
};
pub use widgets::{MENU_W, MenuRow, menu_row, menu_rule, menu_style, menu_sub};
pub use widgets::{NAV_ROW_H, NavAside, find_line, nav_row, palette_field};
pub use widgets::{PICKER_ICON_SQUARE, PickerLine, eye_square, picker_foot, picker_line};
pub use widgets::{
    PictureTile, TILE_PICTURE_H, ZoneWords, drop_zone_box, picture_tile, start_title,
};
pub use widgets::{ReadoutWords, layer_readout};
pub use widgets::{ZONE_INSET, inline_button_w};
pub use widgets::{
    address_label, block_title, figure_line, page_title, state_word, status_glyph, tool_title,
    verdict_card,
};
