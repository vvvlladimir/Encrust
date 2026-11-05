//! The steps a recipe runs on one model at a time, each by the command line's own call for
//! it: `--orient`'s search, `--repair`, `--hollow`, `--drain-at` and `--supports`.

use anyhow::{Result, bail};
use clap::ValueEnum;
use core_engine::{PREPARE_REPAIR, SupportPlan};
use core_geometry::{Mat3, Scalar, Transform};
use core_plate::{OrientGoal, OrientPreset, OrientSettings};
use core_recipe::{Metric, StepKind};
use core_volume::{HollowSettings, hollowing_saves_mm3, placed_volume_and_area, raw_saving_mm3};

use super::piece::Piece;
use super::prepared::{Wall, profile_name};
use super::settings::{Settings, flag, number, word};
use super::{Ctx, Did, drain};
use crate::args::RepairMethod;
use crate::hollowing::{self, HollowArgs, Mode, Pattern};
use crate::pipeline::{orientation, read_again, repaired_later, turn_in_place};
use crate::profiles::{AUTO, plan_of};
use crate::supports::stand_under;

/// A turn this small, degrees, leaves the model as it stood.
const UNTURNED_DEG: Scalar = 1e-3;

/// Runs step `kind`, which works on one model, on `piece` with `settings`.
pub(super) fn run(
    kind: StepKind,
    piece: &mut Piece,
    settings: &Settings,
    ctx: &Ctx,
) -> Result<Did> {
    match kind {
        StepKind::Orient => orient(piece, settings, ctx),
        StepKind::Repair => repair(piece, settings, ctx),
        StepKind::Hollow => hollow(piece, settings, ctx),
        StepKind::Drain => drain::drain(piece, settings, ctx),
        StepKind::Supports => supports(piece, settings, ctx),
        StepKind::Check | StepKind::Arrange | StepKind::Save | StepKind::Gate | StepKind::Pause => {
            Ok(Did::default())
        }
    }
}

/// The search `--orient` runs, judged on the supports the recipe stands it on; supports
/// already under it are taken off.
fn orient(piece: &mut Piece, settings: &Settings, ctx: &Ctx) -> Result<Did> {
    let preset = word(settings, "goal").map_or(OrientSettings::default().preset, preset_of);
    let asked = (preset, number(settings, "max_tilt_deg"));
    piece.take_off_supports();
    piece.held.clear();
    let judged = ctx.support_plan(piece)?;
    let found = orientation(&piece.mesh, asked, judged.as_ref(), ctx.chosen, ctx.watch)?;
    let turn = turn_in_place(&piece.mesh, found.rotation)?;
    piece.moved(turn);
    if ctx.watch.talk {
        println!("Oriented: {}\n", found.describe());
    }
    piece.turned |= found.degrees().abs() > UNTURNED_DEG;
    piece.seated = found.on_plate;
    piece.oriented = Some(found);
    piece.prepared.oriented = true;
    read_again(&mut piece.import, &piece.mesh, ctx.chosen.printer.as_ref())?;
    Ok(Did::default())
}

fn preset_of(goal: &str) -> OrientPreset {
    match goal {
        "fewest-supports" => OrientGoal::FewestSupports.into(),
        "least-peel" => OrientGoal::LeastPeel.into(),
        "shortest-print" => OrientGoal::ShortestPrint.into(),
        _ => OrientPreset::Auto,
    }
}

/// `--repair` by the step's method; a shell repaired is remeshed, so its cavity is not known
/// any more.
fn repair(piece: &mut Piece, settings: &Settings, ctx: &Ctx) -> Result<Did> {
    let method = match word(settings, "method") {
        Some(name) => RepairMethod::from_str(name, true)
            .map_err(|why| anyhow::anyhow!("no repair method `{name}`: {why}"))?
            .into(),
        None => PREPARE_REPAIR,
    };
    let asked = (method, &piece.shaping.import);
    repaired_later(&mut piece.import, &mut piece.mesh, asked, ctx.watch)?;
    piece.cavity = None;
    piece.prepared.repaired = true;
    read_again(&mut piece.import, &piece.mesh, ctx.chosen.printer.as_ref())?;
    if ctx.watch.talk {
        print!("{}", piece.import);
    }
    Ok(Did::default())
}

