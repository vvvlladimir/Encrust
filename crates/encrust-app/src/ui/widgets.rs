//! The controls the Encrust design is made of, painted from `theme` tokens only.

use crate::i18n::tr;

mod bin;
mod button;
mod field;
mod layout;
mod list;
mod toggle;

pub use bin::{BinLook, bin_button, confirm_bin};
pub use button::{
    blocked_hover, compact_button, companion_button, filter_chip, focus_ring, icon_button,
    icon_toggle, inline_button, picker, primary_button, quiet_button, rail_button,
    secondary_button, summary_button, swatch, toggle_button, zone_button,
};
pub use field::{
    Carried, address_field, axis_label, carried_row, count_row, field_label, number_field,
    number_row, picker_row, stepped_row, text_box, text_row, unit_field,
};
pub use layout::{
    body_and_foot, busy, card, card_foot, describe, described, describing, dialog_frame,
    dialog_head, hairline, heading, help_mark, hint, meta, nested, notice, progress_bar, readings,
    section, section_with_action, section_with_switch, stats, subheading, subheading_with_switch,
    vertical_hairline,
};
pub use list::{issue_row, list, list_row, two_lines};
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
        seconds => tr!("ago-days", count = seconds / 86_400),
    }
}

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
    }
}
