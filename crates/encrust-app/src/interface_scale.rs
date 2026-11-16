//! How large the window draws its interface, over the system's own scale; see
//! `docs/decisions/0453`.

use egui::{Id, Vec2, ViewportCommand, vec2};

use crate::ui::theme::{WINDOW_MIN_H, WINDOW_MIN_W};

/// The sizes offered, in percent of the system's own, smallest first.
pub const STEPS: [u16; 8] = [75, 90, 100, 110, 125, 150, 175, 200];

/// The system's own size, where a step of 0 returns.
pub const SYSTEM_PERCENT: u16 = 100;

/// The step after `percent` in the direction of `by`, held at the ends; 0 is the system's size.
pub fn step(percent: u16, by: i8) -> u16 {
    match by.signum() {
        1 => STEPS.into_iter().find(|step| *step > percent),
        -1 => STEPS.into_iter().rev().find(|step| *step < percent),
        _ => Some(SYSTEM_PERCENT),
    }
    .unwrap_or(percent)
}

/// The scale `percent` is drawn at on this screen: within the steps, never past what lets
/// the window's smallest size fit, and never below the system's own for that reason.
pub fn drawn(ctx: &egui::Context, percent: u16) -> f32 {
    let asked = f32::from(percent.clamp(STEPS[0], STEPS[STEPS.len() - 1])) / 100.0;
    let screen = ctx.data(|data| data.get_temp::<Vec2>(screen_id()));
    asked.min(screen.map_or(f32::INFINITY, fitting).max(1.0))
}

/// Draws the interface at `percent` from the next pass, and grows the window's smallest
/// size with it, so no panel is laid out narrower than it was designed for.
pub fn apply(ctx: &egui::Context, percent: u16) {
    measure_screen(ctx);
    let scale = drawn(ctx, percent);
    let now = ctx.zoom_factor();
    if (scale - now).abs() < f32::EPSILON {
        return;
    }
    ctx.set_zoom_factor(scale);
    // The command is carried out at the scale of this pass, the new one only from the next.
    let smallest = vec2(WINDOW_MIN_W, WINDOW_MIN_H) * scale / now;
    ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(smallest));
}

/// Keeps the monitor's size in points of the system's own scale. The window reports it at
/// the zoom of the pass before, which a change only reaches here one pass later.
fn measure_screen(ctx: &egui::Context) {
    let reported = ctx.input(|input| input.viewport().monitor_size);
    let now = ctx.zoom_factor();
    ctx.data_mut(|data| {
        let before = data.get_temp::<f32>(zoom_id()).unwrap_or(now);
        match reported {
            Some(size) => {
                data.insert_temp(screen_id(), size * before);
            }
            None => data.remove::<Vec2>(screen_id()),
        }
        data.insert_temp(zoom_id(), now);
    });
}

fn screen_id() -> Id {
    Id::new("interface_scale::screen")
}

fn zoom_id() -> Id {
    Id::new("interface_scale::zoom")
}

/// The largest scale at which the window's smallest size fits `screen`, in points of the
/// system's own scale.
fn fitting(screen: Vec2) -> f32 {
    (screen.x / WINDOW_MIN_W).min(screen.y / WINDOW_MIN_H)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_moves_to_the_next_size_and_holds_at_the_ends() {
        assert_eq!(step(100, 1), 110);
        assert_eq!(step(100, -1), 90);
        assert_eq!(step(200, 1), 200, "held at the largest");
        assert_eq!(step(75, -1), 75, "held at the smallest");
        assert_eq!(step(150, 0), SYSTEM_PERCENT);
    }

    #[test]
    fn a_size_between_steps_moves_to_the_nearest_one_its_way() {
        assert_eq!(step(105, 1), 110);
        assert_eq!(step(105, -1), 100);
    }

    #[test]
    fn the_window_is_scaled_only_as_far_as_its_smallest_size_fits() {
        let screen = vec2(WINDOW_MIN_W, WINDOW_MIN_H) * 1.5;
        assert!(
            (fitting(screen) - 1.5).abs() < 1e-6,
            "a screen one and a half times the window's smallest holds 1.5, got {}",
            fitting(screen)
        );
    }

    #[test]
    fn a_scale_nothing_was_said_against_is_drawn_as_asked() {
        let ctx = egui::Context::default();
        assert!((drawn(&ctx, 175) - 1.75).abs() < 1e-6);
    }

    #[test]
    fn a_stored_size_off_the_steps_is_drawn_within_them() {
        let ctx = egui::Context::default();
        assert!((drawn(&ctx, 0) - 0.75).abs() < 1e-6, "0 % draws at 75 %");
        assert!(
            (drawn(&ctx, 900) - 2.0).abs() < 1e-6,
            "900 % draws at 200 %"
        );
    }

    /// The monitor is reported at the zoom in force when the pass's input was gathered, as
    /// the window does it, so a capped scale settles instead of swinging pass after pass.
    #[test]
    fn a_capped_scale_settles_at_what_the_screen_holds() {
        let ctx = egui::Context::default();
        let screen = vec2(WINDOW_MIN_W, WINDOW_MIN_H) * 1.5;
        let mut zooms = Vec::new();
        for _ in 0..6 {
            let mut input = egui::RawInput::default();
            input
                .viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .monitor_size = Some(screen / ctx.zoom_factor());
            ctx.run_ui(input, |ui| apply(ui.ctx(), 200))
                .textures_delta
                .clear();
            zooms.push(ctx.zoom_factor());
        }
        assert!(
            zooms[1..].iter().all(|zoom| (zoom - 1.5).abs() < 1e-5),
            "held at 1.5 from the first change on, got {zooms:?}"
        );
    }
}
