use core_analysis::{MIN_CUP_MM2, PEEL_LIMIT_N};
use core_geometry::{Mesh, Quat, Scalar, Vec3};
use rayon::prelude::*;

use crate::candidates::{directions, flat_faces};
use crate::cut::cut;
use crate::error::PlateError;
use crate::policy::Ruling;
use crate::printed::Printed;
use crate::score::{Faces, OrientGoal, OrientPreset, Score};
use crate::select::{Candidate, ranked};

/// The most drain holes tried as the way down: each is a candidate cut through.
const VENTS_TRIED: usize = 8;

/// What the search is allowed to spend, and what counts as an overhang while it looks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrientSettings {
    /// How far a surface may lean before it needs holding up, degrees from vertical, the
    /// same measure a support profile carries.
    pub max_overhang_deg: Scalar,
    /// How far apart supports touch an overhang, millimetres: what one tip of the model
    /// costs, as the patch of overhang one contact holds.
    pub contact_spacing_mm: Scalar,
    /// Whether the model stands on supports, lifted off the plate, so that its underside
    /// hangs like any other rather than resting on the plate.
    pub on_supports: bool,
    /// How far supports stand the model off the plate, millimetres: the least any support
    /// is tall, which the fewest supports weighs every contact by.
    pub lift_mm: Scalar,
    /// Whether a way up that needs no supports at all is taken straight on the plate
    /// instead, where the model is otherwise judged on supports.
    pub rest_on_plate: bool,
    /// The hardest pull on the film an orientation may ask of any layer, newtons, or
    /// `None` for no limit.
    pub peel_limit_n: Option<Scalar>,
    /// The narrowest hole sealed from below that counts as a suction cup, mm^2.
    pub min_cup_mm2: Scalar,
    /// Directions taken off the sphere, on top of the model's own flat faces.
    pub samples: usize,
    /// Candidates carried into the second pass, where each one is cut to find what it
    /// pulls on the film and whether it seals a cup.
    pub shortlist: usize,
    /// How many planes a shortlisted candidate is cut at. These are samples across the
    /// model's height, not the layers it will print at.
    pub sections: usize,
    /// What the orientation is searched for: one goal, or all of them balanced.
    pub preset: OrientPreset,
    /// How far the model may turn from the way it stands, degrees, or `None` for any
    /// way at all.
    pub max_tilt_deg: Option<Scalar>,
    /// The way down the model stands now, in its own space, which the tilt is measured
    /// from and which a turn has to beat. `-Z` is the way its file was written.
    pub down_now: Vec3,
}

impl Default for OrientSettings {
    fn default() -> Self {
        Self {
            max_overhang_deg: 45.0,
            contact_spacing_mm: 4.0,
            on_supports: false,
            lift_mm: 5.0,
            rest_on_plate: false,
            peel_limit_n: Some(PEEL_LIMIT_N as Scalar),
            min_cup_mm2: MIN_CUP_MM2,
            samples: 200,
            shortlist: 8,
            sections: 24,
            preset: OrientPreset::default(),
            max_tilt_deg: None,
            down_now: Vec3::NEG_Z,
        }
    }
}

/// How a model should stand, and what that costs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oriented {
    /// The rotation to place the model with. It replaces whatever rotation the model
    /// carries: the search measures the mesh in its own space.
    pub rotation: Quat,
    pub score: Score,
    /// Whether this way up needs no supports and is judged set straight on the plate.
    pub on_plate: bool,
    /// Orientations that were measured, after the ones a print cannot tell apart were
    /// dropped.
    pub considered: usize,
    /// What decided its place among the search's finalists: whether it is worth offering
    /// beside the first.
    pub ruling: Ruling,
}

/// Finds the orientation a model prints best in.
///
/// Every candidate is scored on what can be read off the mesh without turning it; the
/// shortlist is cut to measure its pull and its cups; the best of those is picked by
/// priority; and the model stays as it stands unless the turn is worth it. See
/// `docs/design/orientation.md`.
pub fn orient(mesh: &Mesh, settings: &OrientSettings) -> Result<Oriented, PlateError> {
    orient_printed(&Printed::solid(mesh), settings)
}

