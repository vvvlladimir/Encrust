//! How many faces this machine draws in the time a frame is given, learnt from the frames
//! themselves: nothing is drawn lighter until a frame shows it has to be.

use std::sync::mpsc::{self, Receiver, Sender};

use crate::render::frame_time::CardTime;

/// What the card is given for the scene while the camera moves, seconds: a 60 Hz frame
/// less the window's own pass and presenting it.
const MOVING_S: f64 = 0.012;
/// And while it rests: thirty frames a second keeps up with a pointer passing over it.
const RESTING_S: f64 = 0.030;
/// How much of what was learnt each new pass keeps: a pass dearer than foreseen by
/// `DEARER` is believed at once, a cheaper one only over a second or so of frames.
const KEEP_CHEAPER: f64 = 0.97;
const KEEP_DEARER: f64 = 0.02;
const DEARER: f64 = 1.25;
/// Below this spread of face counts, as a share of their mean, the passes cannot tell
/// what a face costs from what the frame costs anyway, and all of it is put on the faces.
const ONE_LOAD: f64 = 0.1;

/// The frame time the camera is meant to keep up at without the card's clock, seconds.
const FRAME_S: f32 = 1.0 / 60.0;
/// A frame slower than this while the camera moves counts against the card, seconds: past
/// a display's own interval, and a frame or two of jitter, but not a hand at rest.
const SLOW_S: f32 = 0.022;
const PAUSE_S: f32 = 0.2;
/// Slow frames in a row before the budget is cut, so one hitch does not set it.
const SLOW_RUN: u32 = 3;
/// How far under what kept up the budget is set, so the next frame does not sit on the edge.
const MARGIN: f32 = 0.85;

/// Faces the card can draw in a frame's time. Where it times its own passes this follows
/// a line through them, a fixed cost and a cost a face, down and back up; elsewhere it is
/// cut by runs of slow frames while the camera moves, and never raised.
#[derive(Debug)]
pub struct Budget {
    fit: Fit,
    paced: Option<usize>,
    slow: u32,
    times: Receiver<CardTime>,
    report: Sender<CardTime>,
}

impl Default for Budget {
    fn default() -> Self {
        let (report, times) = mpsc::channel();
        Self {
            fit: Fit::default(),
            paced: None,
            slow: 0,
            times,
            report,
        }
    }
}

impl Budget {
    /// Where the viewport sends what the card took over each frame.
    pub fn report(&self) -> Sender<CardTime> {
        self.report.clone()
    }

    /// Takes in the card's times sent since the last frame.
    pub fn take_times(&mut self) {
        while let Ok(time) = self.times.try_recv() {
            self.fit.add(time);
        }
    }

    /// Takes one frame drawn while the camera moved: `dt_s` after the one before it, which
    /// drew `faces` faces. Only a card that is not timed is judged by it.
    pub fn observe(&mut self, dt_s: f32, faces: usize) {
        if self.fit.weight > 0.0 || dt_s >= PAUSE_S {
            return;
        }
        if dt_s < SLOW_S {
            self.slow = 0;
            return;
        }
        self.slow += 1;
        if self.slow < SLOW_RUN {
            return;
        }
        self.slow = 0;
        let kept_up = (faces as f32 * FRAME_S / dt_s * MARGIN) as usize;
        self.paced = Some(self.paced.map_or(kept_up, |faces| faces.min(kept_up)));
    }

    /// Faces a frame may draw while the camera moves, or `None` while nothing says the
    /// card cannot draw everything.
    pub fn moving(&self) -> Option<usize> {
        self.fit.faces(MOVING_S).or(self.paced)
    }

    /// Faces a frame may draw while the camera rests, known only on a timed card.
    pub fn resting(&self) -> Option<usize> {
        self.fit.faces(RESTING_S)
    }
}

/// A least-squares line through the timed passes, card seconds against millions of faces,
/// the older passes weighing less.
#[derive(Debug, Default, Clone, Copy)]
struct Fit {
    weight: f64,
    x: f64,
    y: f64,
    xx: f64,
    xy: f64,
}

impl Fit {
    fn add(&mut self, time: CardTime) {
        let (x, y) = (time.faces as f64 / 1e6, f64::from(time.card_s));
        let dearer = self
            .line()
            .is_some_and(|(fixed_s, face_s)| y > (fixed_s + face_s * x) * DEARER);
        let keep = if dearer { KEEP_DEARER } else { KEEP_CHEAPER };
        *self = Self {
            weight: self.weight * keep + 1.0,
            x: self.x * keep + x,
            y: self.y * keep + y,
            xx: self.xx * keep + x * x,
            xy: self.xy * keep + x * y,
        };
    }

