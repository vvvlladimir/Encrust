//! The changes by hand and the repair before an entry's object is placed, a picture's relief
//! after, as edits noted on its import report (ADR 0472).

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Result, anyhow};
use core_document::{ApplyError, Report};
use core_edit::{Edit, ObjectId};
use core_geometry::list_shells;

use super::document::{Staging, said};
use super::{Found, Shaping};
use crate::args::ImportArgs;
use crate::edits::{mend_asked, refused};
use crate::pipeline::{Watch, repair_refused, with_repair_bar};
use crate::report::{ImportReport, Wrapped};

/// What import leaves to edits: the changes by hand, the repair and a picture's relief.
pub(super) fn left_to_edits(args: &mut ImportArgs) {
    args.repair = None;
    args.flip_faces.clear();
    args.drop_faces.clear();
    args.drop_shells.clear();
    args.bridge.clear();
    if args.relief_image.is_some() {
        args.relief = None;
    }
}

/// Mends and repairs each entry's one object as its flags ask, before it is placed.
pub(super) fn repaired(
    staging: &mut Staging,
    found: &mut [Found],
    entries: &[(PathBuf, Shaping)],
    watch: &Watch,
) -> Result<()> {
    for (entry, ((import, _), (_, shaping))) in found.iter_mut().zip(entries).enumerate() {
        let id = ObjectId(entry as u64 + 1);
        mend(staging, id, &shaping.import, import, watch)?;
        repair(staging, id, &shaping.import, import, watch)?;
    }
    Ok(())
}

/// Presses each entry's picture into its object where its flags ask, once it is placed.
pub(super) fn pressed(
    staging: &mut Staging,
    entries: &[(PathBuf, Shaping)],
    watch: &Watch,
) -> Result<()> {
    for (entry, (_, shaping)) in entries.iter().enumerate() {
        relief(staging, ObjectId(entry as u64 + 1), &shaping.import, watch)?;
    }
    Ok(())
}

/// Whether the flags edit the object before modelling, so what import read is read again.
pub(super) fn reshaped(args: &ImportArgs) -> bool {
    let transform = &args.transform;
    args.repair.is_some()
        || !mend_asked(args).is_empty()
        || args.relief_image.is_some()
        || transform.scale.is_some()
        || transform.rotate.is_some()
        || transform.center
        || transform.orient
}

fn mend(
    staging: &mut Staging,
    object: ObjectId,
    args: &ImportArgs,
    import: &mut ImportReport,
    watch: &Watch,
) -> Result<()> {
    let asked = mend_asked(args);
    if asked.is_empty() {
        return Ok(());
    }
    let counts = staging
        .mesh(object)
        .map_or((0, 0), |mesh| (mesh.faces.len(), list_shells(&mesh).len()));
    let edit = Edit::Mend {
        object,
        flip_faces: asked.flip_faces,
        drop_faces: asked.drop_faces,
        drop_shells: asked.drop_shells,
        bridge: asked.bridge,
    };
    let applied = (staging.apply(&edit, object, watch.stop)).map_err(|error| match error {
        ApplyError::Cancelled => said(error),
        other => refused(&other, counts, args),
    })?;
    if let Report::Mended(edited) = applied.report {
        import.edited = Some(edited);
    }
    Ok(())
}

fn repair(
    staging: &mut Staging,
    object: ObjectId,
    args: &ImportArgs,
    import: &mut ImportReport,
    watch: &Watch,
) -> Result<()> {
    let Some(method) = args.repair else {
        return Ok(());
    };
    let method: core_volume::RepairMethod = method.into();
    let edit = Edit::Repair {
        object,
        method,
        seam_tolerance_mm: Some(args.seam_tolerance),
        smallest_shell_mm2: Some(args.smallest_shell),
    };
    let applied = with_repair_bar(watch, |cancel| staging.apply_with(&edit, object, cancel))
        .map_err(repair_refused)?;
    if let Report::Repaired { mended, wrapped } = applied.report {
        import.mended = Some(mended);
        import.wrapped = method.wraps().then_some(match wrapped {
            true => Wrapped::Exactly,
            false => Wrapped::LeftOut,
        });
    }
    Ok(())
}

fn relief(staging: &mut Staging, object: ObjectId, args: &ImportArgs, watch: &Watch) -> Result<()> {
    let (Some(depth_mm), Some(image)) = (args.relief, args.relief_image.clone()) else {
        return Ok(());
    };
    let started = Instant::now();
    let faces = staging.mesh(object).map_or(0, |mesh| mesh.faces.len());
    let edit = Edit::Relief {
        object,
        depth_mm,
        image: image.clone(),
        tile_mm: args.relief_tile,
        angle_deg: args.relief_angle,
        invert: args.relief_invert,
        symmetric: args.relief_symmetric,
        smoothing_px: args.relief_smooth,
        hold_bottom_deg: args.relief_hold_bottom,
        hold_top_deg: args.relief_hold_top,
        precision: args.precision,
    };
    let applied = (staging.apply(&edit, object, watch.stop)).map_err(|error| match error {
        ApplyError::Failed { reason, .. } => {
            anyhow!("pressing the picture into the model: {reason}")
        }
        ApplyError::Cancelled => said(error),
        other => anyhow!("{other}"),
    })?;
    if watch.talk
        && let Report::Relieved {
            voxel_mm,
            coarsened,
        } = applied.report
    {
        let lattice = match coarsened {
            true => format!("{voxel_mm:.2} mm lattice, coarsened to fit the memory budget"),
            false => format!("{voxel_mm:.2} mm lattice"),
        };
        let became = staging.mesh(object).map_or(0, |mesh| mesh.faces.len());
        println!(
            "Relief: {depth_mm} mm of {} tiled every {} mm pressed in on a {lattice}, {faces} \
             faces became {became}, took {:.1}s\n",
            image.display(),
            args.relief_tile,
            started.elapsed().as_secs_f32()
        );
    }
    Ok(())
}
