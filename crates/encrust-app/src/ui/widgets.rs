//! The controls the Encrust design is made of, painted from `theme` tokens only.

mod button;
mod field;
mod layout;
mod list;
mod toggle;

pub use button::{
    compact_button, companion_button, icon_button, icon_toggle, inline_button, picker,
    primary_button, secondary_button, summary_button, tool_button,
};
pub use field::{
    Carried, axis_label, carried_row, count_row, field_label, number_field, number_row, text_row,
};
pub use layout::{
    card, describe, fold, hairline, heading, hint, meta, nested, readings, section,
    section_with_action, stats, subheading,
};
pub use list::{issue_row, list, list_row};
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
