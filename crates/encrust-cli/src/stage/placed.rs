//! The placing flags as edits of each entry's object, in the order `place` runs them, and the
//! copies once the plate is finished (ADR 0458, 0459).

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use core_document::{ApplyError, Report, degrees_of, euler_degrees, lift_by};
use core_edit::{Edit, ObjectId, OrientAim};
use core_geometry::{Quat, Vec2, Vec3, center_over_plate, drop_to_plate};
use printer_profiles::{Catalogue, PrinterProfile};

use super::document::{Staging, said};
use super::finished::usable_area;
use super::{Found, Shaping};
use crate::args::{Goal, OrientPick, TransformArgs};
use crate::pipeline::{OrientChoice, OrientSummary, Watch, with_bar};

/// What import leaves to edits of the placing flags.
pub(super) fn left_to_edits(transform: &mut TransformArgs) {
    transform.scale = None;
    transform.rotate = None;
    transform.center = false;
    transform.orient = false;
}

/// A model's file and what its flags ask of it.
type Entry = (PathBuf, Shaping);

/// Each entry's placing split where its modelling makes pieces: the search, the turn after it
/// and the lift wait for the pieces, each stood on its own; the rest places the whole model.
pub(super) fn split_at_pieces(entries: &[Entry]) -> (Vec<Entry>, Vec<Entry>) {
    let early = entries.iter().map(|(path, shaping)| {
        let mut shaping = shaping.clone();
        let transform = &mut shaping.import.transform;
        if transform.orient {
            transform.rotate = None;
        }
        (transform.orient, transform.lift) = (false, None);
        (path.clone(), shaping)
    });
    let late = entries.iter().map(|(path, shaping)| {
        let mut shaping = shaping.clone();
        shaping.position = None;
        let transform = &mut shaping.import.transform;
        if !transform.orient {
            transform.rotate = None;
        }
        (transform.scale, transform.center) = (None, false);
        (transform.placing.mirror, transform.placing.move_by) = (None, None);
        (path.clone(), shaping)
    });
    (early.collect(), late.collect())
}

/// Scales, orients, turns and centres each entry's objects the flags shape, an orient proved on
/// the catalogue printer `trial`; what the search chose is noted where import notes it.
pub(super) fn placed(
    staging: &mut Staging,
    found: &mut [Found],
    entries: &[(PathBuf, Shaping)],
    (trial, printer): (Option<&str>, Option<&PrinterProfile>),
    watch: &Watch,
) -> Result<()> {
    for (entry, ((_, oriented), (_, shaping))) in found.iter_mut().zip(entries).enumerate() {
        let ids = staging.targets(entry);
        for &id in &ids {
            place(staging, id, (shaping, oriented), (trial, printer), watch)?;
            if let Some(summary) = oriented.take_if(|_| ids.len() > 1) {
                staging.note_oriented(id, summary);
            }
        }
    }
    Ok(())
}

/// Places object `id` as `shaping`'s flags ask, in their order.
fn place(
    staging: &mut Staging,
    id: ObjectId,
    (shaping, oriented): (&Shaping, &mut Option<OrientSummary>),
    (trial, printer): (Option<&str>, Option<&PrinterProfile>),
    watch: &Watch,
) -> Result<()> {
    if let Some(position) = shaping.position {
        stand_at(staging, id, position, watch)?;
    }
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
        let summary = orient(staging, id, (transform, trial), (printer, supported), watch)?;
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
    if let Some(axis) = transform.placing.mirror {
        let edit = Edit::Mirror {
            object: id,
            axis: axis.into(),
        };
        staging.apply(&edit, id, watch.stop).map_err(said)?;
    }
    if transform.center {
        centre(staging, id, printer, watch)?;
    }
    if let Some(by_mm) = transform.placing.move_by {
        let edit = Edit::Move { object: id, by_mm };
        staging.apply(&edit, id, watch.stop).map_err(said)?;
    }
    if let Some(lift_mm) = transform.lift {
        lift(staging, id, lift_mm, watch)?;
    }
    Ok(())
}

/// Puts each entry's `--copies` beside each of its objects, in a row along X, each as far on as
/// the object and its supports are wide plus a gap; `--arrange` spreads them after.
pub(super) fn copied(
    staging: &mut Staging,
    entries: &[(PathBuf, Shaping)],
    watch: &Watch,
) -> Result<()> {
    for (entry, (_, shaping)) in entries.iter().enumerate() {
        let copies = shaping.import.transform.placing.copies.unwrap_or(0);
        for id in staging.targets(entry) {
            let width_mm =
                (staging.standing_bounds(id)).map_or(0.0, |bounds| bounds.maxs.x - bounds.mins.x);
            for copy in 1..=copies {
                let offset_mm = Vec3::new((width_mm + COPY_GAP_MM) * copy as f32, 0.0, 0.0);
                let edit = Edit::Duplicate {
                    object: id,
                    offset_mm,
                };
                staging.apply(&edit, id, watch.stop).map_err(said)?;
            }
        }
    }
    Ok(())
}