/// The same for a model hollowed or drilled: supports are judged on its outer surface,
/// and its pull and its suction cups on the sections it actually cures, where a cavity
/// pulls less and a drain hole standing low lets the air in.
pub fn orient_printed(
    printed: &Printed,
    settings: &OrientSettings,
) -> Result<Oriented, PlateError> {
    finalists(printed, settings, 1)?
        .into_iter()
        .next()
        .ok_or(PlateError::NothingToOrient)
}

/// The best `count` ways to stand, best first and no two within `apart_deg` of each
/// other: what a caller able to print them judges between. The first is the answer of
/// [`orient_printed`].
pub fn finalists(
    printed: &Printed,
    settings: &OrientSettings,
    count: usize,
) -> Result<Vec<Oriented>, PlateError> {
    let mesh = printed.outer;
    let faces = Faces::of(mesh)
        .ok_or(PlateError::NothingToOrient)?
        .for_settings(settings);
    let now = settings.down_now.try_normalize().unwrap_or(Vec3::NEG_Z);
    let mut kept = vec![now];
    let by_hand = printed
        .vents
        .iter()
        .filter_map(|out| out.try_normalize())
        .take(VENTS_TRIED)
        .chain(tilts(now));
    kept.extend(by_hand.filter(|down| allowed(*down, now, settings)));

    let mut candidates = within_tilt(directions(mesh, settings.samples), now, settings);
    candidates.extend(&kept[1..]);
    let ranked: Vec<(Vec3, Score)> = candidates
        .par_iter()
        .map(|down| (*down, faces.score(*down)))
        .collect();

    let measured = shortlist(&ranked, settings.shortlist, kept)
        .into_par_iter()
        .map(|down| {
            let on_plate = !settings.on_supports;
            let sections = cut(
                printed,
                down,
                settings.sections,
                on_plate,
                settings.min_cup_mm2,
            )?;
            Ok(Candidate {
                down,
                score: faces.with_cut(faces.score(down), sections),
                on_plate,
            })
        })
        .collect::<Result<Vec<_>, PlateError>>()?;

    let mut measured = measured;
    measured.extend(seated(printed, &faces, settings, now)?);
    let considered = ranked.len();
    Ok(ordered(&faces, settings, &measured, now)
        .into_iter()
        .take(count.max(1))
        .map(|(candidate, ruling)| Oriented {
            rotation: rotation_for(candidate.down),
            score: candidate.score,
            on_plate: candidate.on_plate,
            considered,
            ruling,
        })
        .collect())
}

/// How far apart two finalists stand at least, degrees: nearer, and printing both tells
/// a caller nothing the first did not. Support volume changes slowly with a turn.
fn apart_deg(preset: OrientPreset) -> Scalar {
    match preset {
        OrientPreset::Goal(OrientGoal::FewestSupports) => 20.0,
        OrientPreset::Goal(OrientGoal::LeastPeel | OrientGoal::ShortestPrint)
        | OrientPreset::Auto => 10.0,
    }
}

/// `measured` best first by the judge's priorities, the way the model stands kept ahead of
/// a turn that is not worth making, and anything within `apart_deg` of a better one held
/// the same way dropped.
fn ordered(
    faces: &Faces,
    settings: &OrientSettings,
    measured: &[Candidate],
    now: Vec3,
) -> Vec<(Candidate, Ruling)> {
    let order = ranked(faces, settings, measured, now);
    let apart = apart_deg(settings.preset).to_radians().cos();
    let mut chosen: Vec<(Candidate, Ruling)> = Vec::new();
    for (at, ruling) in order {
        let candidate = measured[at];
        let alone = chosen
            .iter()
            .filter(|(kept, _)| kept.on_plate == candidate.on_plate)
            .all(|(kept, _)| kept.down.dot(candidate.down) < apart);
        // Every turn is weighed against the way the model stands, so that is kept
        // however near a better one it lies.
        if alone || candidate.down == now {
            chosen.push((candidate, ruling));
        }
    }
    chosen
}

/// Flat faces tried as a seat on the plate, on top of the way the model stands.
const SEATS: usize = 6;

/// The most of its surface a model set on the plate may have hanging and still be said to
/// need no supports: a hundredth, which a chamfer stays under.
const REST_HELD: Scalar = 0.01;

