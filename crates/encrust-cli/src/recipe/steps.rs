//! The steps a recipe runs on one model at a time, each the edits `core_recipe::edits_of` makes
//! of it carried out on the plate's document, with what the flags give laid over (ADR 0470, 0472).

use std::time::Instant;

use anyhow::{Context, Result, bail};
use core_document::{ApplyError, Report};
use core_edit::{AUTO, Edit};
use core_engine::{Drained, Hollowing, SupportPlan};
use core_geometry::{Aabb, Mat3, Scalar, Vec3};
use core_recipe::{Metric, StepKind, edits_of};
use core_supports::Unstood;
use core_volume::HoleKind;

use super::piece::Piece;
use super::prepared::{Wall, profile_name};
use super::settings::Settings;
use super::{Ctx, Did};
use crate::hollowing::{AutoReport, HollowReport};
use crate::pipeline::{OrientSummary, read_again};
use crate::profiles::plan_of;
use crate::stage::{self, Staging, said, shipped_printer, summary_of, support_report, usable_area};
use core_recipe::resolved::text;

/// A turn smaller than this, degrees, leaves the model as the user stood it.
const UNTURNED_DEG: Scalar = 1e-3;

/// Runs step `kind` on `piece` by `settings`.
pub(super) fn run(
    kind: StepKind,
    (piece, staging): (&mut Piece, &mut Staging),
    settings: &Settings,
    ctx: &Ctx,
) -> Result<Did> {
    if kind == StepKind::Hollow && staging.hollowed(piece.id) {
        return Ok(Did::nothing("the model is hollow already"));
    }
    if kind == StepKind::Supports {
        hold_profile(piece, staging, settings)?;
    }
    let placed = (staging.mesh(piece.id).and_then(|mesh| mesh.aabb()))
        .context("the model has no surface")?;
    let area = ctx
        .chosen
        .printer
        .as_ref()
        .map_or_else(|| whole_plate(placed), usable_area);
    let drift_mm = ctx.chosen.material.compensation.fit_drift_mm();
    let edits = edits_of(kind, settings, piece.id, placed, area, drift_mm)
        .context("the recipe's step names no edit")?;
    let mut did = Did::default();
    for edit in edits {
        let edit = typed(edit, piece, (staging, ctx));
        if let Edit::Drain { size, .. } = &edit
            && size.diameter_mm <= 0.0
        {
            bail!("a drain hole of no width cannot be drilled");
        }
        if let Edit::Hollow { settings, .. } = &edit {
            piece.prepared.hollowed = wall_of(settings.thickness_mm, staging, piece);
        }
        let faces = staging.mesh(piece.id).map_or(0, |mesh| mesh.faces.len());
        let started = Instant::now();
        let applied = match staging.apply(&edit, piece.id, ctx.watch.stop) {
            Err(ApplyError::NoCavity(_)) => {
                piece.note(Metric::Trapped, 0.0);
                return Ok(Did::nothing("no cavity to drain"));
            }
            Err(ApplyError::NoRoom(_)) => {
                piece.hollow =
                    hollow_report(&edit, staging, piece, (faces, (0.0, 0.0, false)), started);
                continue;
            }
            applied => applied.map_err(said)?,
        };
        let asked = (&edit, faces, started);
        noted(piece, staging, (asked, applied.report), ctx, &mut did)?;
    }
    Ok(did)
}

/// The plate as wide as a model's own box where no printer names one.
fn whole_plate(placed: Aabb) -> ([Scalar; 2], [Scalar; 2]) {
    let size = placed.maxs - placed.mins;
    ([placed.mins.x, placed.mins.y], [size.x, size.y])
}

/// The profile a supports step names, put on the plate as the group in hand where it is the
/// user's or a flag's, so the edit builds to it.
fn hold_profile(piece: &Piece, staging: &mut Staging, settings: &Settings) -> Result<()> {
    let plan = match &piece.shaping.supports {
        Some(plan) => plan.clone(),
        None => plan_of(text(settings, "profile").unwrap_or(AUTO))?,
    };
    if plan.sizing().is_none() {
        staging.hold_profile(plan.placing());
    }
    Ok(())
}

