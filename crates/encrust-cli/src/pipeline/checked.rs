//! What import does to a mesh before it is placed: doubled faces dropped, winding fixed,
//! changes by hand, the repair asked for, and what was found.

use std::path::Path;

use anyhow::{Context, Result};
use core_geometry::{
    MendSettings, Mended, Mesh, Welded, diagnose, list_shells, orient_outward,
    remove_duplicate_faces,
};
use core_volume::{Cancel, RepairSettings, repair};
use printer_profiles::PrinterProfile;

use crate::args::{ImportArgs, RepairMethod};
use crate::edits;
use crate::report::{FitCheck, ImportReport, Wrapped};
use crate::stats::MeshStats;

/// Drops doubled faces, fixes the winding, makes the changes by hand and the repair asked
/// for, and says what import found; `with_shells` lists the shells as they came.
pub(super) fn checked(
    mesh: &mut Mesh,
    input: &Path,
    welded: Welded,
    args: &ImportArgs,
    profile: Option<&PrinterProfile>,
    with_shells: bool,
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
    let repaired = args
        .repair
        .map(|method| repaired(mesh, method, args))
        .transpose()?;
    let (mended, wrapped) =
        repaired.map_or((None, None), |(mended, wrapped)| (Some(mended), wrapped));
    let diagnostics = (!args.no_validate).then(|| diagnose(mesh));
    let shells = diagnostics.as_ref().map(|found| found.shells);
    let stats = MeshStats::of(mesh, shells).context("mesh has no vertices")?;
    let fit = profile.map(|p| FitCheck::of(&stats, p));

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
    })
}

/// Runs the repair `--repair` named on `mesh` in place: what it changed, and how a wrap went.
fn repaired(
    mesh: &mut Mesh,
    method: RepairMethod,
    args: &ImportArgs,
) -> Result<(Mended, Option<Wrapped>)> {
    let settings = RepairSettings {
        mend: MendSettings {
            seam_tolerance_mm: args.seam_tolerance,
            smallest_shell_mm2: args.smallest_shell,
        },
    };
    let method = core_volume::RepairMethod::from(method);
    let done =
        repair(mesh, method, settings, Cancel::never()).context("could not repair the model")?;
    *mesh = done.mesh;
    let wrapped = method.wraps().then_some(if done.wrapped {
        Wrapped::Exactly
    } else {
        Wrapped::LeftOut
    });
    Ok((done.mended, wrapped))
}
