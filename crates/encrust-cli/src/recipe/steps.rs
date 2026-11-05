//! The steps a recipe runs on one model at a time, each by the command line's own call for
//! it: `--orient`'s search, `--repair`, `--hollow`, `--drain-at` and `--supports`.

use anyhow::Result;
use clap::ValueEnum;
use core_engine::{Hollowing, PREPARE_REPAIR, SupportPlan};
use core_geometry::{Mat3, Scalar, Transform, Vec3};
use core_plate::OrientSettings;
use core_recipe::{Metric, StepKind, preset_of};
use core_volume::HollowSettings;

use super::piece::Piece;
use super::prepared::{Wall, profile_name};
use super::settings::Settings;
use super::{Ctx, Did, drain};
use crate::args::RepairMethod;
use crate::hollowing::{self, HollowArgs, Mode, Pattern};
use crate::keep_solid::KeepSolidArgs;
use crate::pipeline::{orientation, read_again, repaired_later, turn_in_place};
use crate::profiles::{AUTO, plan_of};
use crate::supports::{stand_at, stand_under};
use core_recipe::resolved::{flag, number, text};

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
    let preset = text(settings, "goal").map_or(OrientSettings::default().preset, preset_of);
    let asked = (preset, number(settings, "max_tilt_deg"));
    piece.take_off_supports();
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

/// `--repair` by the step's method; a shell repaired is remeshed, so its cavity is not known
/// any more.
fn repair(piece: &mut Piece, settings: &Settings, ctx: &Ctx) -> Result<Did> {
    let method = match text(settings, "method") {
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

/// `--hollow` at the step's wall, mode, infill and thinnest part hollowed, keeping a wall
/// round the holes drilled before it.
fn hollow(piece: &mut Piece, settings: &Settings, ctx: &Ctx) -> Result<Did> {
    if piece.cavity.is_some() {
        return Ok(Did::nothing("the model is hollow already"));
    }
    let wall_mm = number(settings, "wall_mm").unwrap_or(HollowSettings::default().thickness_mm);
    let (mode, open_floor) = match text(settings, "mode") {
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
        keep_solid: KeepSolidArgs {
            under_mm: number(settings, "keep_solid_under_mm"),
            ..typed.keep_solid.clone()
        },
        ..typed.clone()
    };
    let saves_mm3 = Hollowing::reckoned(&piece.mesh, Mat3::IDENTITY, wall_mm).saves_mm3;
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

/// `--supports` to the step's profile, inside the cavity too where it asks; the supports of
/// an earlier step are taken off first, and a model the search seated gets none it needs not.
fn supports(piece: &mut Piece, settings: &Settings, ctx: &Ctx) -> Result<Did> {
    let mut plan = match &piece.shaping.supports {
        Some(plan) => plan.clone(),
        None => plan_of(text(settings, "profile").unwrap_or(AUTO))?,
    };
    plan.cavity = flag(settings, "cavity").unwrap_or(false);
    piece.take_off_supports();
    piece.prepared.supports = Some(profile_name(&plan));
    stand(piece, plan, ctx)?;
    Ok(Did::default())
}

/// Stands `piece` on supports by `plan`, from where it stands with none under it.
fn stand(piece: &mut Piece, plan: SupportPlan, ctx: &Ctx) -> Result<()> {
    let mut cuts = piece.cuts()?;
    let model = (&mut piece.mesh, piece.cavity.clone(), &mut cuts);
    let seated = piece.seated;
    let stood = stand_under(model, &plan, ctx.layer_height_mm, seated)?;
    piece.plan = Some(plan);
    if seated && stood.0.contacts.is_empty() {
        return Ok(());
    }
    let up = Transform::from_translation(Vec3::Z * stood.0.lifted_mm);
    for hole in &mut piece.holes {
        hole.at = up.to_matrix().transform_point3(hole.at);
    }
    if ctx.watch.talk {
        print!("{}", stood.0);
    }
    piece.supports = Some(stood);
    Ok(())
}

/// A support by `plan` under each of `contacts`, plate mm, added to those `piece` stands on.
pub(super) fn hold_up(piece: &mut Piece, plan: &SupportPlan, contacts: &[Vec3]) -> Result<()> {
    let mut cuts = piece.cuts()?;
    let model = (&mut piece.mesh, piece.cavity.clone(), &mut cuts);
    let (report, mesh) = stand_at(model, plan, contacts)?;
    match &mut piece.supports {
        Some((standing, under)) => {
            standing.add(report);
            under.append(&mesh);
        }
        None => piece.supports = Some((report, mesh)),
    }
    Ok(())
}

/// Notes the islands, pull and support resin a check measured on `piece`.
pub(super) fn checked(piece: &mut Piece, islands: usize, peel_n: Scalar) {
    piece.note(Metric::Islands, islands as Scalar);
    piece.note(Metric::PeelN, peel_n);
    let support_ml = piece.support_ml();
    piece.note(Metric::SupportMl, support_ml);
}
