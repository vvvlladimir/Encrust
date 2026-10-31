use egui::{Sense, Ui, vec2};

use crate::ui::theme;

/// Points between the name and the value of a reading, which is the least that still
/// reads as two columns.
const READING_GAP: f32 = 10.0;

/// Height of a reading's row, and the room kept between the leader and the text.
const READING_H: f32 = 22.0;
const LEADER_GAP: f32 = 6.0;

/// Lines a value may wrap to before it is cut: a panel size states pixels and
/// millimetres, which is two lines in a column this narrow.
const VALUE_ROWS: usize = 2;

/// Rows of one reading each, the name at the left, the value at the right and a dotted
/// leader between them, for a summary read top to bottom rather than compared across.
///
/// A value too long for the row wraps and then is cut, so that the name beside it stays
/// readable: nothing here is drawn over anything else.
pub fn readings(ui: &mut Ui, entries: &[(String, String)]) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (key, value) in entries {
            reading(ui, key, value);
        }
    });
}

fn reading(ui: &mut Ui, key: &str, value: &str) {
    let colors = theme::colors();
    {
        let name = ui
            .painter()
            .layout_no_wrap(key.to_owned(), theme::reading(), colors.text_mid);
        let room = ui.available_width() - name.size().x - READING_GAP;
        let mut job = egui::text::LayoutJob::simple(
            value.to_owned(),
            theme::reading(),
            colors.text_high,
            room.max(READING_GAP),
        );
        job.wrap.max_rows = VALUE_ROWS;
        job.halign = egui::Align::RIGHT;
        let reading = ui.painter().layout_job(job);

        let height = READING_H.max(reading.size().y + 4.0);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let painter = ui.painter();
        let name_at = rect.left_center() - vec2(0.0, name.size().y / 2.0);
        let baseline = name_at.y + name.size().y - 3.0;
        let from = name_at.x + name.size().x + LEADER_GAP;
        let to = rect.right() - reading.size().x - LEADER_GAP;
        if reading.rows.len() == 1 && to > from {
            painter.extend(egui::Shape::dotted_line(
                &[egui::pos2(from, baseline), egui::pos2(to, baseline)],
                colors.line,
                2.0,
                0.6,
            ));
        }
        painter.galley(name_at, name, colors.text_mid);
        painter.galley(
            rect.right_center() - vec2(0.0, reading.size().y / 2.0),
            reading,
            colors.text_high,
        );
    }
}
