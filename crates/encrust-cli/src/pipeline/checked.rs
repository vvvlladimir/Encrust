//! What import does to a mesh before it is placed: doubled faces dropped, winding fixed, and
//! what was found; the changes by hand and the repair are edits (`stage/repaired.rs`).

use std::path::Path;

use anyhow::{Context, Result};
use core_document::ApplyError;
use core_engine::hollowing;
use core_geometry::{
    Aabb, Mat3, Mesh, MeshDiagnostics, Welded, diagnose, orient_outward, remove_duplicate_faces,
};
use core_volume::{Cancel, Progress};
use printer_profiles::PrinterProfile;

use crate::args::ImportArgs;
use crate::exit::Cancelled;
use crate::pipeline::Watch;
use crate::progress::bar;
use crate::report::{FitCheck, ImportReport};
use crate::stats::MeshStats;

/// Drops doubled faces, fixes the winding and says what import found.
pub(super) fn checked(
    mesh: &mut Mesh,
    (input, welded): (&Path, Welded),
    args: &ImportArgs,
    profile: Option<&PrinterProfile>,
) -> Result<ImportReport> {
    // Before the orientation fix: a face laid over another reads as a branch to it.
    let doubled = if args.no_validate {
        0
    } else {
        remove_duplicate_faces(mesh).len()
    };
    let orientation = (!args.no_validate).then(|| orient_outward(mesh));
    let facts = Facts::of(mesh, !args.no_validate, profile)?;
    Ok(ImportReport {
        path: input.to_path_buf(),
        stats: facts.stats,
        welded,
        doubled,
        diagnostics: facts.diagnostics,
        orientation,
        mended: None,
        wrapped: None,
        crossing_pairs: None,
        edited: None,
        shells: None,
        fit: facts.fit,
        hollowing_saves: facts.hollowing_saves,
    })
}

/// What import reads off a mesh as it stands: its size, what is wrong with it when
/// `validated`, how it fits `profile` and what hollowing it would save.
struct Facts {
    stats: MeshStats,
    diagnostics: Option<MeshDiagnostics>,
    fit: Option<FitCheck>,
    hollowing_saves: Option<core_engine::Hollowing>,
}

impl Facts {
    fn of(mesh: &Mesh, validated: bool, profile: Option<&PrinterProfile>) -> Result<Self> {
        let diagnostics = validated.then(|| diagnose(mesh));
        let shells = diagnostics.as_ref().map(|found| found.shells);
        let stats = MeshStats::of(mesh, shells).context("mesh has no vertices")?;
        let bounds = Aabb::new(stats.min, stats.max);
        let hollowing_saves = diagnostics
            .as_ref()
            .and_then(|found| hollowing(mesh, Mat3::IDENTITY, found));
        Ok(Self {
            stats,
            diagnostics,
            fit: profile.map(|p| FitCheck::of(&bounds, p)),
            hollowing_saves,
        })
    }
}

/// `import` read again off `mesh` once a later step turned or mended it.
pub(crate) fn read_again(
    import: &mut ImportReport,
    mesh: &Mesh,
    profile: Option<&PrinterProfile>,
) -> Result<()> {
    let facts = Facts::of(mesh, import.diagnostics.is_some(), profile)?;
    import.stats = facts.stats;
    import.diagnostics = facts.diagnostics;
    import.fit = facts.fit;
    import.hollowing_saves = facts.hollowing_saves;
    Ok(())
}

/// `run` given a stop that also draws its share on a bar labelled `what`, where a run talks.
pub(crate) fn with_bar<T>(watch: &Watch, what: &str, run: impl FnOnce(Cancel<'_>) -> T) -> T {
    let progress = Progress::default();
    let shown = watch.progress.then(|| bar(what));
    let asked = || {
        if let (Some(shown), Some((_, share))) = (&shown, progress.share()) {
            shown.set_length(100);
            shown.set_position((share * 100.0) as u64);
        }
        watch.stop.requested()
    };
    let done = run(Cancel::when(&asked).reporting(&progress));
    if let Some(shown) = &shown {
        shown.finish_and_clear();
    }
    done
}

/// Why a repair did not finish, as the command line says it.
pub(crate) fn repair_refused(error: ApplyError) -> anyhow::Error {
    match error {
        ApplyError::Cancelled => Cancelled.into(),
        ApplyError::Failed { reason, .. } => {
            anyhow::anyhow!(reason).context("could not repair the model")
        }
        other => anyhow::Error::new(other).context("could not repair the model"),
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid};

    use core_document::repaired as repaired_by;
    use core_edit::ObjectId;

    use super::*;
    use crate::exit::{Stop, is_cancelled};

    #[test]
    fn a_repair_stopped_by_ctrl_c_ends_as_cancelled() {
        let cube = cuboid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let stop = Stop::default();
        stop.request();
        let wrapped = with_bar(&Watch::quiet(&stop), "% of the repair", |cancel| {
            let asked = (core_volume::RepairMethod::Wrap, Default::default());
            repaired_by(&cube, asked, cancel, ObjectId::UNSET)
        });
        let error = wrapped
            .map_err(repair_refused)
            .expect_err("a stop asked for before the wrap stops it");
        assert!(
            is_cancelled(&error),
            "exit 130, not a failed repair: {error:#}"
        );
    }
}
