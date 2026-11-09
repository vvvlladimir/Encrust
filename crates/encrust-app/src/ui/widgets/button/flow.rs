//! Buttons sharing rows by the width of their words, wrapping where a language runs long.

use std::ops::Range;

use egui::{Response, Ui, vec2};

use super::{GLYPH_W, Skin, filled_button, label_w};
use crate::ui::theme;

/// Room a flowing button keeps around its words, both sides together.
const FLOW_PAD: f32 = 16.0;

/// How strongly the accent edges the button a row suggests.
const LEAD_EDGE: f32 = 0.55;

/// One button of a flowing row; `lead` is the answer the row suggests.
pub struct FlowButton<'a> {
    pub glyph: &'a str,
    pub text: &'a str,
    pub tip: &'a str,
    pub lead: bool,
}

/// Which of `widths` stand on each line `room` wide, `gap` apart, in order; a button wider
/// than the room has a line to itself.
fn lines(widths: &[f32], room: f32, gap: f32) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let (mut start, mut used) = (0, 0.0);
    for (index, width) in widths.iter().enumerate() {
        if index > start && used + gap + width > room {
            found.push(start..index);
            (start, used) = (index, 0.0);
        }
        used += if index > start { gap } else { 0.0 } + width;
    }
    if start < widths.len() {
        found.push(start..widths.len());
    }
    found
}

fn skin(lead: bool, enabled: bool) -> Skin {
    let colors = theme::colors();
    let plain = Skin::raised_or_off(enabled);
    match lead && enabled {
        true => Skin {
            foreground: colors.accent_soft,
            border: Some(colors.accent.gamma_multiply(LEAD_EDGE)),
            ..plain
        },
        false => plain,
    }
}

fn press(ui: &mut Ui, button: &FlowButton<'_>, width: f32, enabled: bool) -> Response {
    let size = vec2(width, theme::BUTTON_H);
    let look = skin(button.lead, enabled);
    filled_button(ui, button.glyph, button.text, size, &look, enabled).on_hover_text(button.tip)
}

/// `buttons` over as many rows as their words need, each row filling the width, greyed while
/// `enabled` is false. Answers which was pressed.
pub fn flow_buttons(ui: &mut Ui, enabled: bool, buttons: &[FlowButton<'_>]) -> Option<usize> {
    let widths: Vec<f32> = buttons
        .iter()
        .map(|button| label_w(ui, button.text) + GLYPH_W + FLOW_PAD)
        .collect();
    let room = ui.available_width();
    let gap = theme::BUTTON_GAP;
    let mut pressed = None;
    for line in lines(&widths, room, gap) {
        let used: f32 = widths[line.clone()].iter().sum();
        let count = line.len() as f32;
        let spare = ((room - used - gap * (count - 1.0)) / count).max(0.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for index in line {
                let width = (widths[index] + spare).min(room);
                if press(ui, &buttons[index], width, enabled).clicked() {
                    pressed = Some(index);
                }
            }
        });
    }
    pressed
}

#[cfg(test)]
mod tests {
    use super::lines;

    #[test]
    fn buttons_that_fit_share_a_line() {
        assert_eq!(lines(&[100.0, 100.0], 206.0, 6.0), vec![0..2]);
    }

    #[test]
    fn a_button_past_the_room_starts_the_next_line() {
        assert_eq!(lines(&[100.0, 100.0, 100.0], 206.0, 6.0), vec![0..2, 2..3]);
    }

    #[test]
    fn a_button_wider_than_the_room_has_a_line_to_itself() {
        assert_eq!(lines(&[300.0, 50.0], 206.0, 6.0), vec![0..1, 1..2]);
    }

    #[test]
    fn no_buttons_make_no_lines() {
        assert!(lines(&[], 206.0, 6.0).is_empty());
    }
}
