//! Hollowing, drains, channels, plugs and supports as edits of each entry's object, in the
//! window's order: holes stand where the object does, supports grow under it after (ADR 0472).

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use core_document::{ApplyError, Report};
use core_edit::{AUTO, Edit, ObjectId};
use core_engine::project::HollowState;
use core_engine::{Drained, SupportPlan};
use core_geometry::{Scalar, Vec3};
use core_supports::Unstood;
use core_volume::HoleKind;
use printer_profiles::PrinterProfile;

use super::Shaping;
use super::document::{Staging, said};
use crate::hollowing::{AutoReport, HollowArgs, HollowReport};
use crate::pipeline::Watch;
use crate::supports::SupportReport;

/// What hollowing, drilling and supporting one entry's object found.
#[derive(Default)]
pub(crate) struct Finished {
    pub hollow: Option<HollowReport>,
    pub drains: Option<Drained>,
    pub supports: Option<SupportReport>,
}

/// Hollows, drills, plugs and supports each entry's object as its flags ask, on the plate
/// `printer` has, plugs `drift_mm` narrower than asked as the resin prints them wider.
pub(super) fn finished(
    staging: &mut Staging,
    entries: &[(PathBuf, Shaping)],
    (printer, drift_mm): (Option<&PrinterProfile>, Scalar),
    watch: &Watch,
) -> Result<Vec<Finished>> {
    let mut found = Vec::with_capacity(entries.len());
    for (entry, (path, shaping)) in entries.iter().enumerate() {
        let Some(&id) = staging.ids_of(entry).first() else {
            found.push(Finished::default());
            continue;
        };
        if watch.talk && entries.len() > 1 && super::shapes_anything(shaping) {
            println!("{}", path.display());
        }
        let hollow = &shaping.hollow;
        let mut done = Finished {
            hollow: hollowed(staging, id, (hollow, shaping.import.precision), watch)?,
            ..Finished::default()
        };
        drilled(staging, id, hollow, watch)?;
        done.drains = searched(staging, id, hollow, watch)?;
        plugged(staging, id, (path, hollow), (printer, drift_mm), watch)?;
        if let Some(plan) = &shaping.supports {
            done.supports = Some(supported(staging, id, plan, watch)?);
            merged(staging, id, plan, watch)?;
        }
        rafted(staging, id, hollow, watch)?;
        found.push(done);
    }
    Ok(found)
}

fn hollowed(
    staging: &mut Staging,
    id: ObjectId,
    (args, precision): (&HollowArgs, Scalar),
    watch: &Watch,
) -> Result<Option<HollowReport>> {
    if args.walled {
        let points: Vec<Vec3> = args.points().collect();
        drain(staging, id, (&points, HoleKind::Walled), args, watch)?;
    }
    let Some(state) = args.state(precision) else {
        return Ok(None);
    };
    let started = Instant::now();
    let faces_before = staging.mesh(id).map_or(0, |mesh| mesh.faces.len());
    let edit = Edit::Hollow {
        object: id,
        settings: state.clone(),
        keep_solid: args.keep_solid.runs.clone(),
        keep_solid_reach_mm: args.keep_solid.reach_mm,
    };
    let (cavity_mm3, voxel_mm, coarsened) = match staging.apply(&edit, id, watch.stop) {
        Ok(applied) => match applied.report {
            Report::Hollowed {
                cavity_mm3,
                voxel_mm,
                coarsened,
            } => (cavity_mm3, voxel_mm, coarsened),
            _ => return Ok(None),
        },
        // A wall that leaves no cavity is the report's defect, the model left solid.
        Err(ApplyError::NoRoom(_)) => (0.0, 0.0, false),
        Err(ApplyError::NoSurface(_)) => {
            return Err(anyhow!(
                "no surface near a --keep-solid point to keep solid"
            ));
        }
        Err(other) => return Err(said(other).context("hollowing the model")),
    };
    let lattice = (cavity_mm3, voxel_mm, coarsened);
    let report = hollow_report(staging, id, &state, (faces_before, lattice), started);
    if watch.talk {
        print!("{report}");
    }
    Ok(Some(report))
}

