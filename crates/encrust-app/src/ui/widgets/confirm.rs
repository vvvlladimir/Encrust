//! An action undo cannot take back asks by a second press: the first arms the control for
//! `motion::ARMED_S`, the second acts, and Esc, a click elsewhere or the time disarms it.

use egui::{Id, Key, Modifiers, Response, Ui, WidgetInfo, WidgetType, vec2};

use super::button::{Face, GLYPH_W, Skin, paint_button, sense_for};
use crate::i18n::tr;
use crate::ui::{icon, theme, theme::motion};

/// The one control armed in the window, when it was, s of the input's clock, and the pass
/// that last drew it.
#[derive(Clone, Copy)]
struct Armed {
    id: Id,
    at_s: f64,
    drawn_pass: u64,
}

fn armed_key() -> Id {
    Id::new("confirm-armed")
}

/// What a press on `response` did: whether it stands armed now, and whether it acted.
pub(super) struct Arming {
    pub armed: bool,
    pub fired: bool,
}

/// Arms `response` on its first press and answers the second.
pub(super) fn arm(ui: &Ui, response: &Response, enabled: bool) -> Arming {
    let now = ui.input(|input| input.time);
    let mine = ui
        .data(|data| data.get_temp::<Armed>(armed_key()))
        .filter(|armed| armed.id == response.id);
    let mut fired = false;
    if let Some(armed) = mine {
        let timed_out = now - armed.at_s > motion::ARMED_S;
        let escaped = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        fired = enabled && !timed_out && response.clicked();
        if fired || timed_out || escaped || response.clicked_elsewhere() {
            ui.data_mut(|data| data.remove::<Armed>(armed_key()));
        } else {
            let drawn = Armed {
                drawn_pass: ui.ctx().cumulative_pass_nr(),
                ..armed
            };
            ui.data_mut(|data| data.insert_temp(armed_key(), drawn));
            let left_s = (motion::ARMED_S - (now - armed.at_s)).max(0.0);
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(left_s));
        }
    } else if enabled && response.clicked() {
        let armed = Armed {
            id: response.id,
            at_s: now,
            drawn_pass: ui.ctx().cumulative_pass_nr(),
        };
        ui.data_mut(|data| data.insert_temp(armed_key(), armed));
        let wait = std::time::Duration::from_secs_f64(motion::ARMED_S);
        ui.ctx().request_repaint_after(wait);
    }
    let armed = ui
        .data(|data| data.get_temp::<Armed>(armed_key()))
        .is_some_and(|armed| armed.id == response.id);
    Arming { armed, fired }
}

/// Whether the control `id` stands armed, so a row draws it though the pointer has left.
pub fn is_armed(ctx: &egui::Context, id: Id) -> bool {
    ctx.data(|data| data.get_temp::<Armed>(armed_key()))
        .is_some_and(|armed| armed.id == id)
}

/// Whether a control drawn last pass stands armed and still waiting, so Esc is left to it
/// this frame.
pub fn any_armed(ctx: &egui::Context) -> bool {
    let now = ctx.input(|input| input.time);
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|data| data.get_temp::<Armed>(armed_key()))
        .is_some_and(|armed| now - armed.at_s <= motion::ARMED_S && armed.drawn_pass + 1 >= pass)
}

/// What a `confirm_button` says and does.
pub struct Confirm<'a> {
    pub glyph: &'a str,
    pub text: &'a str,
    /// What it asks while armed: *Delete?*
    pub asked: &'a str,
    /// What the action costs, or why it cannot be taken; read on hover.
    pub tooltip: &'a str,
    pub enabled: bool,
}

/// A lesser action of a row that cannot be undone, as wide as what it says: `text` with
/// its `glyph` at rest, armed `asked` on the danger's wash with a question mark. Answers
/// the second press.
pub fn confirm_button(ui: &mut Ui, confirm: &Confirm<'_>) -> bool {
    let size = vec2(widest(ui, confirm) + GLYPH_W + 24.0, theme::BUTTON_H);
    let (_, response) = ui.allocate_exact_size(size, sense_for(confirm.enabled));
    let Arming { armed, fired } = arm(ui, &response, confirm.enabled);
    let (face, skin) = face_and_skin(ui, confirm, armed);
    let label = face.text.to_owned();
    let enabled = confirm.enabled;
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &label));
    paint_button(ui, &response, &face, &skin, enabled);
    let tooltip = confirm.tooltip;
    let response = match tooltip.is_empty() {
        true => response,
        false => response
            .on_hover_text(tooltip)
            .on_disabled_hover_text(tooltip),
    };
    if armed {
        response.on_hover_text(tr!("confirm-click-again"));
    }
    fired
}

/// The wider of what the button says at rest and armed, so arming never moves its row.
fn widest(ui: &Ui, confirm: &Confirm<'_>) -> f32 {
    let colors = theme::colors();
    [confirm.text, confirm.asked]
        .iter()
        .map(|words| {
            let font = theme::label();
            let galley = ui
                .painter()
                .layout_no_wrap((*words).to_owned(), font, colors.text_high);
            galley.size().x
        })
        .fold(0.0, f32::max)
}