/// `--hollow` at the step's wall, mode and infill, keeping a wall round the holes drilled
/// before it.
fn hollow(piece: &mut Piece, settings: &Settings, ctx: &Ctx) -> Result<Did> {
    if piece.cavity.is_some() {
        return Ok(Did::nothing("the model is hollow already"));
    }
    // TODO(step-6): keep_solid_under_mm as core_volume's Keep solid, fitted to the model.
    if settings.contains_key("keep_solid_under_mm") {
        bail!("keep_solid_under_mm is not supported yet");
    }
    let wall_mm = number(settings, "wall_mm").unwrap_or(HollowSettings::default().thickness_mm);
    let (mode, open_floor) = match word(settings, "mode") {
        Some("outside") => (Mode::External, false),
        Some("open-floor") => (Mode::Internal, true),
        _ => (Mode::Internal, false),
    };
    let typed = &piece.shaping.hollow;
    let args = HollowArgs {
        thickness_mm: Some(wall_mm),
        mode,
        open_floor,
        infill: flag(settings, "infill")
            .unwrap_or(false)
            .then(|| typed.infill.unwrap_or(Pattern::Hive)),
        ..typed.clone()
    };
    let saves_mm3 = saves_mm3(piece, wall_mm);
    let placed = (piece.holes.clone(), Vec::new());
    let precision = piece.shaping.import.precision;
    let (shell, cavity, report) =
        hollowing::run(&piece.mesh, &placed, &args, precision, ctx.watch.stop)?;
    if ctx.watch.talk {
        print!("Hollowed\n{report}\n");
    }
    piece.mesh = shell;
    piece.cavity = Some(cavity);
    piece.wall = Some(report.wall());
    piece.hollow = Some(report);
    piece.prepared.hollowed = Some(Wall {
        wall_mm,
        saves_mm3: typed.thickness_mm.is_none().then_some(saves_mm3),
    });
    Ok(Did::default())
}

/// What ADR 0393 reckons hollowing `piece` at `wall_mm` saves, mm³, the plain estimate where
/// the saving is too small to name.
fn saves_mm3(piece: &Piece, wall_mm: Scalar) -> Scalar {
    hollowing_saves_mm3(&piece.mesh, Mat3::IDENTITY, wall_mm).unwrap_or_else(|| {
        let (volume_mm3, area_mm2) = placed_volume_and_area(&piece.mesh, Mat3::IDENTITY);
        raw_saving_mm3(volume_mm3, area_mm2, wall_mm).max(0.0) as Scalar
    })
}

/// `--supports` to the step's profile, inside the cavity too where it asks; the supports of
/// an earlier step are taken off first, and a model the search seated gets none it needs not.
fn supports(piece: &mut Piece, settings: &Settings, ctx: &Ctx) -> Result<Did> {
    let mut plan = match &piece.shaping.supports {
        Some(plan) => plan.clone(),
        None => plan_of(word(settings, "profile").unwrap_or(AUTO))?,
    };
    plan.cavity = flag(settings, "cavity").unwrap_or(false);
    piece.take_off_supports();
    piece.held.clear();
    piece.prepared.supports = Some(profile_name(&plan));
    stand(piece, plan, ctx)?;
    Ok(Did::default())
}

/// Stands `piece` on supports by `plan`, holding up its held islands, from where it stands
/// with none under it.
pub(super) fn stand(piece: &mut Piece, plan: SupportPlan, ctx: &Ctx) -> Result<()> {
    let mut cuts = piece.cuts()?;
    let model = (&mut piece.mesh, piece.cavity.clone(), &mut cuts);
    let seated = piece.seated;
    let stood = stand_under(model, &plan, ctx.layer_height_mm, seated, &piece.held)?;
    piece.plan = Some(plan);
    if seated && stood.0.contacts.is_empty() {
        return Ok(());
    }
    let up = Transform::from_translation(core_geometry::Vec3::Z * stood.0.lifted_mm);
    for hole in &mut piece.holes {
        hole.at = up.to_matrix().transform_point3(hole.at);
    }
    if ctx.watch.talk {
        print!("{}", stood.0);
    }
    piece.supports = Some(stood);
    Ok(())
}

/// Notes the islands, pull and support resin a check measured on `piece`.
pub(super) fn checked(piece: &mut Piece, islands: usize, peel_n: Scalar) {
    piece.note(Metric::Islands, islands as Scalar);
    piece.note(Metric::PeelN, peel_n);
    let support_ml = piece.support_ml();
    piece.note(Metric::SupportMl, support_ml);
}
