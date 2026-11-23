//! The placing flags as edits of each entry's object: the scale, the orientation search, a
//! turn and centring, in that order (ADR 0472).

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use core_document::{ApplyError, Report, degrees_of, euler_degrees};
use core_edit::{Edit, ObjectId, OrientAim};
use core_geometry::{Quat, Vec3};
use printer_profiles::{Catalogue, PrinterProfile};

use super::document::{Staging, said};
use super::{Found, Shaping};
use crate::args::{Goal, ProfileArgs, TransformArgs};
use crate::pipeline::{OrientChoice, OrientSummary, Watch, with_bar};

/// What import leaves to edits of the placing flags.
pub(super) fn left_to_edits(transform: &mut TransformArgs) {
    transform.scale = None;
    transform.rotate = None;
    transform.center = false;
    transform.orient = false;
}

/// Scales, orients, turns and centres each entry's one object as its flags ask; what the
/// search chose is noted where import notes it.
pub(super) fn placed(
    staging: &mut Staging,
    found: &mut [Found],
    entries: &[(PathBuf, Shaping)],
    (profile, printer): (&ProfileArgs, Option<&PrinterProfile>),
    watch: &Watch,
) -> Result<()> {
    for (entry, ((_, oriented), (_, shaping))) in found.iter_mut().zip(entries).enumerate() {
        let id = ObjectId(entry as u64 + 1);
        let transform = &shaping.import.transform;
        if let Some(factors) = transform.scale {
            let edit = Edit::Scale {
                object: id,
                factors,
            };
            staging.apply(&edit, id, watch.stop).map_err(said)?;
        }
        if transform.orient {
            let supported = shaping.supports.is_some();
            let summary = orient(
                staging,
                id,
                (transform, profile),
                (printer, supported),
                watch,
            )?;
            *oriented = Some(summary);
        }
        if let Some(degrees) = transform.rotate {
            turn(
                staging,
                id,
                euler_degrees(degrees),
                oriented.as_mut(),
                watch,
            )?;
        }
        if transform.center {
            centre(staging, id, printer, watch)?;
        }
    }
    Ok(())
}

fn orient(
    staging: &mut Staging,
    object: ObjectId,
    (transform, profile): (&TransformArgs, &ProfileArgs),
    (printer, supported): (Option<&PrinterProfile>, bool),
    watch: &Watch,
) -> Result<OrientSummary> {
    let edit = orient_edit(object, (transform, profile), printer);
    let report = searched(staging, &edit, object, watch)?;
    let summary = summary_of(report, transform, supported)?;
    if watch.talk {
        let seated = if summary.on_plate {
            ", set on the plate"
        } else {
            ""
        };
        println!(
            "Oriented: {}, {:.1} mm2 largest section, {:.1} mm2 overhang{seated}\n",
            summary.describe(),
            summary.peak_mm2,
            summary.overhang_mm2,
        );
    }
    Ok(summary)
}

/// The search the flags ask, on the usable plate of `printer` where one is named.
fn orient_edit(
    object: ObjectId,
    (transform, profile): (&TransformArgs, &ProfileArgs),
    printer: Option<&PrinterProfile>,
) -> Edit {
    let area = printer.map(|printer| {
        let volume = &printer.build_volume;
        let [low, high] = (printer.plate_inset.unwrap_or_default()).area_mm(volume.x, volume.y);
        (low, [high[0] - low[0], high[1] - low[1]])
    });
    let keep_clear = !transform.orient_keep_clear.is_empty();
    Edit::Orient {
        object,
        goal: aim_of(transform.orient_goal),
        max_tilt_deg: transform.max_tilt,
        clean_surface: transform.clean_surface,
        pick: usize::from(transform.orient_pick.unwrap_or(1)).saturating_sub(1),
        plate_mm: area.map(|(_, size)| size),
        low_mm: area.map_or([0.0, 0.0], |(low, _)| low),
        printer: shipped_printer(profile.printer.as_deref(), profile.profile.is_some()),
        resin: None,
        keep_clear_mm: transform.orient_keep_clear.clone(),
        keep_clear_radius_mm: Some(transform.orient_keep_clear_radius),
        keep_clear_seen_deg: keep_clear
            .then(|| transform.orient_keep_clear_turn.unwrap_or(Vec3::ZERO)),
    }
}

/// The search run with its bar on a terminal, Ctrl-C stopping it.
fn searched(staging: &mut Staging, edit: &Edit, object: ObjectId, watch: &Watch) -> Result<Report> {
    let applied = with_bar(watch, "% of the orientation search", |cancel| {
        staging.apply_with(edit, object, cancel)
    });
    let applied = applied.map_err(|error| match error {
        ApplyError::NoChoice { pick, found, .. } => anyhow!(
            "--orient-pick {}: the search found {found} ways to stand this model",
            pick + 1
        ),
        ApplyError::Cancelled => said(error),
        other => anyhow::Error::new(other).context("cannot find an orientation for this model"),
    })?;
    Ok(applied.report)
}