/// Gap between a model and its copies, mm, before an arrange spreads them.
const COPY_GAP_MM: f32 = 5.0;

/// Moves the object so the middle of its footprint is over a plate file's `position` and its
/// lowest point is on the plate.
fn stand_at(staging: &mut Staging, object: ObjectId, position: Vec2, watch: &Watch) -> Result<()> {
    let bounds =
        (staging.mesh(object).and_then(|mesh| mesh.aabb())).context("mesh has no vertices")?;
    let middle = (bounds.mins.truncate() + bounds.maxs.truncate()) / 2.0;
    let by_mm = (position - middle).extend(0.0) + drop_to_plate(&bounds);
    let edit = Edit::Move { object, by_mm };
    staging.apply(&edit, object, watch.stop).map_err(said)?;
    Ok(())
}

/// Stands the object `lift_mm` over its raft, as the window's Lift row does.
fn lift(staging: &mut Staging, object: ObjectId, lift_mm: f32, watch: &Watch) -> Result<()> {
    let bounds =
        (staging.mesh(object).and_then(|mesh| mesh.aabb())).context("mesh has no vertices")?;
    let by_mm = lift_by(&bounds, staging.raft_lift_mm(object), lift_mm);
    let edit = Edit::Move { object, by_mm };
    staging.apply(&edit, object, watch.stop).map_err(said)?;
    Ok(())
}

fn orient(
    staging: &mut Staging,
    object: ObjectId,
    (transform, trial): (&TransformArgs, Option<&str>),
    (printer, supported): (Option<&PrinterProfile>, bool),
    watch: &Watch,
) -> Result<OrientSummary> {
    let edit = orient_edit(object, (transform, trial), printer);
    let report = searched(staging, &edit, object, watch)?;
    let summary = summary_of(report, (transform, object), supported)?;
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

/// The search the flags ask, on the usable plate of `printer` where one is named, proved on the
/// catalogue printer `trial`.
fn orient_edit(
    object: ObjectId,
    (transform, trial): (&TransformArgs, Option<&str>),
    printer: Option<&PrinterProfile>,
) -> Edit {
    let area = printer.map(usable_area);
    let keep_clear = !transform.orient_keep_clear.is_empty();
    Edit::Orient {
        object,
        goal: aim_of(transform.orient_goal),
        max_tilt_deg: transform.max_tilt,
        clean_surface: transform.clean_surface,
        pick: usize::from(OrientPick::of(transform.orient_pick.as_ref(), object)).saturating_sub(1),
        plate_mm: area.map(|(_, size)| size),
        low_mm: area.map_or([0.0, 0.0], |(low, _)| low),
        printer: shipped_printer(trial, false),
        resin: None,
        keep_clear_mm: transform.orient_keep_clear.clone(),
        keep_clear_radius_mm: Some(transform.orient_keep_clear_radius),
        keep_clear_seen_deg: keep_clear
            .then(|| transform.orient_keep_clear_turn.unwrap_or(Vec3::ZERO)),
        lift_mm: transform.lift,
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
    (transform, object): (&TransformArgs, ObjectId),
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
    let pick =
        usize::from(OrientPick::of(transform.orient_pick.as_ref(), object)).saturating_sub(1);
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
    let (plate_x, plate_y) = printer.map_or((0.0, 0.0), |printer| {
        (printer.build_volume.x, printer.build_volume.y)
    });
    let by_mm = center_over_plate(&bounds, plate_x, plate_y) + drop_to_plate(&bounds);
    let edit = Edit::Move { object, by_mm };
    staging.apply(&edit, object, watch.stop).map_err(said)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(clap::Parser)]
    struct Flags {
        #[command(flatten)]
        transform: TransformArgs,
    }

    #[test]
    fn an_orient_is_proved_on_the_printer_the_run_stands_on_not_the_standard_one() {
        use clap::Parser;
        let flags = Flags::parse_from(["encrust", "--orient"]);
        let named = "elegoo-mars-4-ultra";

        let edit = orient_edit(ObjectId(1), (&flags.transform, Some(named)), None);

        let Edit::Orient { printer, .. } = edit else {
            panic!("an orient flag makes an orient edit");
        };
        assert_eq!(printer.as_deref(), Some(named), "a project's printer");
    }

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
