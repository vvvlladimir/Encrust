//! The modelling flags as edits of the plate, in the order they shape: combine, label, cut,
//! then split or part-off each piece (ADR 0312, 0472).

use anyhow::{Context, Result, anyhow, bail};
use core_document::{ApplyError, Report};
use core_edit::{Edit, ObjectId};
use core_geometry::{Scalar, Vec3};
use core_joint::{JointKind, Joints};
use printer_profiles::PrinterProfile;

use super::document::{Staging, said};
use super::placed::centre;
use crate::exit::Stop;
use crate::modelling::{ModelArgs, Op};
use crate::pipeline::Watch;

/// Combines, centres the one model that makes where `centred` names the printer's plate, then
/// labels, cuts and parts the plate's objects as `args` ask.
pub(super) fn modelled(
    staging: &mut Staging,
    args: &ModelArgs,
    centred: Option<Option<&PrinterProfile>>,
    watch: &Watch,
) -> Result<()> {
    let (talk, stop) = (watch.talk, watch.stop);
    if let Some(op) = args.combine {
        combine(staging, op, (talk, stop))?;
        if let Some(printer) = centred {
            for id in staging.every_target() {
                centre(staging, id, printer, watch)?;
            }
        }
    }
    if let Some(text) = &args.label {
        for id in staging.every_target() {
            label(staging, id, text, args, (talk, stop))?;
        }
    }
    if let Some(plane) = args.cut {
        for id in staging.every_target() {
            cut(staging, id, plane, args, (talk, stop))?;
        }
    }
    for id in staging.every_target() {
        part(staging, id, args, (talk, stop))?;
    }
    Ok(())
}

fn combine(staging: &mut Staging, op: Op, (talk, stop): (bool, &Stop)) -> Result<()> {
    let objects = staging.every_target();
    if objects.len() < 2 {
        bail!("--combine takes two models or more, got {}", objects.len());
    }
    let edit = Edit::Combine {
        objects: objects.clone(),
        op: op.into(),
        toward: None,
    };
    let applied = (staging.apply(&edit, objects[0], stop))
        .map_err(|error| match error {
            ApplyError::NotSolid(_) => anyhow!("a model is not a closed solid"),
            other => said(other),
        })
        .context("combining the models")?;
    if talk {
        match applied.report {
            Report::Combined { voxel_mm: None } => println!("Combined {} models\n", objects.len()),
            Report::Combined {
                voxel_mm: Some(voxel_mm),
            } => println!(
                "Combined {} models on a {voxel_mm:.2} mm lattice, as one is not a closed solid\n",
                objects.len()
            ),
            _ => {}
        }
    }
    Ok(())
}

fn label(
    staging: &mut Staging,
    object: ObjectId,
    text: &str,
    args: &ModelArgs,
    (talk, stop): (bool, &Stop),
) -> Result<()> {
    let at_mm = args.label_at.context("--label needs --label-at")?;
    let edit = Edit::Label {
        object,
        text: text.to_owned(),
        at_mm,
        height_mm: Some(args.label_height),
        depth_mm: Some(args.label_depth),
        raised: !args.label_sunk,
        follow_surface: args.label_follow,
        angle_deg: args.label_angle,
        font: args.label_font.clone(),
    };
    (staging.apply(&edit, object, stop))
        .map_err(|error| match error {
            ApplyError::NoSurface(_) => anyhow!("the model has no surface near --label-at"),
            ApplyError::NotSolid(_) => anyhow!("the model is not a closed solid"),
            ApplyError::BadFont {
                path: Some(path), ..
            } => anyhow!("cannot read the font {}", path.display()),
            other => said(other),
        })
        .context("writing the label")?;
    if talk {
        println!("Labelled with {text:?} at {at_mm}\n");
    }
    Ok(())
}

fn cut(
    staging: &mut Staging,
    object: ObjectId,
    (axis, at_mm): (core_edit::Axis, Scalar),
    args: &ModelArgs,
    (talk, stop): (bool, &Stop),
) -> Result<()> {
    let edit = Edit::Cut {
        object,
        axis,
        at_mm,
        keep: args.keep.into(),
        joints: Joints::asked(args.joint.map(JointKind::from), &args.joint_asked()),
        lay_flat: !args.cut_upright,
    };
    let applied = staging
        .apply(&edit, object, stop)
        .map_err(|error| match error {
            ApplyError::Missed(_) => anyhow!("the --cut plane misses the model"),
            other => said(other),
        })?;
    let Report::Cut {
        open_loops,
        placed,
        unjoined,
        ..
    } = applied.report
    else {
        return Ok(());
    };
    if let Some(reason) = unjoined {
        bail!("the joint could not be built into the halves: {reason}");
    }
    if talk {
        println!("Cut: {placed} fixings, {open_loops} open loops left uncapped\n");
    }
    Ok(())
}

fn part(
    staging: &mut Staging,
    object: ObjectId,
    args: &ModelArgs,
    (talk, stop): (bool, &Stop),
) -> Result<()> {
    if let Some(at_mm) = args.part_off {
        return part_off(staging, object, at_mm, (talk, stop));
    }
    if !args.split {
        return Ok(());
    }
    let edit = Edit::Split {
        object,
        smallest_mm3: Some(args.split_smallest),
    };
    let applied = staging.apply(&edit, object, stop).map_err(said)?;
    if talk
        && let Report::Split {
            parts,
            joined,
            dropped,
        } = applied.report
    {
        println!("Split into {parts} parts: {joined} scraps joined, {dropped} dropped\n");
    }
    Ok(())
}

fn part_off(
    staging: &mut Staging,
    object: ObjectId,
    at_mm: Vec3,
    (talk, stop): (bool, &Stop),
) -> Result<()> {
    let edit = Edit::PartOff { object, at_mm };
    (staging.apply(&edit, object, stop))
        .map_err(|error| match error {
            ApplyError::NoNeck(_) => anyhow!("there is no narrow join to cut at from this point"),
            other => said(other),
        })
        .context("cutting the part off")?;
    if talk {
        println!("Cut the part at {at_mm} off where it joins the rest\n");
    }
    Ok(())
}
