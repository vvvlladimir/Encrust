use core_geometry::Vec3;

use crate::orient::OrientSettings;
use crate::policy::{Noise, Readings, rank};
use crate::score::{Faces, OrientGoal, Score};

/// One way down, measured by both passes, on supports or set on the plate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub down: Vec3,
    pub score: Score,
    pub on_plate: bool,
}

/// A sliver of a dimensionless term, for a term that is zero at best.
const SLIVER: f64 = 0.002;

/// `measured` best first, the way the model stands — `now` — first when no turn is worth
/// it. See `docs/design/orientation.md`.
pub fn ranked(
    faces: &Faces,
    settings: &OrientSettings,
    measured: &[Candidate],
    now: Vec3,
) -> Vec<usize> {
    let readings: Vec<Readings> = measured
        .iter()
        .map(|candidate| Readings {
            fatal: f64::from(unseated(candidate)),
            suction: f64::from(suction(&candidate.score, settings)),
            unheld: None,
            roomless: 0.0,
            goals: OrientGoal::ALL.map(|goal| f64::from(faces.reading(goal, &candidate.score))),
            tiebreak: [f64::from(candidate.score.total), 0.0],
            standing: candidate.down == now,
        })
        .collect();
    rank(&readings, settings.goal, &NOISE)
}

/// What of each goal's search reading is noise: half a newton of pull, as the proof reads
/// it, and a sliver of every dimensionless term.
const NOISE: Noise = [SLIVER, 0.5, SLIVER, SLIVER];

/// A model set on the plate with no face flat on it, which only supports could hold there.
fn unseated(candidate: &Candidate) -> u8 {
    u8::from(candidate.on_plate && candidate.score.bottom_mm2 <= 0.0)
}

/// What the film would fail the print by, worse first: a suction cup counts two, a pull
/// past the limit one.
fn suction(score: &Score, settings: &OrientSettings) -> u8 {
    let cup = score.cup_mm2.unwrap_or(0.0) > 0.0;
    let tearing = settings
        .peel_limit_n
        .is_some_and(|limit| score.peak_pull_n.unwrap_or(0.0) > limit);
    u8::from(cup) * 2 + u8::from(tearing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::OrientGoal;
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

    /// A measured candidate pointing `down`, costing `overhang_mm2` of support held 10 mm up,
    /// and `total`.
    fn candidate(down: Vec3, overhang_mm2: f32, total: f32, cup_mm2: f32) -> Candidate {
        Candidate {
            down,
            score: Score {
                overhang_mm2,
                low_points: 0,
                support_mm3: overhang_mm2 * 10.0,
                under_mm2: overhang_mm2,
                under_mm3: overhang_mm2 * 10.0,
                bottom_mm2: 0.0,
                height_mm: 10.0,
                shadow_mm2: 100.0,
                peak_mm2: Some(100.0),
                peak_pull_n: Some(1.0),
                top_overhang_mm2: 0.0,
                cup_mm2: Some(cup_mm2),
                lift_mm: 0.0,
                total,
            },
            on_plate: false,
        }
    }

    /// Where `candidate` ranks among `measured`, best first being 0.
    fn place(measured: &[Candidate], candidate: usize, now: Vec3) -> usize {
        place_for(OrientGoal::FewestSupports, measured, candidate, now)
    }

    fn place_for(goal: OrientGoal, measured: &[Candidate], candidate: usize, now: Vec3) -> usize {
        let settings = OrientSettings {
            goal,
            ..OrientSettings::default()
        };
        let mesh = cube();
        let faces = Faces::of(&mesh)
            .expect("a cube has faces")
            .for_settings(&settings);
        let order = ranked(&faces, &settings, measured, now);
        order
            .iter()
            .position(|at| *at == candidate)
            .expect("every candidate is ranked")
    }

    #[test]
    fn a_suction_cup_loses_whatever_it_scores_but_for_the_fewest_supports() {
        let cupping = candidate(Vec3::X, 0.0, 0.1, 400.0);
        let clean = candidate(Vec3::Y, 50.0, 9.0, 0.0);
        let peel = OrientGoal::LeastPeel;
        assert_eq!(place_for(peel, &[cupping, clean], 1, Vec3::NEG_Z), 0);
        assert_eq!(
            place(&[cupping, clean], 0, Vec3::NEG_Z),
            0,
            "a cup is a drain hole's to fix, not more support's"
        );
    }

    #[test]
    fn the_goals_own_term_decides_before_the_total() {
        // Fewest supports: 100 mm3 of support against 600, whatever the other terms add.
        let few = candidate(Vec3::X, 10.0, 5.0, 0.0);
        let many = candidate(Vec3::Y, 60.0, 1.0, 0.0);
        assert_eq!(place(&[many, few], 1, Vec3::NEG_Z), 0);

        // No band: the least support wins however close the next, whatever its total.
        let close = candidate(Vec3::NEG_X, 10.5, 4.0, 0.0);
        assert_eq!(place(&[few, close], 1, Vec3::NEG_Z), 1);
    }

    #[test]
    fn the_fewest_supports_turn_for_any_less_support() {
        // The gain is the goal's own: the support volume, not the total.
        let now = candidate(Vec3::NEG_Z, 10.0, 1.0, 0.0);
        let slightly = candidate(Vec3::X, 9.8, 2.0, 0.0);
        let same = candidate(Vec3::Y, 10.0, 0.5, 0.0);
        assert_eq!(
            place(&[slightly, now], 0, Vec3::NEG_Z),
            0,
            "2 % less is less"
        );
        assert_eq!(place(&[same, now], 1, Vec3::NEG_Z), 0, "as much stays");
    }
}
