use core_analysis::{MIN_CUP_MM2, PEEL_LIMIT_N};
use core_geometry::{Mesh, Quat, Scalar, Vec3};
use rayon::prelude::*;

use crate::candidates::{directions, flat_faces};
use crate::cut::cut;
use crate::error::PlateError;
use crate::fit::{Envelope, turn_to_fit};
use crate::policy::Ruling;
use crate::printed::Printed;
use crate::score::{Faces, OrientGoal, OrientPreset, Score};
use crate::select::{Candidate, ranked};
use crate::watch::{Phase, Pieces, Watch};

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
    /// The printer's build volume, millimetres along X, Y and Z, that every pose has to
    /// stand inside on its supports, or `None` for no limit.
    pub build_volume_mm: Option<Vec3>,
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
            build_volume_mm: None,
        }
    }
}

/// How a model should stand, and what that costs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oriented {
    /// The rotation to place the model with. It replaces whatever rotation the model
    /// carries, the search measuring the mesh in its own space, and turns it about the
    /// vertical only as far as standing inside the build volume needs.
    pub rotation: Quat,
    pub score: Score,
    /// Whether this way up needs no supports and is judged set straight on the plate.
    pub on_plate: bool,
    /// Whether it reaches past the build volume however it is turned about the vertical.
    pub overflows: bool,
    /// Orientations that were measured, after the ones a print cannot tell apart were
    /// dropped.
    pub considered: usize,
    /// What decided its place among the search's finalists: whether it is worth offering
    /// beside the first.
    pub ruling: Ruling,
}

/// Finds the orientation a model prints best in: `docs/design/orientation.md`.
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
    finalists(printed, settings, 1, Watch::never())?
        .into_iter()
        .next()
        .ok_or(PlateError::NothingToOrient)
}

/// The best `count` ways to stand, best first and no two within `apart_deg` of each
/// other: what a caller able to print them judges between. The first is the answer of
/// [`orient_printed`]. `watch` is told how far the cuts have got and may stop them.
pub fn finalists(
    printed: &Printed,
    settings: &OrientSettings,
    count: usize,
    watch: Watch<'_>,
) -> Result<Vec<Oriented>, PlateError> {
    let mesh = printed.outer;
    let faces = Faces::of(mesh)
        .ok_or(PlateError::NothingToOrient)?
        .for_settings(settings);
    let now = settings.down_now.try_normalize().unwrap_or(Vec3::NEG_Z);
    let kept = kept_poses(printed, settings, now);
    let envelope = settings
        .build_volume_mm
        .map(|room_mm| Envelope::of(mesh, room_mm));
    let envelope = envelope.as_ref();

    let mut candidates = within_tilt(directions(mesh, settings.samples), now, settings);
    candidates.extend(&kept[1..]);
    let ranked = first_pass(&faces, settings, &candidates, envelope);
    let shortlisted = shortlist(&ranked, settings.shortlist, kept);
    let cuts = Pieces::start(watch, Phase::Looking, shortlisted.len())?;
    let mut measured = measure(printed, &faces, settings, shortlisted, (envelope, &cuts))?;
    measured.extend(seated(printed, &faces, settings, now, envelope)?);
    let considered = ranked.len();
    Ok(ordered(&faces, settings, &measured, now)
        .into_iter()
        .take(count.max(1))
        .map(|(candidate, ruling)| Oriented {
            rotation: Quat::from_rotation_z(candidate.turn.unwrap_or(0.0))
                * rotation_for(candidate.down),
            score: candidate.score,
            on_plate: candidate.on_plate,
            overflows: candidate.turn.is_none(),
            considered,
            ruling,
        })
        .collect())
}

