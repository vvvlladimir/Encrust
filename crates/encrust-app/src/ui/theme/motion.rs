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
// TODO(step-2): OPEN (0.24 s, cubic-bezier(.2,.8,.2,1)), CLOSE (0.17 s, cubic_in) and
// ARMED_S (3 s, kept under Reduce motion) arrive with `details_fold` and `confirm_button`.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_motion_sets_every_duration_to_nought() {
        let ctx = Context::default();
        assert!(FAST.seconds(&ctx) > 0.0);
        reduce(&ctx, true);
        assert!(FAST.seconds(&ctx) == 0.0);
        assert!(
            ctx.global_style().animation_time == 0.0,
            "egui's own animations stop too"
        );
        reduce(&ctx, false);
        assert!((ctx.global_style().animation_time - FAST.seconds).abs() < 1e-6);
    }

    #[test]
    fn under_reduce_motion_a_change_lands_in_one_frame() {
        let ctx = Context::default();
        reduce(&ctx, true);
        let id = Id::new("knob");
        assert!(toggle(&ctx, id, false, FAST) == 0.0);
        assert!(toggle(&ctx, id, true, FAST) == 1.0);
    }
}
