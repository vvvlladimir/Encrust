//! The controls the Encrust design is made of, painted from `theme` tokens only.

mod button;
mod field;
mod layout;
mod list;
mod toggle;

pub use button::{
    compact_button, companion_button, filter_chip, icon_button, icon_toggle, inline_button, picker,
    primary_button, quiet_button, rail_button, secondary_button, summary_button,
};
pub use field::{
    Carried, axis_label, carried_row, count_row, field_label, number_field, number_row, text_row,
};
pub use layout::{
    card, describe, dialog_frame, dialog_head, hairline, heading, hint, later, meta, nested,
    notice, progress_bar, readings, section, section_with_action, stats, subheading,
};
pub use list::{issue_row, list, list_row, two_lines};
pub use toggle::{Segment, Segmented, switch, tone};

/// `6h36m39s`, as a printer's own screen states it.
///
/// Two sections state a print time — the one a job comes to and the one an opened file
/// carries — so the shape lives here rather than in either of them.
pub fn duration(seconds: u32) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}h{minutes:02}m{seconds:02}s")
    } else {
        format!("{minutes}m{seconds:02}s")
    }
}

/// How long ago `at_s` was, as a person says it; both are seconds since the Unix epoch.
///
/// The last look for an update and the files opened lately both say it, so the shape lives
/// here rather than in either of them.
pub fn ago(at_s: Option<u64>, now_s: u64) -> String {
    let Some(at) = at_s else {
        return "never".to_owned();
    };
    match now_s.saturating_sub(at) {
        0..60 => "just now".to_owned(),
        seconds @ 60..3_600 => format!("{} min ago", seconds / 60),
        seconds @ 3_600..86_400 => format!("{} h ago", seconds / 3_600),
        seconds => match seconds / 86_400 {
            1 => "yesterday".to_owned(),
            days => format!("{days} days ago"),
        },
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
