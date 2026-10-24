use core_geometry::{Scalar, Vec3};

use crate::orient::OrientSettings;
use crate::policy::{Noise, Readings, Ruling, rank, rulings};
use crate::score::{Faces, OrientGoal, OrientPreset, Score};

/// One way down, measured by both passes, on supports or set on the plate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub down: Vec3,
    pub score: Score,
    pub on_plate: bool,
    /// The turn about the vertical that stands it inside the build volume, radians, or
    /// `None` where none does.
    pub turn: Option<Scalar>,
}

/// A sliver of a dimensionless term, for a term that is zero at best.
const SLIVER: f64 = 0.002;

/// `measured` best first with what ruled each, `now` first when no turn is worth it, and
/// for auto each goal's own best right behind: `docs/design/orientation.md`.
pub fn ranked(
    faces: &Faces,
    settings: &OrientSettings,
    measured: &[Candidate],
    now: Vec3,
) -> Vec<(usize, Ruling)> {
    let readings_for = |preset| readings(faces, settings, preset, measured, now);
    let readings = readings_for(settings.preset);
    let mut order = rank(&readings, settings.preset, &NOISE);
    if settings.preset == OrientPreset::Auto && !order.is_empty() {
        let mut ends = vec![order[0]];
        for goal in OrientGoal::ALL {
            let own = readings_for(goal.into());
            let best = rank(&own, goal.into(), &NOISE).first().copied();
            ends.extend(best.filter(|at| !ends.contains(at)));
        }
        order.retain(|at| !ends.contains(at));
        ends.append(&mut order);
        order = ends;
    }
    let rulings = rulings(&readings, settings.preset, &NOISE);
    order.into_iter().map(|at| (at, rulings[at])).collect()
}

/// What each of `measured` reads ranked for `preset`, tie-broken on that preset's own total.
fn readings(
    faces: &Faces,
    settings: &OrientSettings,
    preset: OrientPreset,
    measured: &[Candidate],
    now: Vec3,
) -> Vec<Readings> {
    measured
        .iter()
        .map(|candidate| {
            let score = &candidate.score;
            let total = if preset == settings.preset {
                score.total
            } else {
                faces.total_for(preset, score)
            };
            Readings {
                fatal: f64::from(unseated(candidate) + u8::from(candidate.turn.is_none())),
                suction: f64::from(suction(score)),
                tearing: f64::from(tearing(score, settings)),
                unheld: None,
                roomless: 0.0,
                goals: OrientGoal::ALL.map(|goal| f64::from(faces.reading(goal, score))),
                tiebreak: tiebreak(preset, score, total),
                standing: candidate.down == now,
            }
        })
        .collect()
}

/// What of each goal's search reading is noise: half a newton of pull and a tenth of a
/// millimetre of stack, as the proof reads them, and a sliver of every dimensionless term.
const NOISE: Noise = [SLIVER, 0.5, 0.1];

/// What orders candidates the goal's own rules leave alike: the total, but for the shortest
/// print its stack to the noise it is read to first, support not being weighed (ADR 0283).
fn tiebreak(preset: OrientPreset, score: &Score, total: f32) -> [f64; 2] {
    let total = f64::from(total);
    if preset == OrientGoal::ShortestPrint.into() {
        let stack = f64::from(score.stack_mm()) / NOISE[OrientGoal::ShortestPrint.index()];
        [stack.round(), total]
    } else {
        [total, 0.0]
    }
}

/// A model set on the plate with no face flat on it, which only supports could hold there.
fn unseated(candidate: &Candidate) -> u8 {
    u8::from(candidate.on_plate && candidate.score.bottom_mm2 <= 0.0)
}

/// A suction cup the film would fail the print by, counted two: worse than a tearing pull.
fn suction(score: &Score) -> u8 {
    u8::from(score.cup_mm2.unwrap_or(0.0) > 0.0) * 2
}

/// A pull past the limit the film would fail the print by.
fn tearing(score: &Score, settings: &OrientSettings) -> u8 {
    u8::from(
        settings
            .peel_limit_n
            .is_some_and(|limit| score.peak_pull_n.unwrap_or(0.0) > limit),
    )
}

#[cfg(test)]
mod tests;
