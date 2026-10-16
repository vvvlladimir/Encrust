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
                cup_mm2: Some(cup_mm2),
                lift_mm: 0.0,
                total,
            },
            on_plate: false,
            turn: Some(0.0),
        }
    }

    /// Where `candidate` ranks among `measured`, best first being 0.
    fn place(measured: &[Candidate], candidate: usize, now: Vec3) -> usize {
        place_for(OrientGoal::FewestSupports, measured, candidate, now)
    }

    fn place_for(
        preset: impl Into<OrientPreset>,
        measured: &[Candidate],
        candidate: usize,
        now: Vec3,
    ) -> usize {
        let settings = OrientSettings {
            preset: preset.into(),
            ..OrientSettings::default()
        };
        let mesh = cube();
        let faces = Faces::of(&mesh)
            .expect("a cube has faces")
            .for_settings(&settings);
        let order = ranked(&faces, &settings, measured, now);
        order
            .iter()
            .position(|(at, _)| *at == candidate)
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
    fn a_pose_past_the_build_volume_loses_whatever_it_scores() {
        let mut overflowing = candidate(Vec3::X, 0.0, 0.1, 0.0);
        overflowing.turn = None;
        let fitting = candidate(Vec3::Y, 50.0, 9.0, 0.0);
        for preset in OrientPreset::all() {
            assert_eq!(
                place_for(preset, &[overflowing, fitting], 1, Vec3::NEG_Z),
                0,
                "{}",
                preset.label()
            );
        }
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

    #[test]
    fn within_its_band_the_shortest_print_takes_the_shorter_stack_not_the_lesser_support() {
        let mut short = candidate(Vec3::X, 60.0, 5.0, 0.0);
        short.score.height_mm = 10.0;
        let mut tall = candidate(Vec3::Y, 10.0, 1.0, 0.0);
        tall.score.height_mm = 10.5;
        assert_eq!(
            place_for(OrientGoal::ShortestPrint, &[tall, short], 1, Vec3::NEG_Z),
            0,
            "10.5 mm is within 10 * 1.02 + 1, and support is not weighed (ADR 0283)"
        );
    }

    #[test]
    fn auto_ranks_each_goals_own_best_right_behind_its_answer() {
        let mut tall = candidate(Vec3::X, 1.0, 1.0, 0.0);
        tall.score.height_mm = 60.0;
        let mut pulling = candidate(Vec3::Y, 30.0, 2.0, 0.0);
        pulling.score.peak_pull_n = Some(5.0);
        let mut short = candidate(Vec3::Z, 40.0, 3.0, 0.0);
        short.score.height_mm = 5.0;
        let middling = candidate(Vec3::NEG_X, 20.0, 0.5, 0.0);
        let mut measured = [middling, tall, pulling, short];
        for (at, pose) in measured.iter_mut().enumerate() {
            if at != 2 {
                pose.score.peak_pull_n = Some(20.0);
            }
        }

        let settings = OrientSettings {
            preset: OrientPreset::Auto,
            ..OrientSettings::default()
        };
        let mesh = cube();
        let faces = Faces::of(&mesh)
            .expect("a cube has faces")
            .for_settings(&settings);
        let order = ranked(&faces, &settings, &measured, Vec3::NEG_Z);
        let ends: Vec<usize> = order.iter().take(4).map(|(at, _)| *at).collect();
        for (goal, best) in [
            (OrientGoal::FewestSupports, 1),
            (OrientGoal::LeastPeel, 2),
            (OrientGoal::ShortestPrint, 3),
        ] {
            assert_eq!(place_for(goal, &measured, best, Vec3::NEG_Z), 0);
            assert!(ends.contains(&best), "{} leads the proof", goal.label());
        }
    }

    #[test]
    fn auto_follows_its_answer_with_the_shortest_stack_not_auto_s_own_tiebreak() {
        let pose = |down: Vec3, overhang_mm2: f32, total: f32, height_mm: f32| {
            let mut pose = candidate(down, overhang_mm2, total, 0.0);
            pose.score.height_mm = height_mm;
            pose
        };
        let measured = [
            pose(Vec3::X, 60.0, 1.0, 30.0),
            pose(Vec3::Y, 120.0, 1.0, 28.0),
            pose(Vec3::Z, 180.0, 1.0, 26.0),
            pose(Vec3::NEG_X, 240.0, 1.0, 24.0),
            pose(Vec3::new(1.0, 1.0, 0.0).normalize(), 1800.0, 0.1, 10.5),
            pose(Vec3::NEG_Y, 2400.0, 9.0, 10.0),
        ];
        let short = 5;
        assert_eq!(
            place_for(OrientGoal::ShortestPrint, &measured, short, Vec3::NEG_Z),
            0
        );

        let order = ranked_for(OrientPreset::Auto, &measured);
        assert!(
            order
                .iter()
                .take(OrientGoal::ALL.len() + 1)
                .any(|(at, _)| *at == short),
            "10 mm is the shortest print's own best, whatever auto's total says: {order:?}"
        );
    }

    fn ranked_for(preset: OrientPreset, measured: &[Candidate]) -> Vec<(usize, Ruling)> {
        let settings = OrientSettings {
            preset,
            ..OrientSettings::default()
        };
        let mesh = cube();
        let faces = Faces::of(&mesh)
            .expect("a cube has faces")
            .for_settings(&settings);
        ranked(&faces, &settings, measured, Vec3::NEG_Z)
    }
}
