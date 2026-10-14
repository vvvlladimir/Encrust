use core_geometry::{Scalar, Vec3};

use crate::orient::OrientSettings;
use crate::score::{Faces, Score};

/// How far over the best a candidate's own term may be and still be in the running: a
/// tenth, and a sliver more for a term that is zero at best.
const BAND: Scalar = 0.10;
const SLIVER: Scalar = 0.002;

/// The share of its total a turn has to win to be worth taking over standing still.
const GAIN: Scalar = 0.05;

/// One way down, measured by both passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub down: Vec3,
    pub score: Score,
}

/// Picks between measured candidates by priority rather than by the total alone: first
/// what would fail the print, then the goal's own term within `BAND` of the best, and only
/// then the total. See `docs/design/orientation.md`.
pub struct Judge<'a> {
    faces: &'a Faces<'a>,
    settings: &'a OrientSettings,
    fewest_failures: u8,
    best_primary: Scalar,
}

impl<'a> Judge<'a> {
    /// The judge of `measured`, whose best decides the band everything is held to.
    pub fn over(
        faces: &'a Faces<'a>,
        settings: &'a OrientSettings,
        measured: &[Candidate],
    ) -> Option<Self> {
        let fewest_failures = measured
            .iter()
            .map(|candidate| failures(&candidate.score, settings))
            .min()?;
        let best_primary = measured
            .iter()
            .filter(|candidate| failures(&candidate.score, settings) == fewest_failures)
            .map(|candidate| faces.primary(&candidate.score))
            .fold(Scalar::INFINITY, Scalar::min);
        Some(Self {
            faces,
            settings,
            fewest_failures,
            best_primary,
        })
    }

    /// Whether `a` is the better of the two.
    pub fn beats(&self, a: &Candidate, b: &Candidate) -> bool {
        let rank = |candidate: &Candidate| {
            let failing = failures(&candidate.score, self.settings);
            (
                failing,
                failing > self.fewest_failures || !self.in_band(candidate),
            )
        };
        match rank(a).cmp(&rank(b)) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => a.score.total < b.score.total,
        }
    }

    /// Whether `turned` is worth taking over `now`, the way the model stands: it has to
    /// fail less, or win `GAIN` of the total, so a model standing nearly right stays put.
    pub fn worth_turning(&self, turned: &Candidate, now: &Candidate) -> bool {
        if !self.beats(turned, now) {
            return false;
        }
        let fails_less =
            failures(&turned.score, self.settings) < failures(&now.score, self.settings);
        let gain = now.score.total - turned.score.total;
        fails_less || !self.in_band(now) || gain > GAIN * now.score.total.abs()
    }

    fn in_band(&self, candidate: &Candidate) -> bool {
        let limit = self.best_primary + BAND * self.best_primary.abs() + SLIVER;
        self.faces.primary(&candidate.score) <= limit
    }
}

/// What would fail the print outright, worse first: a suction cup counts four, a pull past
/// the limit two, and a model set on the plate with no face flat on it, which only
/// supports could hold there, one.
fn failures(score: &Score, settings: &OrientSettings) -> u8 {
    let cup = score.cup_mm2.unwrap_or(0.0) > 0.0;
    let tearing = settings
        .peel_limit_mm4
        .is_some_and(|limit| score.peak_pull_mm4.unwrap_or(0.0) > limit);
    let unseated = !settings.on_supports && score.bottom_mm2 <= 0.0;
    u8::from(cup) * 4 + u8::from(tearing) * 2 + u8::from(unseated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Mesh;

    fn cube() -> Mesh {
        let corners = (0..8)
            .map(|at| Vec3::new((at & 1) as f32, ((at >> 1) & 1) as f32, (at >> 2) as f32) * 10.0)
            .collect();
        Mesh::new(
            corners,
            vec![
                [0, 2, 1],
                [1, 2, 3],
                [4, 5, 6],
                [5, 7, 6],
                [0, 1, 4],
                [1, 5, 4],
                [2, 6, 3],
                [3, 6, 7],
                [0, 4, 2],
                [2, 4, 6],
                [1, 3, 5],
                [3, 7, 5],
            ],
        )
    }

    /// A measured candidate costing `overhang_mm2` of support and `total` in all.
    fn candidate(overhang_mm2: Scalar, total: Scalar, cup_mm2: Scalar) -> Candidate {
        Candidate {
            down: Vec3::NEG_Z,
            score: Score {
                overhang_mm2,
                low_points: 0,
                support_mm3: 0.0,
                bottom_mm2: 0.0,
                height_mm: 10.0,
                shadow_mm2: 100.0,
                peak_mm2: Some(100.0),
                peak_pull_mm4: Some(1000.0),
                top_overhang_mm2: 0.0,
                cup_mm2: Some(cup_mm2),
                total,
            },
        }
    }

    #[test]
    fn a_suction_cup_loses_whatever_it_scores() {
        let mesh = cube();
        let faces = Faces::of(&mesh).expect("a cube has faces");
        let settings = OrientSettings::default();
        let cupping = candidate(0.0, 0.1, 400.0);
        let clean = candidate(50.0, 9.0, 0.0);
        let judge = Judge::over(&faces, &settings, &[cupping, clean]).expect("two candidates");
        assert!(judge.beats(&clean, &cupping));
    }

    #[test]
    fn the_goals_own_term_decides_before_the_total() {
        let mesh = cube();
        let faces = Faces::of(&mesh).expect("a cube has faces");
        let settings = OrientSettings::default();
        // Fewest supports: 10 mm2 of overhang against 60, whatever the other terms add.
        let few = candidate(10.0, 5.0, 0.0);
        let many = candidate(60.0, 1.0, 0.0);
        let judge = Judge::over(&faces, &settings, &[few, many]).expect("two candidates");
        assert!(judge.beats(&few, &many));

        // Within a tenth of the best, the total breaks the tie.
        let close = candidate(10.5, 4.0, 0.0);
        assert!(judge.beats(&close, &few));
    }

    #[test]
    fn a_turn_winning_less_than_its_gain_is_not_worth_making() {
        let mesh = cube();
        let faces = Faces::of(&mesh).expect("a cube has faces");
        let settings = OrientSettings::default();
        let now = candidate(10.0, 1.00, 0.0);
        let slightly = candidate(10.0, 0.98, 0.0);
        let clearly = candidate(10.0, 0.80, 0.0);
        let judge = Judge::over(&faces, &settings, &[now, slightly, clearly]).expect("three");
        assert!(
            !judge.worth_turning(&slightly, &now),
            "2 % is not worth a turn"
        );
        assert!(judge.worth_turning(&clearly, &now), "20 % is");
    }
}