/// Drills `--drain-at`'s points, unless `--walled` drilled them before the hollow, and digs
/// each `--channel`.
fn drilled(staging: &mut Staging, id: ObjectId, args: &HollowArgs, watch: &Watch) -> Result<()> {
    let points: Vec<Vec3> = args.points().collect();
    if !args.walled {
        drain(staging, id, (&points, HoleKind::AtDepth), args, watch)?;
    }
    for points_mm in &args.channels {
        let edit = Edit::Channel {
            object: id,
            points_mm: points_mm.clone(),
            diameter_mm: args.channel_mm,
        };
        staging
            .apply(&edit, id, watch.stop)
            .map_err(said)
            .context("digging the channel")?;
    }
    if watch.talk && (!points.is_empty() || !args.channels.is_empty()) {
        println!(
            "  drains        {} hole(s), {} channel(s)",
            points.len(),
            args.channels.len()
        );
    }
    Ok(())
}

fn drain(
    staging: &mut Staging,
    id: ObjectId,
    (at_mm, kind): (&[Vec3], HoleKind),
    args: &HollowArgs,
    watch: &Watch,
) -> Result<()> {
    if at_mm.is_empty() {
        return Ok(());
    }
    let edit = Edit::Drain {
        object: id,
        at_mm: at_mm.to_vec(),
        size: args.hole_size(),
        kind,
        auto: false,
        vent: false,
    };
    staging
        .apply(&edit, id, watch.stop)
        .map_err(|error| match error {
            ApplyError::NoSurface(_) => anyhow!("no surface near a --drain-at point to drill"),
            ApplyError::OffAFace { at_mm, .. } => {
                anyhow!("--drain-at {at_mm} is nearest an edge or corner, not a face, to drill")
            }
            other => said(other),
        })?;
    Ok(())
}

/// `--drain-at auto`: the engine's search on the object where it stands.
fn searched(
    staging: &mut Staging,
    id: ObjectId,
    args: &HollowArgs,
    watch: &Watch,
) -> Result<Option<Drained>> {
    if !args.auto() {
        return Ok(None);
    }
    let edit = Edit::Drain {
        object: id,
        at_mm: Vec::new(),
        size: args.hole_size(),
        kind: HoleKind::default(),
        auto: true,
        vent: !args.no_vent,
    };
    let applied = staging
        .apply(&edit, id, watch.stop)
        .map_err(|error| match error {
            ApplyError::NoCavity(_) => {
                anyhow!("--drain-at auto drains a cavity, and --hollow was not given")
            }
            other => said(other).context("searching for drain holes"),
        })?;
    let Report::Drained { searched, .. } = applied.report else {
        return Ok(None);
    };
    if watch.talk
        && let Some(drained) = &searched
    {
        print!("{}", AutoReport(drained, 0.0));
    }
    Ok(searched)
}

fn plugged(
    staging: &mut Staging,
    id: ObjectId,
    (path, args): (&Path, &HollowArgs),
    (printer, drift_mm): (Option<&PrinterProfile>, Scalar),
    watch: &Watch,
) -> Result<()> {
    if !args.plugs {
        return Ok(());
    }
    let area = printer.map(usable_area);
    let gap_mm = args.plug_gap_mm - drift_mm;
    let edit = Edit::Plugs {
        object: id,
        gap_mm,
        plate_mm: area.map(|(_, size)| size),
        low_mm: area.map_or([0.0, 0.0], |(low, _)| low),
    };
    let applied = staging
        .apply(&edit, id, watch.stop)
        .map_err(said)
        .context("cutting the plugs")?;
    let Report::Plugged { plugs, crowded, .. } = applied.report else {
        return Ok(());
    };
    if crowded > 0 {
        bail!(
            "the plugs for {} run off the plate or onto another model; give it room or leave out --drain-plugs",
            path.display()
        );
    }
    if watch.talk && plugs > 0 {
        println!("  plugs         {plugs} beside the model, {gap_mm:.2} mm narrower a side\n");
    }
    Ok(())
}

