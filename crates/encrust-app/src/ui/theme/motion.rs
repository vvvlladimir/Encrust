//! Every duration and curve the window moves by, and the one switch that stops them all.
#![expect(clippy::disallowed_methods, reason = "every motion is timed here")]

use egui::{Context, Id, emath::easing};

/// One way of moving: how long it takes and how it eases.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    seconds: f32,
    easing: fn(f32) -> f32,
}

/// Hover, press, a switch's knob, a tab's underline.
pub const FAST: Motion = Motion {
    seconds: 0.12,
    easing: easing::cubic_out,
};
/// A fold, a window, a notice or a menu coming in.
pub const OPEN: Motion = Motion {
    seconds: 0.24,
    easing: ease_out,
};
/// The same going away, faster than it came.
pub const CLOSE: Motion = Motion {
    seconds: 0.17,
    easing: easing::cubic_in,
};
/// How long an armed `confirm_button` waits for its second press, s. A wait rather than a
/// motion, so *Reduce motion* leaves it.
pub const ARMED_S: f64 = 3.0;
/// The loops, s a round: a job's glyph turning, the sheen crossing a bar, a busy printer's
/// dot breathing, an armed control's ring pulsing.
const SPIN_S: f64 = 0.8;
const SHEEN_S: f64 = 1.4;
const BREATHE_S: f64 = 1.6;
const PULSE_S: f64 = 1.0;
/// The camera swinging to a face of the view cube, s.
const TURN_S: f64 = 0.35;
/// How far a window over the plate rises as it comes in, points.
pub const LIFT_PT: f32 = 6.0;

impl Motion {
    /// How long this takes, s, or nought while *Reduce motion* is on.
    pub fn seconds(self, ctx: &Context) -> f32 {
        if reduced(ctx) { 0.0 } else { self.seconds }
    }

    /// How long this takes with *Reduce motion* off, s.
    pub(super) const fn unreduced_s(self) -> f32 {
        self.seconds
    }
}

/// How far `id` has moved towards `on`, 0 to 1, by one motion both ways. Held for the
/// frame: a pass laid out again must find everything as tall as the one before it.
pub fn toggle(ctx: &Context, id: Id, on: bool, motion: Motion) -> f32 {
    let frame = ctx.cumulative_frame_nr();
    let held = id.with("this-frame");
    if let Some((at, towards, value)) = ctx.data(|data| data.get_temp::<(u64, bool, f32)>(held))
        && (at, towards) == (frame, on)
    {
        return value;
    }
    let value = ctx.animate_bool_with_time_and_easing(id, on, motion.seconds(ctx), motion.easing);
    ctx.data_mut(|data| data.insert_temp(held, (frame, on, value)));
    value
}

/// How far `id` is open, 0 to 1: coming in by `OPEN`, going away by `CLOSE`.
pub fn fold(ctx: &Context, id: Id, open: bool) -> f32 {
    toggle(ctx, id, open, if open { OPEN } else { CLOSE })
}

/// How far `id` has come in, 0 to 1, by `OPEN`: from nought on the first frame it is
/// drawn after a frame it was not, so a card that comes back comes in again.
pub fn appear(ctx: &Context, id: Id) -> f32 {
    let pass = ctx.cumulative_pass_nr();
    let was = ctx.data(|data| data.get_temp::<u64>(id.with("before")));
    ctx.data_mut(|data| data.insert_temp(id.with("before"), pass));
    if was.is_none_or(|before| before + 1 < pass) {
        toggle(ctx, id, false, OPEN);
        ctx.request_repaint();
        return 0.0;
    }
    fold(ctx, id, true)
}

/// How far round a job's turning glyph is, 0 to 1; `None` under *Reduce motion*.
pub fn spin(ctx: &Context) -> Option<f32> {
    looping(ctx, SPIN_S)
}

/// Where the sheen crossing a bar stands, 0 to 1; `None` under *Reduce motion*.
pub fn sheen(ctx: &Context) -> Option<f32> {
    looping(ctx, SHEEN_S)
}

/// How bright a busy printer's dot is, 0 to 1 and back; full under *Reduce motion*.
pub fn breathe(ctx: &Context) -> f32 {
    looping(ctx, BREATHE_S).map_or(1.0, wave)
}

/// How strong an armed control's ring is, 0 to 1 and back; full under *Reduce motion*.
pub fn pulse(ctx: &Context) -> f32 {
    looping(ctx, PULSE_S).map_or(1.0, wave)
}

/// How long the camera takes to swing to a face of the view cube, s; nought under
/// *Reduce motion*.
pub fn turn_s(ctx: &Context) -> f64 {
    if reduced(ctx) { 0.0 } else { TURN_S }
}