/// `edit` with what the flags give that no step setting holds: the lattice, the infill, the
/// places kept solid, the hole and the plug gap typed, the printer a pose is proved on.
fn typed(mut edit: Edit, piece: &Piece, (staging, ctx): (&Staging, &Ctx)) -> Edit {
    let (import, hollow) = (&piece.shaping.import, &piece.shaping.hollow);
    match &mut edit {
        Edit::Hollow {
            settings,
            keep_solid,
            keep_solid_reach_mm,
            ..
        } => {
            settings.precision = import.precision;
            if let Some(state) = hollow
                .state(import.precision)
                .filter(|_| settings.infill_on)
            {
                settings.infill = state.infill;
            }
            *keep_solid = hollow.keep_solid.runs.clone();
            *keep_solid_reach_mm = hollow.keep_solid.reach_mm;
        }
        Edit::Drain {
            object, size, kind, ..
        } => {
            *kind = match staging.hollowed(*object) {
                true => HoleKind::AtDepth,
                false => HoleKind::Walled,
            };
            *size = core_volume::HoleSize {
                diameter_mm: size.diameter_mm,
                depth_mm: size.depth_mm,
                ..hollow.hole_size()
            };
        }
        Edit::Plugs { gap_mm, .. } => {
            *gap_mm = hollow.plug_gap_mm - ctx.chosen.material.compensation.fit_drift_mm();
        }
        Edit::Orient {
            printer, lift_mm, ..
        } => {
            let job = &ctx.job.profile;
            *printer = shipped_printer(job.printer.as_deref(), job.profile.is_some());
            // The lift asked stays the model's, as the window keeps the row's for its orient.
            *lift_mm = import.transform.lift.or(*lift_mm);
        }
        Edit::Repair {
            seam_tolerance_mm,
            smallest_shell_mm2,
            ..
        } => {
            *seam_tolerance_mm = Some(import.seam_tolerance);
            *smallest_shell_mm2 = Some(import.smallest_shell);
        }
        _ => {}
    }
    edit
}

/// Notes on `piece` what one of its step's edits did, said as the flag's own run says it.
fn noted(
    piece: &mut Piece,
    staging: &mut Staging,
    ((edit, faces, started), report): ((&Edit, usize, Instant), Report),
    ctx: &Ctx,
    did: &mut Did,
) -> Result<()> {
    let talk = ctx.watch.talk;
    match report {
        Report::Repaired { mended, wrapped } => {
            let wraps = matches!(edit, Edit::Repair { method, .. } if method.wraps());
            repaired(piece, staging, (mended, wraps.then_some(wrapped)), ctx)?;
        }
        report @ Report::Oriented { .. } => oriented(piece, staging, report, ctx)?,
        Report::Hollowed {
            cavity_mm3,
            voxel_mm,
            coarsened,
        } => {
            let lattice = (cavity_mm3, voxel_mm, coarsened);
            piece.hollow = hollow_report(edit, staging, piece, (faces, lattice), started);
            if let (Some(report), true) = (&piece.hollow, talk) {
                print!("Hollowed\n{report}\n");
            }
        }
        Report::Drained {
            holes,
            trapped,
            searched,
        } => drained(piece, staging, (holes, trapped, searched), (ctx, did))?,
        Report::Supported {
            trees,
            contacts_mm,
            standing,
            unheld,
            ..
        } => {
            let parts = (trees, contacts_mm, standing, unheld);
            supported(piece, staging, edit, parts, talk);
        }
        _ => {}
    }
    Ok(())
}

/// Notes the holes a drain edit drilled and what it left trapped, searched or scanned.
fn drained(
    piece: &mut Piece,
    staging: &Staging,
    (holes, trapped, searched): (usize, usize, Option<Drained>),
    (ctx, did): (&Ctx, &mut Did),
) -> Result<()> {
    did.holes = Some(did.holes.unwrap_or(0) + holes);
    if let (Some(drained), true) = (&searched, ctx.watch.talk) {
        print!("{}", AutoReport(drained, 0.0));
    }
    if searched.is_some() {
        piece.prepared.drained = true;
        piece.note(Metric::Trapped, trapped as Scalar);
        piece.drains = searched;
    } else {
        let trapped = piece.trapped(staging, ctx.watch.stop)?;
        piece.note(Metric::Trapped, trapped as Scalar);
    }
    Ok(())
}

/// Notes the supports a supports edit grew and the profile it named.
fn supported(
    piece: &mut Piece,
    staging: &Staging,
    edit: &Edit,
    parts: (usize, Vec<Vec3>, usize, Vec<(Vec3, Unstood)>),
    talk: bool,
) {
    let report = support_report(staging, piece.id, parts);
    if talk {
        print!("{report}");
    }
    if let Edit::Supports { profile, .. } = edit {
        let named = profile.clone().unwrap_or_else(|| AUTO.to_owned());
        piece.prepared.supports = Some(plan_named(&named));
        piece.supported_by = Some(named);
    }
    piece.supports = Some(report);
}

