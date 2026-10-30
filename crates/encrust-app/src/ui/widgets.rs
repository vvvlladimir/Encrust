//! The controls the Encrust design is made of, painted from `theme` tokens only.

use crate::i18n::tr;

mod bin;
mod button;
mod chooser;
mod confirm;
mod field;
mod figures;
#[cfg(test)]
mod fixtures;
mod keycap;
mod layout;
mod limited;
mod list;
mod over_stage;
mod section;
mod toggle;

pub use bin::{BinLook, bin_button, confirm_bin};
pub use button::{
    RailButton, blocked_hover, compact_button, companion_button, companion_square, filter_chip,
    focus_ring, hover_mix, icon_button, icon_toggle, inline_button, picker, preset_picker,
    primary_button, primary_with_companion, quiet_button, quiet_square, rail_button,
    secondary_button, small_icon_button, summary_button, swatch, toggle_button, zone_button,
};
pub use chooser::{
    Aside, ChooserRow, Lead, chooser_foot, chooser_frame, chooser_group, chooser_row, group_name,
};
pub use confirm::{Confirm, any_armed, confirm_button};
pub use field::{
    Carried, address_field, axis_label, carried_row, count_row, field_label, number_field,
    number_row, picker_row, row_name, stepped_row, text_box, text_row, unit_field,
};
pub use figures::{Figure, details_fold, figure_grid, split_readings};
pub use keycap::keycap;
pub use layout::{
    body_and_foot, busy, card, card_foot, describe, described, describing, dialog, dialog_head,
    empty_state, field_note, hairline, heading, help_mark, hint, inset, meta, mode_group, nested,
    notice, palette_dialog, progress_bar, progress_bar_of, readings, stats, subheading,
    subheading_with_switch, vertical_hairline,
};
pub use limited::{Held, limit_verdict, limited_row};
pub use list::{issue_list, issue_row, list, list_row, tag, tag_w, two_lines};
pub use over_stage::{
    NoticeReply, ZoomPress, hint_bar, job_notice, notice_card, spinner, zoom_bar,
};
pub use section::{section, section_with_action, section_with_actions, section_with_switch};
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