/// A loop's place, 0 to 1, as a rise and fall from 1 down to 0 and back.
fn wave(phase: f32) -> f32 {
    0.5 + 0.5 * (phase * std::f32::consts::TAU).cos()
}

/// Where a loop of `period_s` stands, 0 to 1, asking for the next frame; `None` under
/// *Reduce motion*, which keeps everything still.
fn looping(ctx: &Context, period_s: f64) -> Option<f32> {
    if reduced(ctx) {
        return None;
    }
    ctx.request_repaint();
    let now = ctx.input(|input| input.time);
    Some((now.rem_euclid(period_s) / period_s) as f32)
}

/// Stops every motion while `on`, egui's own included. Called each frame with the
/// preference.
pub fn reduce(ctx: &Context, on: bool) {
    if reduced(ctx) != on {
        ctx.data_mut(|data| data.insert_temp(reduced_id(), on));
        let seconds = FAST.seconds(ctx);
        ctx.all_styles_mut(|style| style.animation_time = seconds);
    }
}

/// Whether *Reduce motion* is on, for a motion whose length is worked out elsewhere, such as
/// the camera's flight to a point.
pub fn reduced(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp(reduced_id()))
        .unwrap_or(false)
}

fn reduced_id() -> Id {
    Id::new("reduce-motion")
}

/// The curve `cubic-bezier(.2, .8, .2, 1)`, as the design system names it.
fn ease_out(x: f32) -> f32 {
    let bezier = |p1: f32, p2: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
    };
    // x(t) rises monotonically, so 20 halvings pin t to within 1e-6.
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..20 {
        let mid = 0.5 * (low + high);
        if bezier(0.2, 0.2, mid) < x {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier(0.8, 1.0, 0.5 * (low + high))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Motion; 3] = [FAST, OPEN, CLOSE];

    #[test]
    fn reduce_motion_sets_every_duration_to_nought() {
        let ctx = Context::default();
        assert!(ALL.iter().all(|motion| motion.seconds(&ctx) > 0.0));
        reduce(&ctx, true);
        assert!(ALL.iter().all(|motion| motion.seconds(&ctx) == 0.0));
        assert!(
            ctx.global_style().animation_time == 0.0,
            "egui's own animations stop too"
        );
        reduce(&ctx, false);
        assert!((ctx.global_style().animation_time - FAST.seconds).abs() < 1e-6);
    }

    #[test]
    fn under_reduce_motion_a_fold_lands_in_one_frame() {
        let ctx = Context::default();
        reduce(&ctx, true);
        let id = Id::new("fold");
        assert!(fold(&ctx, id, false) == 0.0);
        assert!(fold(&ctx, id, true) == 1.0);
    }

    #[test]
    fn a_card_comes_in_from_nought_and_again_after_it_was_gone() {
        let ctx = Context::default();
        let id = Id::new("notice");
        let mut shown = Vec::new();
        for drawn in [true, true, false, true] {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                if drawn {
                    shown.push(appear(ui.ctx(), id));
                }
            });
            // No renderer takes the font atlas here.
            output.textures_delta.clear();
        }
        assert_eq!(shown[0], 0.0, "the first frame starts it from nought");
        assert_eq!(shown[2], 0.0, "a frame without it, and it comes in again");
    }

    #[test]
    fn under_reduce_motion_nothing_loops() {
        let ctx = Context::default();
        assert!(spin(&ctx).is_some() && sheen(&ctx).is_some());
        assert!(turn_s(&ctx) > 0.0);
        reduce(&ctx, true);
        assert!(spin(&ctx).is_none() && sheen(&ctx).is_none());
        assert_eq!((breathe(&ctx), pulse(&ctx)), (1.0, 1.0), "held at full");
        assert_eq!(turn_s(&ctx), 0.0);
    }

    #[test]
    fn a_wave_falls_from_full_to_nothing_and_rises_again() {
        assert!((wave(0.0) - 1.0).abs() < 1e-6);
        assert!(wave(0.5).abs() < 1e-6);
        assert!((wave(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn closing_is_faster_than_opening() {
        let ctx = Context::default();
        assert!(CLOSE.seconds(&ctx) < OPEN.seconds(&ctx));
    }

    #[test]
    fn the_open_curve_runs_from_rest_to_rest_and_never_turns_back() {
        assert!(ease_out(0.0).abs() < 1e-5, "starts at 0");
        assert!((ease_out(1.0) - 1.0).abs() < 1e-5, "ends at 1");
        let samples: Vec<f32> = (0..=100).map(|i| ease_out(i as f32 / 100.0)).collect();
        assert!(samples.windows(2).all(|pair| pair[0] <= pair[1] + 1e-6));
        assert!(
            ease_out(0.25) > 0.6,
            "an ease out covers most of the way early, got {}",
            ease_out(0.25)
        );
    }
}