/// Notes a repair on `piece`'s import, as `--repair` notes it; `wrapped`, whether a wrap held.
fn repaired(
    piece: &mut Piece,
    staging: &Staging,
    (mended, wrapped): (core_geometry::Mended, Option<bool>),
    ctx: &Ctx,
) -> Result<()> {
    piece.import.mended = Some(mended);
    piece.import.wrapped = wrapped.map(|held| match held {
        true => crate::report::Wrapped::Exactly,
        false => crate::report::Wrapped::LeftOut,
    });
    piece.prepared.repaired = true;
    reread(piece, staging, ctx.chosen.printer.as_ref())?;
    if ctx.watch.talk {
        print!("{}", piece.import);
    }
    Ok(())
}

/// Notes the pose the search stood `piece` in, and whether it turned the model.
fn oriented(piece: &mut Piece, staging: &Staging, report: Report, ctx: &Ctx) -> Result<()> {
    let transform = &ctx.job.import.transform;
    let found: OrientSummary = summary_of(report, transform, piece.stands)?;
    if ctx.watch.talk {
        println!("Oriented: {}\n", found.describe());
    }
    piece.turned |= found.degrees().abs() > UNTURNED_DEG;
    piece.seated = found.on_plate;
    piece.oriented = Some(found);
    piece.prepared.oriented = true;
    reread(piece, staging, ctx.chosen.printer.as_ref())
}

/// What import read off `piece`, read again where a step turned or mended it.
fn reread(
    piece: &mut Piece,
    staging: &Staging,
    printer: Option<&printer_profiles::PrinterProfile>,
) -> Result<()> {
    match staging.mesh(piece.id) {
        Some(mesh) => read_again(&mut piece.import, &mesh, printer),
        None => Ok(()),
    }
}

/// The wall `piece` is hollowed at, and what ADR 0393 reckons it saves of the model as it
/// stands, said where the recipe chose the wall.
fn wall_of(wall_mm: Scalar, staging: &Staging, piece: &Piece) -> Option<Wall> {
    let mesh = staging.mesh(piece.id)?;
    let saves_mm3 = Hollowing::reckoned(&mesh, Mat3::IDENTITY, wall_mm).saves_mm3;
    let chosen = piece.shaping.hollow.thickness_mm.is_none();
    Some(Wall {
        wall_mm,
        saves_mm3: chosen.then_some(saves_mm3),
    })
}

/// The report of a hollow `edit` asked, as `--hollow` says it.
fn hollow_report(
    edit: &Edit,
    staging: &Staging,
    piece: &Piece,
    (faces, lattice): (usize, (Scalar, Scalar, bool)),
    started: Instant,
) -> Option<HollowReport> {
    let Edit::Hollow { settings, .. } = edit else {
        return None;
    };
    staging.mesh(piece.id)?;
    Some(stage::hollow_report(
        staging,
        piece.id,
        settings,
        (faces, lattice),
        started,
    ))
}

/// The name `--prepare`'s report gives the supports built to `profile`.
fn plan_named(profile: &str) -> String {
    match plan_of(profile) {
        Ok(plan) => profile_name(&plan),
        Err(_) => profile.to_owned(),
    }
}

/// Every island, lever and neck a check holds up on `piece`, at `contacts` plate mm, to
/// `plan` put on the plate as the group in hand.
pub(super) fn hold_up(
    (piece, staging): (&mut Piece, &mut Staging),
    plan: &SupportPlan,
    contacts: &[Vec3],
    ctx: &Ctx,
) -> Result<()> {
    staging.hold_profile(plan.placing());
    let edit = Edit::Supports {
        object: piece.id,
        profile: Some(plan.placing().name),
        at_mm: contacts.to_vec(),
        only_at: true,
        cavity: false,
        shown_at: Vec::new(),
        clean_surface: false,
    };
    let applied = staging
        .apply(&edit, piece.id, ctx.watch.stop)
        .map_err(said)?;
    if let Report::Supported {
        trees,
        contacts_mm,
        standing,
        unheld,
        ..
    } = applied.report
    {
        let parts = (trees, contacts_mm, standing, unheld);
        piece.supports = Some(support_report(staging, piece.id, parts));
    }
    Ok(())
}

/// What a check measured on `piece`: its islands, its hardest pull and its supports' resin.
pub(super) fn checked(piece: &mut Piece, staging: &Staging, islands: usize, peel_n: Scalar) {
    piece.note(Metric::Islands, islands as Scalar);
    piece.note(Metric::PeelN, peel_n);
    piece.note(Metric::SupportMl, staging.support_ml(piece.id));
}