/// What the search chose, as import notes it: the first `--orient-choices` poses listed.
pub(crate) fn summary_of(
    report: Report,
    transform: &TransformArgs,
    supported: bool,
) -> Result<OrientSummary> {
    let Report::Oriented {
        rotate_deg,
        on_plate,
        peak_mm2,
        overhang_mm2,
        ranked,
    } = report
    else {
        return Err(anyhow!("the orientation search answered no pose"));
    };
    let shown = usize::from(transform.orient_choices.unwrap_or(0));
    let pick = usize::from(transform.orient_pick.unwrap_or(1)).saturating_sub(1);
    let chosen = ranked.get(pick).map(|ranked| ranked.rotation);
    Ok(OrientSummary {
        rotation: chosen.unwrap_or_else(|| euler_degrees(rotate_deg)),
        peak_mm2,
        overhang_mm2,
        on_plate: supported && on_plate,
        choices: (ranked.iter().take(shown))
            .map(|ranked| OrientChoice {
                rotation: ranked.rotation,
                on_plate: ranked.on_plate,
                offered: ranked.offered,
                printout: ranked.printout,
            })
            .collect(),
    })
}

/// The id `--printer` names where the shipped catalogue lists it, which an orient's trial is
/// looked up in; a profile of a file or of the user's own is none, the standard one standing in.
pub(crate) fn shipped_printer(id: Option<&str>, from_a_file: bool) -> Option<String> {
    let id = id.filter(|_| !from_a_file)?;
    let shipped = Catalogue::bundled().ok()?;
    shipped.printer(id).ok().map(|entry| entry.id.clone())
}

fn aim_of(goal: Goal) -> OrientAim {
    match goal {
        Goal::FewestSupports => OrientAim::FewestSupports,
        Goal::LeastPeel => OrientAim::LeastPeel,
        Goal::ShortestPrint => OrientAim::ShortestPrint,
        Goal::Auto => OrientAim::Auto,
    }
}

/// Turns the object by `rotation` after any pose the search chose; one that tips the model
/// off the face the search seated it on leaves it on supports.
fn turn(
    staging: &mut Staging,
    object: ObjectId,
    rotation: Quat,
    oriented: Option<&mut OrientSummary>,
    watch: &Watch,
) -> Result<()> {
    let chosen = oriented
        .as_ref()
        .map_or(Quat::IDENTITY, |turn| turn.rotation);
    let edit = Edit::Rotate {
        object,
        degrees: degrees_of(rotation * chosen),
    };
    staging.apply(&edit, object, watch.stop).map_err(said)?;
    if let Some(summary) = oriented
        && !keeps_face_down(rotation)
    {
        summary.on_plate = false;
    }
    Ok(())
}

/// Whether a turn by `rotation` leaves a model seated by the search on the face it was seated
/// on; a tip onto an edge stands it on supports.
fn keeps_face_down(rotation: Quat) -> bool {
    (rotation * Vec3::NEG_Z).abs_diff_eq(Vec3::NEG_Z, 1e-4)
}

/// Moves the object over the middle of `printer`'s plate, of the origin with none.
pub(super) fn centre(
    staging: &mut Staging,
    object: ObjectId,
    printer: Option<&PrinterProfile>,
    watch: &Watch,
) -> Result<()> {
    let bounds =
        (staging.mesh(object).and_then(|mesh| mesh.aabb())).context("mesh has no vertices")?;
    let plate = printer.map_or(Vec3::ZERO, |printer| {
        let volume = &printer.build_volume;
        Vec3::new(volume.x, volume.y, 0.0) / 2.0
    });
    let middle = (bounds.mins + bounds.maxs) / 2.0;
    let by_mm = Vec3::new(plate.x - middle.x, plate.y - middle.y, -bounds.mins.z);
    let edit = Edit::Move { object, by_mm };
    staging.apply(&edit, object, watch.stop).map_err(said)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rotate_that_tips_a_seated_model_stands_it_on_supports() {
        assert!(
            keeps_face_down(Quat::IDENTITY),
            "no turn keeps the face down"
        );
        assert!(
            !keeps_face_down(euler_degrees(Vec3::new(30.0, 0.0, 0.0))),
            "tipped onto an edge, it no longer rests on a face"
        );
        assert!(
            keeps_face_down(euler_degrees(Vec3::new(0.0, 0.0, 30.0))),
            "a turn about the vertical keeps the face down"
        );
    }
}
