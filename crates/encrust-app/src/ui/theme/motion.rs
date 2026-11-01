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

/// How far `id` has moved towards `on`, 0 to 1, by one motion both ways.
pub fn toggle(ctx: &Context, id: Id, on: bool, motion: Motion) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, motion.seconds(ctx), motion.easing)
}

/// How far `id` is open, 0 to 1: coming in by `OPEN`, going away by `CLOSE`.
pub fn fold(ctx: &Context, id: Id, open: bool) -> f32 {
    toggle(ctx, id, open, if open { OPEN } else { CLOSE })
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

fn reduced(ctx: &Context) -> bool {
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