/// The share of its shadow a model has flat on the plate once it is sitting on a base
/// rather than on a ledge; the way it stands now is tried on the plate whatever it has.
const SEAT_SHARE: Scalar = 0.25;

/// The ways the model stands straight on the plate, measured there, ranked with the rest
/// by the same rule; none unless the settings ask. See `docs/design/orientation.md`.
fn seated(
    printed: &Printed,
    faces: &Faces,
    settings: &OrientSettings,
    now: Vec3,
) -> Result<Vec<Candidate>, PlateError> {
    if !(settings.rest_on_plate && settings.on_supports) {
        return Ok(Vec::new());
    }
    let seats: Vec<Vec3> = std::iter::once(now)
        .chain(
            flat_faces(printed.outer)
                .into_iter()
                .take(SEATS)
                .map(|flat| flat.normal)
                .filter(|down| *down != now && allowed(*down, now, settings)),
        )
        .collect();
    let measured = seats
        .into_par_iter()
        .map(|down| {
            let score = faces.score_on_plate(down);
            let rests = faces.held(&score) <= REST_HELD;
            let seat = down == now || rests || score.bottom_mm2 >= SEAT_SHARE * score.shadow_mm2;
            if score.bottom_mm2 <= 0.0 || !seat {
                return Ok(None);
            }
            let sections = cut(printed, down, settings.sections, true, settings.min_cup_mm2)?;
            Ok(Some(Candidate {
                down,
                score: faces.with_cut(score, sections),
                on_plate: true,
            }))
        })
        .collect::<Result<Vec<_>, PlateError>>()?;
    Ok(measured.into_iter().flatten().collect())
}

/// How far the model is leaned from the way it stands, degrees, and in how many
/// directions: the poses a person tries by hand, which the first pass ranks badly because
/// a leaning model's shadow overstates its sections.
const TILTS_DEG: [Scalar; 3] = [15.0, 30.0, 45.0];
const TILT_SIDES: usize = 8;

fn tilts(now: Vec3) -> impl Iterator<Item = Vec3> {
    let (side, up) = now.any_orthonormal_pair();
    TILTS_DEG.into_iter().flat_map(move |lean_deg| {
        (0..TILT_SIDES).map(move |at| {
            let around = std::f32::consts::TAU * at as Scalar / TILT_SIDES as Scalar;
            let axis = side * around.cos() + up * around.sin();
            Quat::from_axis_angle(axis, lean_deg.to_radians()) * now
        })
    })
}

/// One number read off a score, lower being better.
type Reading = fn(&Score) -> Scalar;

/// Candidates carried into the second pass for being the best at one thing, whatever
/// they total.
const BEST_OF_EACH: usize = 2;

/// The candidates the second pass cuts: `kept` — the way the model stands, which a turn
/// has to beat, its drain holes pointed down and its leans — then the best `count` by
/// total and the best few on each reading alone.
fn shortlist(ranked: &[(Vec3, Score)], count: usize, kept: Vec<Vec3>) -> Vec<Vec3> {
    let readings: [(Reading, usize); 4] = [
        (|score| score.total, count.max(1)),
        (|score| score.overhang_mm2, BEST_OF_EACH),
        (|score| score.shadow_mm2, BEST_OF_EACH),
        (|score| score.stack_mm(), BEST_OF_EACH),
    ];
    let mut chosen = kept;
    for (reading, take) in readings {
        let mut by_reading: Vec<&(Vec3, Score)> = ranked.iter().collect();
        by_reading.sort_by(|a, b| reading(&a.1).total_cmp(&reading(&b.1)));
        for (down, _) in by_reading.into_iter().take(take) {
            if !chosen.contains(down) {
                chosen.push(*down);
            }
        }
    }
    chosen
}

/// The candidates no further than the tilt allows from the way down the model stands,
/// which is always one of them: standing still is never out of reach.
fn within_tilt(candidates: Vec<Vec3>, now: Vec3, settings: &OrientSettings) -> Vec<Vec3> {
    std::iter::once(now)
        .chain(
            candidates
                .into_iter()
                .filter(|down| allowed(*down, now, settings)),
        )
        .collect()
}