/// What the button says and how it is painted: the question on the danger's wash while
/// armed, its glyph and words in the danger at rest.
fn face_and_skin<'a>(ui: &Ui, confirm: &Confirm<'a>, armed: bool) -> (Face<'a>, Skin) {
    let colors = theme::colors();
    if armed {
        let face = Face {
            glyph: icon::ARMED,
            text: confirm.asked,
            font: theme::label(),
        };
        let skin = Skin {
            fill: colors.danger_wash,
            foreground: colors.danger,
            border: Some(armed_ring(ui.ctx())),
            hovered: colors.danger_wash,
            pressed: colors.danger_wash,
        };
        return (face, skin);
    }
    let face = Face {
        glyph: confirm.glyph,
        text: confirm.text,
        font: theme::label(),
    };
    let skin = Skin {
        fill: egui::Color32::TRANSPARENT,
        foreground: if confirm.enabled {
            colors.danger
        } else {
            colors.text_low
        },
        border: None,
        hovered: colors.danger_wash,
        pressed: colors.active,
    };
    (face, skin)
}

/// How much of the danger an armed control's ring keeps at the top of its pulse, and at
/// the bottom.
const ARMED_RING: f32 = 0.55;
const ARMED_RING_LOW: f32 = 0.2;

/// The ring of an armed control, pulsing while it waits.
pub(super) fn armed_ring(ctx: &egui::Context) -> egui::Color32 {
    let strength = ARMED_RING_LOW + (ARMED_RING - ARMED_RING_LOW) * motion::pulse(ctx);
    theme::colors().danger.gamma_multiply(strength)
}

#[cfg(test)]
mod tests {
    use egui::{Event, PointerButton, Pos2, RawInput};

    use super::*;
    use crate::ui::widgets::fixtures;

    /// Runs one frame of a 40 × 32 control at the window's corner, as `input` presses it.
    fn frame(ctx: &egui::Context, input: RawInput) -> Arming {
        let mut arming = None;
        fixtures::frame(ctx, input, |ui| {
            let (_, response) = ui.allocate_exact_size(vec2(40.0, 32.0), egui::Sense::click());
            arming = Some(arm(ui, &response, true));
        });
        arming.expect("the area draws its control every frame")
    }

    fn click_at(at: Pos2, time: f64) -> Vec<RawInput> {
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let input = |events, time| RawInput {
            events,
            time: Some(time),
            ..RawInput::default()
        };
        vec![
            input(vec![Event::PointerMoved(at)], time),
            input(vec![press(true)], time),
            input(vec![press(false)], time),
        ]
    }

    fn click(ctx: &egui::Context, at: Pos2, time: f64) -> Arming {
        // Every frame runs, in order: the last says how the click left the control.
        let mut frames: Vec<Arming> = click_at(at, time)
            .into_iter()
            .map(|input| frame(ctx, input))
            .collect();
        frames.pop().expect("a click is three frames")
    }

    const ON: Pos2 = Pos2::new(10.0, 10.0);

    #[test]
    fn the_first_press_arms_and_the_second_acts() {
        let ctx = fixtures::context();
        let first = click(&ctx, ON, 0.0);
        assert!(first.armed && !first.fired);
        let second = click(&ctx, ON, 1.0);
        assert!(second.fired && !second.armed);
    }

    #[test]
    fn a_press_after_the_wait_arms_again_rather_than_acting() {
        let ctx = fixtures::context();
        click(&ctx, ON, 0.0);
        let late = click(&ctx, ON, motion::ARMED_S + 1.0);
        assert!(!late.fired, "the wait ran out, so the press only disarms");
    }

    #[test]
    fn an_armed_control_holds_escape_until_its_wait_runs_out() {
        let ctx = fixtures::context();
        assert!(!any_armed(&ctx));
        click(&ctx, ON, 0.0);
        assert!(
            any_armed(&ctx),
            "Esc is the armed control's, not the window's"
        );
        let late = RawInput {
            time: Some(motion::ARMED_S + 1.0),
            ..RawInput::default()
        };
        fixtures::frame(&ctx, late, |_| {});
        assert!(
            !any_armed(&ctx),
            "a control not drawn since cannot hold Esc for ever"
        );
    }

    #[test]
    fn an_armed_control_no_longer_drawn_leaves_escape_to_the_window() {
        let ctx = fixtures::context();
        click(&ctx, ON, 0.0);
        let soon = RawInput {
            time: Some(0.5),
            ..RawInput::default()
        };
        fixtures::frame(&ctx, soon, |_| {});
        assert!(!any_armed(&ctx), "its panel closed while it was armed");
    }

    #[test]
    fn a_click_elsewhere_disarms() {
        let ctx = fixtures::context();
        click(&ctx, ON, 0.0);
        let after = click(&ctx, Pos2::new(300.0, 300.0), 0.5);
        assert!(!after.armed && !after.fired);
    }

    #[test]
    fn escape_disarms() {
        let ctx = fixtures::context();
        click(&ctx, ON, 0.0);
        let escape = RawInput {
            events: vec![Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            time: Some(0.5),
            ..RawInput::default()
        };
        assert!(!frame(&ctx, escape).armed);
    }
}
