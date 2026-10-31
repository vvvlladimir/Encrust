//! The controls the Encrust design is made of, painted from `theme` tokens only.

use crate::i18n::tr;

mod bin;
mod binding;
mod button;
mod chooser;
mod confirm;
mod device_row;
mod dropdown;
mod field;
mod figures;
#[cfg(test)]
mod fixtures;
mod hint_bar;
mod keycap;
mod layer_readout;
mod layer_track;
mod layout;
mod limited;
mod list;
mod menu;
mod nav;
mod over_stage;
mod picker_line;
mod print_chip;
mod release;
mod search;
mod section;
mod start_page;
mod theme_card;
mod toggle;
pub mod window_frame;

pub use bin::{bin_button, confirm_bin};
pub use binding::{BindingPress, BindingRow, Recording, binding_row};
pub use button::inline_button_w;
pub use button::{
    RailButton, SquareLook, bar_pill, blocked_hover, compact_button, companion_button,
    companion_square, filter_chip, focus_ring, hover_mix, icon_button, icon_toggle, inline_button,
    numbered_square, primary_button, primary_with_companion, quiet_button, quiet_square,
    rail_button, secondary_button, small_icon_button, summary_button, swatch, toggle_button,
    zone_button,
};
pub use chooser::{
    Aside, ChooserRow, Lead, chooser_foot, chooser_frame, chooser_group, chooser_row, group_name,
};
pub use confirm::{Confirm, any_armed, confirm_button, is_armed};
pub use device_row::{DEVICE_OPEN_INSET, DeviceRow, device_row, print_picture};
pub use dropdown::{
    DropdownFace, DropdownRow, MENU_GAP, dropdown, dropdown_menu, dropdown_row,
    dropdown_search_menu,
};
pub use field::{
    Carried, address_field, axis_label, carried_row, count_row, field_label, number_field,
    number_row, picker_row, row_name, text_box, text_row, unit_field,
};
pub use figures::{Figure, details_fold, figure_grid, split_readings};
pub use hint_bar::{hint_bar, hint_bar_names};
pub use keycap::{key_foot, key_row};
pub use layer_readout::{ReadoutWords, layer_readout};
pub use layer_track::{
    LAYER_HANDLE, LAYER_RISK_HOT, LAYER_RISK_REACH, LayerMark, LayerMarks, PROFILE_PITCH,
    paint_layer_track, span_fraction, up,
};
pub use layout::{
    address_label, block_title, body_and_foot, boxed, busy, card, card_foot, card_title, describe,
    described, describing, dialog, dialog_head, empty_state, field_note, figure_line, hairline,
    heading, help_mark, hint, inset, meta, mode_column, mode_group, name_line, nested, notice,
    page_title, page_trail, palette_dialog, progress_bar, progress_bar_of, readings, share_figure,
    state_word, status_glyph, step_bar, subheading, subheading_with_switch, title_line, tool_title,
    verdict_card, vertical_hairline,
};
pub use limited::{Held, limit_verdict, limited_row};
pub use list::{
    bullet, found_row, is_renaming, issue_list, issue_row, list, list_row, rename_box, row_mark,
    start_rename, tag, tag_label, two_lines,
};
pub use menu::{MENU_W, MenuRow, menu_row, menu_rule, menu_style, menu_sub};
pub use nav::{NAV_ROW_H, NavAside, nav_row};
pub use over_stage::{NoticeReply, ZoomPress, job_notice, notice_card, spinner, zoom_bar};
pub use picker_line::{PICKER_ICON_SQUARE, PickerLine, eye_square, picker_foot, picker_line};
pub use print_chip::{ChipPart, chip_text_w, paint_chip};
pub use release::{StepState, release_hero, steps};
pub use search::{find_line, palette_field, search_button};
pub use section::{section, section_with_action, section_with_actions, section_with_switch};
pub use start_page::ZONE_INSET;
pub use start_page::{
    PictureTile, TILE_PICTURE_H, ZoneWords, drop_zone_box, picture_tile, start_title,
};
pub use theme_card::{CardLook, theme_card};
pub use toggle::{Segment, Segmented, bare_switch, switch, tone};

/// `6h36m39s`, as a printer's own screen states it.
///
/// Two sections state a print time — the one a job comes to and the one an opened file
/// carries — so the shape lives here rather than in either of them.
pub fn duration(seconds: u32) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        tr!(
            "duration-hours",
            hours = hours,
            minutes = format!("{minutes:02}"),
            seconds = format!("{seconds:02}")
        )
    } else {
        tr!(
            "duration-minutes",
            minutes = minutes,
            seconds = format!("{seconds:02}")
        )
    }
}

/// How long ago `at_s` was, as a person says it; both are seconds since the Unix epoch.
///
/// The last look for an update and the files opened lately both say it, so the shape lives
/// here rather than in either of them.
pub fn ago(at_s: Option<u64>, now_s: u64) -> String {
    let Some(at) = at_s else {
        return tr!("ago-never");
    };
    match now_s.saturating_sub(at) {
        0..60 => tr!("ago-just-now"),
        seconds @ 60..3_600 => tr!("ago-minutes", count = seconds / 60),
        seconds @ 3_600..86_400 => tr!("ago-hours", count = seconds / 3_600),
        seconds @ 86_400..WEEK_S => tr!("ago-days", count = seconds / 86_400),
        seconds => tr!("ago-weeks", count = seconds / WEEK_S),
    }
}

const WEEK_S: u64 = 7 * 86_400;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_gone_by_reads_as_a_person_says_it() {
        let now = 10_000_000;
        assert_eq!(ago(None, now), "never");
        assert_eq!(ago(Some(now - 5), now), "just now");
        assert_eq!(ago(Some(now - 600), now), "10 min ago");
        assert_eq!(ago(Some(now - 7_200), now), "2 h ago");
        assert_eq!(ago(Some(now - 90_000), now), "yesterday");
        assert_eq!(ago(Some(now - 3 * 86_400), now), "3 days ago");
        assert_eq!(ago(Some(now - 8 * 86_400), now), "last week");
        assert_eq!(ago(Some(now - 15 * 86_400), now), "2 weeks ago");
    }
}
