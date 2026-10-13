//! How long the viewport's frames take, written to the log under the `frame_time` target
//! so a plate that lags can be measured where it lags; see `docs/design/viewport.md`.

use std::time::Duration;

use web_time::Instant;

/// A gap this long between two frames is the window resting, not a frame taking long.
const IDLE: Duration = Duration::from_secs(1);
/// How often a summary is written while frames keep coming.
const SUMMARY_EVERY: Duration = Duration::from_secs(1);
/// A frame slower than this, milliseconds, is written on its own: under 20 a second.
const SLOW_MS: f32 = 50.0;

/// How long the card took over one frame's scene pass, and the triangles it drew in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardTime {
    pub card_s: f32,
    pub faces: usize,
}

/// What one frame of the viewport cost.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameCost {
    /// The window's own pass over its panels on the frame before, milliseconds of CPU.
    pub ui_ms: f32,
    /// Preparing and recording the viewport's passes, milliseconds of CPU.
    pub prepare_ms: f32,
    /// Triangles sent to the card, every pass counted.
    pub drawn_faces: usize,
    pub draws: u32,
    /// Triangles uploaded because a mesh was new to the card or cut differently.
    pub uploaded_faces: usize,
    pub shadow_baked: bool,
    /// The card's time over the latest scene pass read back, milliseconds, a few frames
    /// behind this one; `None` on a card that cannot time a pass.
    pub card_ms: Option<f32>,
}

/// One second of frames, as it is written.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Summary {
    pub frames: u32,
    /// Time from one frame to the next, milliseconds: what the eye sees, the card included.
    pub interval_avg_ms: f32,
    pub interval_max_ms: f32,
    pub ui_avg_ms: f32,
    pub ui_max_ms: f32,
    pub prepare_avg_ms: f32,
    pub prepare_max_ms: f32,
    pub drawn_faces_max: usize,
    pub draws_max: u32,
    pub uploaded_faces: usize,
    pub shadow_bakes: u32,
    pub card_max_ms: f32,
}

/// Frames gathered a second at a time.
#[derive(Debug, Default)]
pub struct FrameLog {
    last: Option<Instant>,
    started: Option<Instant>,
    sum: Summary,
}

impl FrameLog {
    /// Takes the frame drawn at `now` and writes what is due.
    pub fn record(&mut self, now: Instant, cost: FrameCost) {
        let interval = self.last.map(|last| now.duration_since(last));
        self.last = Some(now);
        if let Some(interval) = interval.filter(|interval| *interval < IDLE) {
            let interval_ms = interval.as_secs_f32() * 1000.0;
            if interval_ms > SLOW_MS {
                tracing::debug!(
                    target: "frame_time",
                    interval_ms,
                    ui_ms = cost.ui_ms,
                    prepare_ms = cost.prepare_ms,
                    drawn_faces = cost.drawn_faces,
                    draws = cost.draws,
                    uploaded_faces = cost.uploaded_faces,
                    shadow_baked = cost.shadow_baked,
                    card_ms = cost.card_ms,
                    "slow viewport frame"
                );
            }
        }
        if let Some(summary) = self.take(now, interval, cost) {
            tracing::debug!(
                target: "frame_time",
                frames = summary.frames,
                interval_avg_ms = summary.interval_avg_ms,
                interval_max_ms = summary.interval_max_ms,
                ui_avg_ms = summary.ui_avg_ms,
                ui_max_ms = summary.ui_max_ms,
                prepare_avg_ms = summary.prepare_avg_ms,
                prepare_max_ms = summary.prepare_max_ms,
                drawn_faces_max = summary.drawn_faces_max,
                draws_max = summary.draws_max,
                uploaded_faces = summary.uploaded_faces,
                shadow_bakes = summary.shadow_bakes,
                card_max_ms = summary.card_max_ms,
                "viewport frames"
            );
        }
    }

    /// Adds the frame to the second under way, and hands that second back once it is
    /// over. A frame after a rest starts a new second rather than counting the rest.
    fn take(
        &mut self,
        now: Instant,
        interval: Option<Duration>,
        cost: FrameCost,
    ) -> Option<Summary> {
        let Some(interval) = interval.filter(|interval| *interval < IDLE) else {
            self.started = Some(now);
            self.sum = Summary::default();
            return None;
        };
        let interval_ms = interval.as_secs_f32() * 1000.0;
        let sum = &mut self.sum;
        sum.frames += 1;
        sum.interval_avg_ms += interval_ms;
        sum.interval_max_ms = sum.interval_max_ms.max(interval_ms);
        sum.ui_avg_ms += cost.ui_ms;
        sum.ui_max_ms = sum.ui_max_ms.max(cost.ui_ms);
        sum.prepare_avg_ms += cost.prepare_ms;
        sum.prepare_max_ms = sum.prepare_max_ms.max(cost.prepare_ms);
        sum.drawn_faces_max = sum.drawn_faces_max.max(cost.drawn_faces);
        sum.draws_max = sum.draws_max.max(cost.draws);
        sum.uploaded_faces += cost.uploaded_faces;
        sum.shadow_bakes += u32::from(cost.shadow_baked);
        sum.card_max_ms = sum.card_max_ms.max(cost.card_ms.unwrap_or(0.0));

        let started = *self.started.get_or_insert(now);
        if now.duration_since(started) < SUMMARY_EVERY {
            return None;
        }
        let mut summary = std::mem::take(&mut self.sum);
        let frames = summary.frames as f32;
        summary.interval_avg_ms /= frames;
        summary.ui_avg_ms /= frames;
        summary.prepare_avg_ms /= frames;
        self.started = Some(now);
        Some(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(ui_ms: f32) -> FrameCost {
        FrameCost {
            ui_ms,
            prepare_ms: 1.0,
            drawn_faces: 100,
            draws: 2,
            ..FrameCost::default()
        }
    }

    #[test]
    fn a_second_of_frames_comes_out_as_one_summary() {
        let mut log = FrameLog::default();
        let start = Instant::now();
        let mut written = Vec::new();
        // Sixty-one frames 20 ms apart: the first only starts the clock.
        for frame in 0..=60u32 {
            let now = start + Duration::from_millis(u64::from(frame) * 20);
            let interval = log.last.map(|last| now.duration_since(last));
            log.last = Some(now);
            written.extend(log.take(now, interval, cost(frame as f32 % 4.0)));
        }
        assert_eq!(written.len(), 1, "a summary a second");
        let summary = written[0];
        assert_eq!(summary.frames, 50, "a second at 20 ms a frame");
        assert!((summary.interval_avg_ms - 20.0).abs() < 0.01);
        assert!((summary.ui_max_ms - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_rest_between_frames_is_not_counted_as_a_slow_frame() {
        let mut log = FrameLog::default();
        let start = Instant::now();
        assert_eq!(log.take(start, None, cost(1.0)), None);
        let after_rest = start + Duration::from_secs(5);
        assert_eq!(
            log.take(after_rest, Some(Duration::from_secs(5)), cost(1.0)),
            None
        );
        assert_eq!(log.sum.frames, 0, "the rest starts a new second");
    }
}