/// Whether `down` is within the tilt limit of `now`.
fn allowed(down: Vec3, now: Vec3, settings: &OrientSettings) -> bool {
    settings
        .max_tilt_deg
        .is_none_or(|max_deg| down.dot(now) >= max_deg.clamp(0.0, 180.0).to_radians().cos() - 1e-6)
}

/// The rotation that stands a model up with `down` pointing at the plate.
///
/// Which way it faces about the vertical is left to the shortest turn: the plate is
/// round to a print, and arranging is what decides where a model looks.
pub fn rotation_for(down: Vec3) -> Quat {
    down.try_normalize().map_or(Quat::IDENTITY, |unit| {
        Quat::from_rotation_arc(unit, Vec3::NEG_Z)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    /// Where the model's own `axis` ends up once the orientation is applied.
    fn placed(rotation: Quat, axis: Vec3) -> Vec3 {
        rotation * axis
    }

    #[test]
    fn a_slab_pulling_past_the_limit_is_turned_off_its_face_for_the_least_peel() {
        let mesh = cuboid(Vec3::new(100.0, 100.0, 4.0));
        let settings = OrientSettings {
            preset: OrientGoal::LeastPeel.into(),
            ..OrientSettings::default()
        };
        let found = orient(&mesh, &settings).expect("a slab can be oriented");

        // Lying flat, a 100 mm square pulls some 98 N; turned, each section is a strip.
        let thin = placed(found.rotation, Vec3::Z);
        assert!(
            thin.z.abs() < 0.95,
            "the slab does not lie flat, got {thin}"
        );
        let pull = found.score.peak_pull_n.expect("the winner was cut");
        assert!(
            f64::from(pull) < PEEL_LIMIT_N / 3.0,
            "under enough, got {pull} N"
        );
        assert!(found.considered > 100, "the sphere was searched too");
    }

    #[test]
    fn a_slab_pulling_little_enough_stays_flat_for_the_least_peel() {
        // A 40 mm square pulls some 8 N, under a third of the limit: nothing to win.
        let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
        let settings = OrientSettings {
            preset: OrientGoal::LeastPeel.into(),
            ..OrientSettings::default()
        };
        let found = orient(&mesh, &settings).expect("a slab can be oriented");
        let thin = placed(found.rotation, Vec3::Z);
        assert!(thin.z.abs() > 0.95, "the slab still lies flat, got {thin}");
    }

    #[test]
    fn a_slab_lying_on_the_plate_stays_there_for_the_fewest_supports() {
        // Flat or on its edge, nothing hangs: no turn wins the goal anything, and the peel
        // of 1600 mm2 is the least-peel goal's to weigh, or a tearing pull's.
        let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
        let found = orient(&mesh, &OrientSettings::default()).expect("a slab can be oriented");
        let thin = placed(found.rotation, Vec3::Z);
        assert!(thin.z.abs() > 0.95, "the slab still lies flat, got {thin}");
    }

    #[test]
    fn a_shortest_print_lays_a_tall_column_down() {
        let mesh = cuboid(Vec3::new(6.0, 6.0, 80.0));
        let settings = OrientSettings {
            preset: OrientGoal::ShortestPrint.into(),
            ..OrientSettings::default()
        };
        let found = orient(&mesh, &settings).expect("a column can be oriented");
        // Lying down it is 6 mm tall rather than 80: far fewer layers.
        let long_axis = placed(found.rotation, Vec3::Z);
        assert!(
            long_axis.z.abs() < 0.05,
            "the long axis lies flat, got {long_axis}"
        );
    }

    #[test]
    fn no_tilt_allowed_leaves_the_model_standing_as_it_is() {
        let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
        let settings = OrientSettings {
            max_tilt_deg: Some(0.0),
            ..OrientSettings::default()
        };
        let found = orient(&mesh, &settings).expect("a slab can be oriented");
        // Unbounded, the slab is stood on its edge; held to no tilt it stays flat.
        let thin = placed(found.rotation, Vec3::Z);
        assert!(thin.z.abs() > 0.999, "the slab still lies flat, got {thin}");
    }

    #[test]
    fn a_tall_column_prints_standing_up() {
        let mesh = cuboid(Vec3::new(6.0, 6.0, 80.0));
        let found = orient(&mesh, &OrientSettings::default()).expect("a column can be oriented");

        // 36 mm2 a layer standing, against 480 mm2 lying down: the extra layers cost less
        // than the peel would.
        let long_axis = placed(found.rotation, Vec3::Z);
        assert!(
            long_axis.z.abs() > 0.99,
            "the long axis stays vertical, got {long_axis}"
        );
    }

    #[test]
    fn a_cube_stands_on_a_face_whichever_one_it_picks() {
        let mesh = cuboid(Vec3::splat(20.0));
        let found = orient(&mesh, &OrientSettings::default()).expect("a cube can be oriented");

        // Every face of a cube is as good as the others, so what is pinned down is that
        // one of them is flat on the plate: an axis of the model points straight down.
        let down: Vec3 = [Vec3::X, Vec3::Y, Vec3::Z]
            .into_iter()
            .map(|axis| placed(found.rotation, axis))
            .max_by(|a, b| a.z.abs().total_cmp(&b.z.abs()))
            .expect("three axes");
        assert!(
            down.z.abs() > 0.999,
            "a face is flat on the plate, got {down}"
        );
    }

    #[test]
    fn a_model_needing_no_supports_rests_on_the_plate() {
        let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
        let settings = OrientSettings {
            on_supports: true,
            rest_on_plate: true,
            ..OrientSettings::default()
        };
        let found = orient(&mesh, &settings).expect("a slab can be oriented");
        // On a face or on an edge nothing of a box hangs, and neither pull is near the limit.
        assert!(found.on_plate, "it is judged set on the plate");
        let upright = [Vec3::X, Vec3::Y, Vec3::Z]
            .into_iter()
            .any(|axis| placed(found.rotation, axis).z.abs() > 0.999);
        assert!(upright, "with one of its faces flat on it");

        let lifted = orient(
            &mesh,
            &OrientSettings {
                rest_on_plate: false,
                ..settings
            },
        )
        .expect("a slab can be oriented");
        assert!(!lifted.on_plate, "unasked, it is judged on supports");
    }

    #[test]
    fn finalists_start_with_the_answer_and_stand_apart() {
        let mesh = cuboid(Vec3::new(30.0, 20.0, 10.0));
        let settings = OrientSettings {
            on_supports: true,
            ..OrientSettings::default()
        };
        let answer = orient(&mesh, &settings).expect("a box can be oriented");
        let found = finalists(&Printed::solid(&mesh), &settings, 5).expect("a box has finalists");

        assert_eq!(found.len(), 5);
        assert_eq!(found[0], answer);
        for (at, pose) in found.iter().enumerate() {
            for other in &found[..at] {
                let apart = (pose.rotation.inverse() * Vec3::NEG_Z)
                    .angle_between(other.rotation.inverse() * Vec3::NEG_Z);
                assert!(apart.to_degrees() > 9.9, "two finalists {apart} rad apart");
            }
        }
    }

    #[test]
    fn the_model_is_leaned_three_ways_eight_times_round() {
        let leans: Vec<Vec3> = tilts(Vec3::NEG_Z).collect();
        assert_eq!(leans.len(), 24);
        for (at, down) in leans.iter().enumerate() {
            let lean_deg = down.angle_between(Vec3::NEG_Z).to_degrees();
            let expected = TILTS_DEG[at / TILT_SIDES];
            assert!(
                (lean_deg - expected).abs() < 1e-3,
                "{lean_deg} for {expected}"
            );
        }
    }

    #[test]
    fn the_rotation_takes_the_chosen_direction_to_the_plate() {
        let down = Vec3::new(1.0, 2.0, -3.0).normalize();
        let turned = rotation_for(down) * down;
        assert!(
            turned.abs_diff_eq(Vec3::NEG_Z, 1e-5),
            "the chosen direction ends up pointing at the plate, got {turned}"
        );
    }

    #[test]
    fn the_peak_section_of_a_box_is_its_footprint() {
        let mesh = cuboid(Vec3::new(10.0, 6.0, 20.0));
        let peak = cut(&Printed::solid(&mesh), Vec3::NEG_Z, 8, true, MIN_CUP_MM2)
            .expect("a box slices")
            .peak_mm2;
        assert!((peak - 60.0).abs() < 1e-2, "a 10 by 6 section, got {peak}");
    }

    #[test]
    fn the_peak_section_counts_overlapping_bodies_once() {
        // Two 10 mm boxes sharing a 2 x 2 mm corner over their whole height: the section
        // is 100 + 100 - 4, not the 200 their contour areas add up to (ADR 0206).
        let mut mesh = cuboid(Vec3::splat(10.0));
        let second = cuboid(Vec3::splat(10.0));
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend(
            second
                .vertices
                .iter()
                .map(|v| *v + Vec3::new(8.0, 8.0, 0.0)),
        );
        mesh.faces.extend(
            second
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );

        let peak = cut(&Printed::solid(&mesh), Vec3::NEG_Z, 4, true, MIN_CUP_MM2)
            .expect("two boxes slice")
            .peak_mm2;
        assert!((peak - 196.0).abs() < 1e-2, "{peak}");
    }

    #[test]
    fn a_shortest_print_lays_a_slab_flat_whatever_it_pulls() {
        let mesh = cuboid(Vec3::new(100.0, 100.0, 4.0));
        let settings = OrientSettings {
            preset: OrientGoal::ShortestPrint.into(),
            on_supports: true,
            ..OrientSettings::default()
        };
        let found = orient(&mesh, &settings).expect("a slab can be oriented");
        // Flat, a 100 mm square pulls some 98 N, past the 30 N limit, and stacks 4 mm: the
        // least of any pose, which the shortest print takes (ADR 0283).
        let thin = placed(found.rotation, Vec3::Z);
        assert!(thin.z.abs() > 0.999, "the slab lies flat, got {thin}");
        let pull = found.score.peak_pull_n.expect("the winner was cut");
        assert!(f64::from(pull) > PEEL_LIMIT_N, "{pull}");
    }

    /// An open-topped 30 mm box with 2 mm walls, open side up as its file has it.
    fn open_box() -> Mesh {
        let mut mesh = Mesh::default();
        let parts = [
            (Vec3::ZERO, Vec3::new(30.0, 30.0, 2.0)),
            (Vec3::ZERO, Vec3::new(2.0, 30.0, 20.0)),
            (Vec3::new(28.0, 0.0, 0.0), Vec3::new(30.0, 30.0, 20.0)),
            (Vec3::ZERO, Vec3::new(30.0, 2.0, 20.0)),
            (Vec3::new(0.0, 28.0, 0.0), Vec3::new(30.0, 30.0, 20.0)),
        ];
        for (min, max) in parts {
            let part = cuboid(max - min);
            let offset = mesh.vertices.len() as u32;
            mesh.vertices.extend(part.vertices.iter().map(|v| *v + min));
            mesh.faces.extend(
                part.faces
                    .iter()
                    .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
            );
        }
        mesh
    }

    #[test]
    fn an_open_box_is_not_left_standing_as_a_suction_cup() {
        let mesh = open_box();
        let as_is =
            cut(&Printed::solid(&mesh), Vec3::NEG_Z, 24, true, MIN_CUP_MM2).expect("the box cuts");
        assert!(
            as_is.cup_mm2 > 600.0,
            "open side up, its floor seals the inside"
        );

        let peel = OrientSettings {
            preset: OrientGoal::LeastPeel.into(),
            ..OrientSettings::default()
        };
        let found = orient(&mesh, &peel).expect("a box can be oriented");
        assert_eq!(
            found.score.cup_mm2,
            Some(0.0),
            "the turn it takes seals nothing"
        );

        let found = orient(&mesh, &OrientSettings::default()).expect("a box can be oriented");
        let floor = found.rotation * Vec3::NEG_Z;
        assert!(
            floor.z < -0.99,
            "the fewest supports leaves its floor on the plate, cup and all, got {floor}"
        );
    }

    #[test]
    fn a_mesh_with_no_faces_cannot_be_oriented() {
        let error = orient(&Mesh::default(), &OrientSettings::default())
            .expect_err("an empty mesh has no orientation");
        assert!(matches!(error, PlateError::NothingToOrient));
    }
}
