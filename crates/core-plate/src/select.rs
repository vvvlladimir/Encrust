use core_geometry::Vec3;

use crate::orient::OrientSettings;
use crate::policy::{Readings, rank};
use crate::score::{Faces, Score};

/// One way down, measured by both passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub down: Vec3,
    pub score: Score,
}

/// What of a goal's dimensionless term is noise: a sliver, for a term that is zero at best.
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
            fatal: f64::from(failures(&candidate.score, settings)),
            unheld: None,
            primary: f64::from(faces.primary(&candidate.score)),
            tiebreak: f64::from(candidate.score.total),
            standing: candidate.down == now,
        })
        .collect();
    rank(&readings, SLIVER)
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

    /// A measured candidate pointing `down`, costing `overhang_mm2` of support and `total`.
    fn candidate(down: Vec3, overhang_mm2: f32, total: f32, cup_mm2: f32) -> Candidate {
        Candidate {
            down,
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

    /// Where `candidate` ranks among `measured`, best first being 0.
    fn place(measured: &[Candidate], candidate: usize, now: Vec3) -> usize {
        let mesh = cube();
        let faces = Faces::of(&mesh).expect("a cube has faces");
        let order = ranked(&faces, &OrientSettings::default(), measured, now);
        order
            .iter()
            .position(|at| *at == candidate)
            .expect("every candidate is ranked")
    }

    #[test]
    fn a_suction_cup_loses_whatever_it_scores() {
        let cupping = candidate(Vec3::X, 0.0, 0.1, 400.0);
        let clean = candidate(Vec3::Y, 50.0, 9.0, 0.0);
        assert_eq!(place(&[cupping, clean], 1, Vec3::NEG_Z), 0);
    }

    #[test]
    fn the_goals_own_term_decides_before_the_total() {
        // Fewest supports: 10 mm2 of overhang against 60, whatever the other terms add.
        let few = candidate(Vec3::X, 10.0, 5.0, 0.0);
        let many = candidate(Vec3::Y, 60.0, 1.0, 0.0);
        assert_eq!(place(&[many, few], 1, Vec3::NEG_Z), 0);

        // Within a tenth of the best, the total breaks the tie.
        let close = candidate(Vec3::Z, 10.5, 4.0, 0.0);
        assert_eq!(place(&[few, close], 1, Vec3::NEG_Z), 0);
    }

    #[test]
    fn a_turn_winning_less_than_its_gain_is_not_worth_making() {
        // The gain is the goal's own: the support term, not the total.
        let now = candidate(Vec3::NEG_Z, 10.0, 1.0, 0.0);
        let slightly = candidate(Vec3::X, 9.8, 0.5, 0.0);
        let clearly = candidate(Vec3::Y, 6.0, 1.0, 0.0);
        assert_eq!(
            place(&[slightly, now], 1, Vec3::NEG_Z),
            0,
            "2 % is not worth a turn"
        );
        assert_eq!(place(&[clearly, now], 0, Vec3::NEG_Z), 0, "20 % is");
    }
}
