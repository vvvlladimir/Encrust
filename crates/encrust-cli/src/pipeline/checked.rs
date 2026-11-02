//! What import does to a mesh before it is placed: doubled faces dropped, winding fixed,
//! changes by hand, the repair asked for, and what was found.

use std::path::Path;

use anyhow::{Context, Result};
use core_engine::{PREPARE_REPAIR, hollowing, needs_repair};
use core_geometry::{
    Aabb, Mat3, MendSettings, Mended, Mesh, Welded, diagnose, list_shells, orient_outward,
    remove_duplicate_faces,
};
use core_volume::{Cancel, Progress, RepairSettings, VolumeError, repair};
use printer_profiles::PrinterProfile;

use crate::args::ImportArgs;
use crate::edits;
use crate::exit::Cancelled;
use crate::pipeline::Watch;
use crate::progress::bar;
use crate::report::{FitCheck, ImportReport, Wrapped};
use crate::stats::MeshStats;

/// Drops doubled faces, fixes the winding, makes the changes by hand and the repair asked
/// for, and says what import found; `with_shells` lists the shells as they came.
pub(super) fn checked(
    mesh: &mut Mesh,
    (input, welded): (&Path, Welded),
    args: &ImportArgs,
    profile: Option<&PrinterProfile>,
    with_shells: bool,
    watch: &Watch,
) -> Result<ImportReport> {
    // Before the orientation fix: a face laid over another reads as a branch to it.
    let doubled = if args.no_validate {
        0
    } else {
        remove_duplicate_faces(mesh).len()
    };
    let orientation = (!args.no_validate).then(|| orient_outward(mesh));
    let listed = with_shells.then(|| list_shells(mesh));
    let edited = edits::apply(mesh, args)?;
    let repaired = repair_asked(mesh, args)
        .map(|method| repaired(mesh, method, args, watch))
        .transpose()?;
    let (mended, wrapped) =
        repaired.map_or((None, None), |(mended, wrapped)| (Some(mended), wrapped));
    let diagnostics = (!args.no_validate).then(|| diagnose(mesh));
    let shells = diagnostics.as_ref().map(|found| found.shells);
    let stats = MeshStats::of(mesh, shells).context("mesh has no vertices")?;
    let bounds = Aabb::new(stats.min, stats.max);
    let fit = profile.map(|p| FitCheck::of(&bounds, p));
    let hollowing_saves = diagnostics
        .as_ref()
        .and_then(|found| hollowing(mesh, Mat3::IDENTITY, found));

    Ok(ImportReport {
        path: input.to_path_buf(),
        stats,
        welded,
        doubled,
        diagnostics,
        orientation,
        mended,
        wrapped,
        crossing_pairs: None,
        edited,
        shells: listed,
        fit,
        hollowing_saves,
    })
}

/// The repair `--repair` names, or under `--prepare` the autopilot's for a model with defects.
fn repair_asked(mesh: &Mesh, args: &ImportArgs) -> Option<core_volume::RepairMethod> {
    match args.repair {
        Some(method) => Some(method.into()),
        None => (args.prepare && !args.no_validate && needs_repair(&diagnose(mesh)))
            .then_some(PREPARE_REPAIR),
    }
}

/// Runs `method` on `mesh` in place: what it changed, and how a wrap went.
fn repaired(
    mesh: &mut Mesh,
    method: core_volume::RepairMethod,
    args: &ImportArgs,
    watch: &Watch,
) -> Result<(Mended, Option<Wrapped>)> {
    let settings = RepairSettings {
        mend: MendSettings {
            seam_tolerance_mm: args.seam_tolerance,
            smallest_shell_mm2: args.smallest_shell,
        },
    };
    let done = watched_repair(mesh, method, settings, watch)?;
    *mesh = done.mesh;
    let wrapped = method.wraps().then_some(if done.wrapped {
        Wrapped::Exactly
    } else {
        Wrapped::LeftOut
    });
    Ok((done.mended, wrapped))
}

/// The repair, with its share on a bar spanned as the window spans it (ADR 0384), and
/// Ctrl-C stopping it between steps; the bar is drawn where the repair asks whether to stop.
fn watched_repair(
    mesh: &Mesh,
    method: core_volume::RepairMethod,
    settings: RepairSettings,
    watch: &Watch,
) -> Result<core_volume::Repaired> {
    let progress = Progress::default();
    let shown = watch.progress.then(|| bar("% of the repair"));
    let asked = || {
        if let (Some(shown), Some((_, share))) = (&shown, progress.share()) {
            shown.set_length(100);
            shown.set_position((share * 100.0) as u64);
        }
        watch.stop.requested()
    };
    let done = repair(
        mesh,
        method,
        settings,
        Cancel::when(&asked).reporting(&progress),
    );
    if let Some(shown) = &shown {
        shown.finish_and_clear();
    }
    match done {
        Err(VolumeError::Cancelled) => Err(Cancelled.into()),
        done => done.context("could not repair the model"),
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid};

    use super::*;
    use crate::exit::{Stop, is_cancelled};

    #[test]
    fn a_repair_stopped_by_ctrl_c_ends_as_cancelled() {
        let cube = cuboid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let stop = Stop::default();
        stop.request();
        let wrapped = watched_repair(
            &cube,
            core_volume::RepairMethod::Wrap,
            RepairSettings::default(),
            &Watch::quiet(&stop),
        );
        let error = wrapped.expect_err("a stop asked for before the wrap stops it");
        assert!(
            is_cancelled(&error),
            "exit 130, not a failed repair: {error:#}"
        );
    }
}