    /// The fixed cost of a pass and the cost of a million faces, seconds.
    fn line(&self) -> Option<(f64, f64)> {
        if self.weight <= 0.0 || self.x <= 0.0 {
            return None;
        }
        let (mean_x, mean_y) = (self.x / self.weight, self.y / self.weight);
        let spread = self.xx / self.weight - mean_x * mean_x;
        let all_on_faces = (0.0, mean_y / mean_x);
        if spread <= (ONE_LOAD * mean_x).powi(2) {
            return Some(all_on_faces);
        }
        let face_s = (self.xy / self.weight - mean_x * mean_y) / spread;
        let fixed_s = mean_y - face_s * mean_x;
        if face_s <= 0.0 || fixed_s < 0.0 {
            return Some(all_on_faces);
        }
        Some((fixed_s, face_s))
    }

    fn faces(&self, frame_s: f64) -> Option<usize> {
        let (fixed_s, face_s) = self.line()?;
        Some(((frame_s - fixed_s).max(0.0) / face_s * 1e6) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed(card_s: f32, faces: usize) -> CardTime {
        CardTime { card_s, faces }
    }

    #[test]
    fn nothing_is_budgeted_before_a_frame_is_seen() {
        let budget = Budget::default();
        assert_eq!((budget.moving(), budget.resting()), (None, None));
    }

    #[test]
    fn the_frames_own_cost_is_told_from_the_cost_of_its_faces() {
        let mut budget = Budget::default();
        // 5 ms a frame and 1.25 ns a face, the plate of sixteen cones measured.
        for _ in 0..20 {
            budget
                .fit
                .add(timed(0.005 + 1.25e-9 * 18_000_000.0, 18_000_000));
            budget
                .fit
                .add(timed(0.005 + 1.25e-9 * 2_000_000.0, 2_000_000));
        }
        let faces = budget.moving().expect("timed");
        assert!(
            (5_400_000..5_800_000).contains(&faces),
            "7 ms left after the frame's own 5 ms is 5.6 million faces: {faces}"
        );
        let resting = budget.resting().expect("timed");
        assert!(
            (19_000_000..21_000_000).contains(&resting),
            "25 ms at rest is 20 million: {resting}"
        );
    }

    #[test]
    fn one_load_puts_all_its_cost_on_the_faces() {
        let mut budget = Budget::default();
        budget.fit.add(timed(0.024, 12_000_000));
        let faces = budget.moving().expect("timed");
        assert!(
            (5_900_000..6_100_000).contains(&faces),
            "2 ns a face makes 12 ms six million faces: {faces}"
        );
    }

    #[test]
    fn a_dearer_card_is_believed_at_once_and_a_cheaper_one_slowly() {
        let mut budget = Budget::default();
        for _ in 0..50 {
            budget.fit.add(timed(0.006, 18_000_000));
        }
        let calm = budget.moving().expect("timed");
        budget.fit.add(timed(0.060, 18_000_000));
        let busy = budget.moving().expect("timed");
        assert!(
            busy < calm / 2,
            "one pass ten times dearer: {busy} of {calm}"
        );
        budget.fit.add(timed(0.006, 18_000_000));
        let after = budget.moving().expect("timed");
        assert!(
            after < calm * 3 / 4,
            "one cheap pass does not undo it: {after} of {calm}"
        );
        for _ in 0..300 {
            budget.fit.add(timed(0.006, 18_000_000));
        }
        let back = budget.moving().expect("timed");
        assert!(
            back * 100 > calm * 99,
            "a run of them does: {back} of {calm}"
        );
    }

    #[test]
    fn times_sent_to_the_budget_land_when_taken() {
        let mut budget = Budget::default();
        budget
            .report()
            .send(timed(0.024, 12_000_000))
            .expect("the budget holds its own receiver");
        assert_eq!(budget.moving(), None, "nothing is taken in until asked");
        budget.take_times();
        assert!(budget.moving().is_some());
    }

    #[test]
    fn an_untimed_card_is_cut_by_a_run_of_slow_frames() {
        let mut budget = Budget::default();
        budget.observe(1.0 / 30.0, 18_000_000);
        budget.observe(1.0 / 30.0, 18_000_000);
        assert_eq!(budget.moving(), None, "two slow frames are a hitch");
        budget.observe(1.0 / 30.0, 18_000_000);
        let faces = budget.moving().expect("three in a row are the card");
        assert!(
            (7_000_000..9_000_000).contains(&faces),
            "half the faces kept up at twice the rate, less the margin: {faces}"
        );
        assert_eq!(
            budget.resting(),
            None,
            "and nothing is known of a resting frame"
        );
    }

    #[test]
    fn a_hand_at_rest_is_not_a_slow_frame() {
        let mut budget = Budget::default();
        for _ in 0..10 {
            budget.observe(0.5, 18_000_000);
        }
        assert_eq!(budget.moving(), None);
    }

    #[test]
    fn a_timed_card_is_not_judged_by_its_frame_intervals() {
        let mut budget = Budget::default();
        budget.fit.add(timed(0.006, 18_000_000));
        let timed_faces = budget.moving();
        for _ in 0..10 {
            budget.observe(1.0 / 20.0, 18_000_000);
        }
        assert_eq!(
            budget.moving(),
            timed_faces,
            "a slow interval is the window's"
        );
    }
}