/// The report of object `id` hollowed by `settings`, as `--hollow` says it; `lattice` is the
/// cavity mm³, the voxel mm and whether it was coarsened.
pub(crate) fn hollow_report(
    staging: &Staging,
    id: ObjectId,
    settings: &HollowState,
    (faces_before, lattice): (usize, (Scalar, Scalar, bool)),
    started: Instant,
) -> HollowReport {
    let faces_after = staging.mesh(id).map_or(0, |mesh| mesh.faces.len());
    let transform = staging.transform(id);
    HollowReport::of(
        (settings, transform),
        (faces_before, faces_after),
        lattice,
        started.elapsed(),
    )
}

/// The report of the supports grown under object `id`, as `--supports` says it.
pub(crate) fn support_report(
    staging: &Staging,
    id: ObjectId,
    (trees, contacts, standing, unheld): (usize, Vec<Vec3>, usize, Vec<(Vec3, Unstood)>),
) -> SupportReport {
    let lowest =
        (staging.mesh(id).and_then(|mesh| mesh.aabb())).map_or(0.0, |box_mm| box_mm.mins.z);
    SupportReport {
        lifted_mm: lowest.max(0.0),
        contacts,
        standing,
        unheld,
        trees,
        faces: staging.support_faces(id),
    }
}

/// The usable plate of `printer`, its inset taken off: its low corner and its size, mm.
pub(crate) fn usable_area(printer: &PrinterProfile) -> ([Scalar; 2], [Scalar; 2]) {
    let [low, high] = printer.usable_area_mm();
    (low, [high[0] - low[0], high[1] - low[1]])
}

fn supported(
    staging: &mut Staging,
    id: ObjectId,
    plan: &SupportPlan,
    watch: &Watch,
) -> Result<SupportReport> {
    let profile = match plan.sizing() {
        Some(_) => AUTO.to_owned(),
        None => plan.placing().name,
    };
    let edit = Edit::Supports {
        object: id,
        profile: Some(profile),
        at_mm: plan.by_hand.at_mm.clone(),
        only_at: plan.by_hand.only,
        cavity: plan.cavity,
        shown_at: plan.shown_at.clone(),
        clean_surface: false,
    };
    let applied = staging
        .apply(&edit, id, watch.stop)
        .map_err(said)
        .context("standing the supports")?;
    let Report::Supported {
        trees,
        contacts_mm,
        standing,
        unheld,
        ..
    } = applied.report
    else {
        return Err(anyhow!("the supports run answered no supports"));
    };
    let report = support_report(staging, id, (trees, contacts_mm, standing, unheld));
    if watch.talk {
        print!("{report}");
    }
    Ok(report)
}

/// `--drain-support-at`: holes into the raft the supports stand on.
fn rafted(staging: &mut Staging, id: ObjectId, args: &HollowArgs, watch: &Watch) -> Result<()> {
    if args.drain_support_at.is_empty() {
        return Ok(());
    }
    let edit = Edit::Drain {
        object: id,
        at_mm: args.drain_support_at.clone(),
        size: args.hole_size(),
        kind: HoleKind::Support,
        auto: false,
        vent: false,
    };
    staging
        .apply(&edit, id, watch.stop)
        .map_err(|error| match error {
            ApplyError::NoRaft(_) => {
                anyhow!("--drain-support-at drills into the raft, and none stands under the model")
            }
            ApplyError::NoSurface(_) => anyhow!("no raft near a --drain-support-at point to drill"),
            other => said(other),
        })?;
    Ok(())
}

/// Each `--merge-supports`, as Merge into one joins the supports it names.
fn merged(staging: &mut Staging, id: ObjectId, plan: &SupportPlan, watch: &Watch) -> Result<()> {
    for near_mm in &plan.merges {
        let edit = Edit::MergeSupports {
            object: id,
            near_mm: near_mm.clone(),
        };
        staging
            .apply(&edit, id, watch.stop)
            .map_err(|error| match error {
                ApplyError::NoSupportNear { at_mm, .. } => {
                    anyhow!("no support has its contact within half a millimetre of {at_mm}")
                }
                other => said(other).context("merging the supports"),
            })?;
    }
    Ok(())
}