/// The way the model stands, then the poses a person tries by hand within the tilt: what
/// the shortlist keeps whatever the first pass makes of them.
fn kept_poses(printed: &Printed, settings: &OrientSettings, now: Vec3) -> Vec<Vec3> {
    let mut kept = vec![now];
    let by_hand = printed
        .vents
        .iter()
        .filter_map(|out| out.try_normalize())
        .take(VENTS_TRIED)
        .chain(tilts(now));
    kept.extend(by_hand.filter(|down| allowed(*down, now, settings)));
    kept
}

/// How high a pose is lifted onto supports, millimetres, or nought on the plate.
fn lift_of(settings: &OrientSettings) -> Scalar {
    if settings.on_supports {
        settings.lift_mm
    } else {
        0.0
    }
}

/// Every candidate scored on what the mesh shows without cutting it.
fn first_pass(
    faces: &Faces,
    settings: &OrientSettings,
    candidates: &[Vec3],
    envelope: Option<&Envelope>,
) -> Vec<First> {
    let lift_mm = lift_of(settings);
    candidates
        .par_iter()
        .map(|down| First {
            down: *down,
            score: faces.score(*down),
            overflows: turn_to_fit(envelope, *down, lift_mm).is_none(),
        })
        .collect()
}

/// The shortlist cut, to read its pull and its cups.
fn measure(
    printed: &Printed,
    faces: &Faces,
    settings: &OrientSettings,
    shortlisted: Vec<Vec3>,
    (envelope, cuts): (Option<&Envelope>, &Pieces<'_>),
) -> Result<Vec<Candidate>, PlateError> {
    let lift_mm = lift_of(settings);
    let on_plate = !settings.on_supports;
    shortlisted
        .into_par_iter()
        .map(|down| {
            let sections = cut(
                printed,
                down,
                settings.sections,
                on_plate,
                settings.min_cup_mm2,
            )?;
            cuts.finish()?;
            Ok(Candidate {
                down,
                score: faces.with_cut(faces.score(down), sections),
                on_plate,
                turn: turn_to_fit(envelope, down, lift_mm),
            })
        })
        .collect()
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
    envelope: Option<&Envelope>,
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
                turn: turn_to_fit(envelope, down, 0.0),
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

/// One way down as the first pass reads it.
struct First {
    down: Vec3,
    score: Score,
    /// Whether it reaches past the build volume however it is turned.
    overflows: bool,
}

/// One number read off a score, lower being better.
type Reading = fn(&Score) -> Scalar;

/// Candidates carried into the second pass for being the best at one thing, whatever
/// they total.
const BEST_OF_EACH: usize = 2;

/// The candidates the second pass cuts: `kept` — the way the model stands, which a turn
/// has to beat, its drain holes pointed down and its leans — then the best `count` by
/// total and the best few on each reading alone, any that fit the build volume first.
fn shortlist(ranked: &[First], count: usize, kept: Vec<Vec3>) -> Vec<Vec3> {
    let readings: [(Reading, usize); 4] = [
        (|score| score.total, count.max(1)),
        (|score| score.overhang_mm2, BEST_OF_EACH),
        (|score| score.shadow_mm2, BEST_OF_EACH),
        (|score| score.stack_mm(), BEST_OF_EACH),
    ];
    let mut chosen = kept;
    for (reading, take) in readings {
        let mut by_reading: Vec<&First> = ranked.iter().collect();
        by_reading.sort_by(|a, b| {
            (a.overflows.cmp(&b.overflows))
                .then_with(|| reading(&a.score).total_cmp(&reading(&b.score)))
        });
        for first in by_reading.into_iter().take(take) {
            if !chosen.contains(&first.down) {
                chosen.push(first.down);
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

/// The rotation that stands a model up with `down` pointing at the plate. Its facing about the
/// vertical is the shortest turn; arranging decides where it looks.
pub fn rotation_for(down: Vec3) -> Quat {
    down.try_normalize().map_or(Quat::IDENTITY, |unit| {
        Quat::from_rotation_arc(unit, Vec3::NEG_Z)
    })
}

#[cfg(test)]
mod tests;
