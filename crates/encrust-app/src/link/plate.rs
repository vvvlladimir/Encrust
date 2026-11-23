//! Schema 3 of the window link: the plate handed to a server as it stands, and the edits it
//! built on it applied while the plate is still that version (ADR 0466).

use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::Sender;

use core_document::{
    ApplyError, CutAsked, LabelAsked, MendAsked, ReliefAsked, blockers_along, boolean_of, mirrored,
    moved, part_settings, repair_settings, sized_to, turned_to,
};
use core_edit::{Edit, ObjectId, Op};
use core_engine::project::HollowState;
use core_geometry::{Mesh, Transform, Vec3};
use core_volume::PartSettings;
use core_volume::{RepairMethod, RepairSettings};
use serde_json::{Value, json};

use crate::job::Method;
use crate::panels::Window;
use crate::repair::RepairState;

mod arrange;
mod drain;
mod orient;
mod supports;

use arrange::start_arrange;
use drain::{start_channel, start_drain, start_plugs};
use orient::{stand_pick, start_orient};
use supports::{inside_pass_of, start_inside, start_merge, start_supports};

/// Edits being applied one after another, each a job of the tool that carries it out.
#[derive(Debug)]
pub struct Applying {
    edits: VecDeque<Edit>,
    reply: Sender<Value>,
    /// The object the edit under way replaces, there still if the edit did not land, and
    /// whether the edit may leave it in place and be done (a split of one body).
    under_way: Option<(ObjectId, bool)>,
    /// The object whose cavity a supports edit holds up once its outside run has landed, and
    /// whether that run is sized by the automatic profile.
    inside: Option<(ObjectId, bool)>,
    /// The object whose orient search, once landed, is stepped to the pose asked for, and its
    /// number from 0.
    picking: Option<(ObjectId, usize)>,
}

fn refused(why: String) -> Value {
    json!({"applied": false, "why_not": why})
}

/// Writes the plate as it stands into `job`'s folder under `jobs` off the drawing thread,
/// and answers with its path, the plate's version, the ids on it and `job_object`, the
/// job's model where it stands on the plate whole.
pub fn hand_plate(
    jobs: &Path,
    (job, job_object): (&str, Option<ObjectId>),
    window: &Window,
    reply: Sender<Value>,
) {
    let Some(dir) = job_dir(jobs, job) else {
        let _ = reply.send(json!({"plate": null, "why_not": format!("{job:?} names no job")}));
        return;
    };
    let version = window.doc.history.plate_version();
    let project = crate::project::captured(window);
    let objects: Vec<ObjectId> = (project.manifest.objects.iter()).map(|o| o.id).collect();
    let path = dir.join(format!("base-{version}.encrust"));
    std::thread::spawn(move || {
        let written = std::fs::create_dir_all(&dir)
            .map_err(|error| error.to_string())
            .and_then(|()| core_engine::project::save(&path, &project).map_err(|e| e.to_string()));
        let answer = match written {
            Ok(()) => json!({"plate": path, "version": version, "objects": objects,
                             "job_object": job_object}),
            Err(why) => json!({"plate": null, "why_not": why}),
        };
        let _ = reply.send(answer);
    });
}

/// `job`'s folder under `jobs`, for a job named by one plain path segment alone.
fn job_dir(jobs: &Path, job: &str) -> Option<PathBuf> {
    let mut parts = Path::new(job).components();
    match (parts.next(), parts.next()) {
        (Some(Component::Normal(_)), None) => Some(jobs.join(job)),
        _ => None,
    }
}

/// Starts applying `edits` if the plate is still at `base_version`; otherwise answers
/// `stale` with the version it is at, for the server to build again.
pub fn start(
    edits: Vec<Edit>,
    base_version: u64,
    window: &Window,
    reply: Sender<Value>,
) -> Option<Applying> {
    let version = window.doc.history.plate_version();
    if base_version != version {
        let _ = reply.send(json!({"applied": false, "stale": true, "version": version}));
        return None;
    }
    Some(Applying {
        edits: edits.into(),
        reply,
        under_way: None,
        inside: None,
        picking: None,
    })
}

/// Moves the edits along once the tool carrying the last one is done, answering when all
/// have landed or one did not. Answers whether edits are still being applied.
pub fn advance(applying: &mut Option<Applying>, window: &mut Window) -> bool {
    let Some(work) = applying.as_mut() else {
        return false;
    };
    if tool_is_busy(window) {
        return true;
    }
    if let Some((id, may_stay)) = work.under_way.take()
        && window.doc.scene.get(id).is_some()
        && !(may_stay && !window.machine.status.is_error())
    {
        let why = window.machine.status.text();
        return finish(applying, refused(why));
    }
    if let Some((object, pick)) = work.picking.take()
        && let Err(why) = stand_pick(window, object, pick)
    {
        return finish(applying, refused(why));
    }
    if let Some((object, sized)) = work.inside.take() {
        return match start_inside(window, object, sized) {
            Ok(()) => {
                work.under_way = Some((object, true));
                true
            }
            Err(why) => finish(applying, refused(why)),
        };
    }
    let Some(edit) = work.edits.pop_front() else {
        return finish(applying, json!({"applied": true}));
    };
    let may_stay = matches!(
        edit,
        Edit::Split { .. }
            | Edit::Repair { .. }
            | Edit::Mend { .. }
            | Edit::Relief { .. }
            | Edit::Hollow { .. }
            | Edit::Drain { .. }
            | Edit::Supports { .. }
            | Edit::Orient { .. }
    );
    match start_edit(&edit, window) {
        Ok(id) => {
            work.under_way = id.map(|id| (id, may_stay));
            work.inside = inside_pass_of(&edit);
            work.picking = match edit {
                Edit::Orient { object, pick, .. } => Some((object, pick)),
                _ => None,
            };
            true
        }
        Err(why) => finish(applying, refused(why)),
    }
}

fn tool_is_busy(window: &Window) -> bool {
    let repairs = &window.doc.repairs;
    window.tools.cut.is_busy()
        || window.tools.boolean.is_busy()
        || window.tools.text.is_busy()
        || repairs.is_busy()
        || repairs.hand.is_busy()
        || window.tools.relief.is_busy()
        || window.tools.hollow.job.is_some()
        || window.tools.supports.job.is_some()
        || window.tools.drain.auto.is_some()
        || window.tools.drain.laying_plugs()
        || window.tools.orient.is_running()
}

fn finish(applying: &mut Option<Applying>, answer: Value) -> bool {
    if let Some(work) = applying.take() {
        let _ = work.reply.send(answer);
    }
    false
}

/// Carries `edit` out: one that runs a tool job answers the object it replaces, any other is
/// made at once and answers `None`.
fn start_edit(edit: &Edit, window: &mut Window) -> Result<Option<ObjectId>, String> {
    match *edit {
        Edit::Cut {
            object,
            axis,
            at_mm,
            keep,
            joints,
            lay_flat,
        } => {
            let asked = CutAsked {
                axis,
                at_mm,
                keep,
                joints,
                lay_flat,
            };
            (window.tools.cut)
                .start_asked(&window.doc.scene, object, asked)
                .map_err(|error| format!("{error:#}"))?;
            Ok(Some(object))
        }
        Edit::Split {
            object,
            smallest_mm3,
        } => start_split(window, object, part_settings(smallest_mm3), None),
        Edit::PartOff { object, at_mm } => {
            start_split(window, object, PartSettings::default(), Some(at_mm))
        }
        Edit::Combine {
            ref objects,
            op,
            toward,
        } => start_combine(window, objects, op, toward),
        Edit::Label { object, .. } => start_label(window, edit, object),
        Edit::Repair {
            object,
            method,
            seam_tolerance_mm,
            smallest_shell_mm2,
        } => {
            let settings = repair_settings(seam_tolerance_mm, smallest_shell_mm2);
            start_repair(window, object, (method, settings))
        }
        Edit::Mend { object, .. } => start_mend(window, edit, object),
        Edit::Relief { object, .. } => start_relief(window, edit, object),
        Edit::Hollow {
            object,
            ref settings,
            ref keep_solid,
            keep_solid_reach_mm,
        } => start_hollow(
            window,
            object,
            (settings, (keep_solid, keep_solid_reach_mm)),
        ),
        Edit::Drain { .. }
        | Edit::Channel { .. }
        | Edit::Plugs { .. }
        | Edit::Supports { .. }
        | Edit::MergeSupports { .. } => start_holding(window, edit),
        Edit::Arrange { .. } => start_arrange(window, edit),
        Edit::Orient { .. } => start_orient(window, edit),
        Edit::Move { .. }
        | Edit::Rotate { .. }
        | Edit::Scale { .. }
        | Edit::Delete { .. }
        | Edit::Duplicate { .. }
        | Edit::Mirror { .. } => start_placing(window, edit),
    }
}

/// Starts combining `objects` as the Combine tool's job, answering the first, which a
/// difference keeps.
fn start_combine(
    window: &mut Window,
    objects: &[ObjectId],
    op: Op,
    toward: Option<Vec3>,
) -> Result<Option<ObjectId>, String> {
    let method = toward.map_or(Method::Sampled, Method::ExactToward);
    (window.tools.boolean)
        .start_asked(&window.doc.scene, objects, (boolean_of(op), method))
        .map_err(|error| format!("{error:#}"))?;
    Ok(objects.first().copied())
}

/// Takes `object` and the plugs laid for it off the plate at once.
fn take_off(window: &mut Window, object: ObjectId) -> Result<Option<ObjectId>, String> {
    let scene = &mut window.doc.scene;
    scene.get(object).ok_or_else(|| no_object(object))?;
    scene.remove_plugs_of(object);
    scene.remove(object);
    Ok(None)
}

/// Starts a split or a part-off as a tool job, answering the object it replaces.
fn start_split(
    window: &mut Window,
    object: ObjectId,
    settings: PartSettings,
    at: Option<Vec3>,
) -> Result<Option<ObjectId>, String> {
    (window.tools.cut)
        .start_split_asked(&window.doc.scene, object, settings, at)
        .map_err(|error| format!("{error:#}"))?;
    Ok(Some(object))
}

/// Starts writing a label edit as a tool job, answering the object it replaces.
fn start_label(
    window: &mut Window,
    edit: &Edit,
    object: ObjectId,
) -> Result<Option<ObjectId>, String> {
    let asked = LabelAsked::of(edit).ok_or_else(|| no_object(object))?;
    (window.tools.text)
        .stamp_asked(&window.doc.scene, object, &asked)
        .map_err(|error| format!("{error:#}"))?;
    Ok(Some(object))
}

/// Starts a repair as the Repair tool's own job, answering the object it reshapes.
fn start_repair(
    window: &mut Window,
    object: ObjectId,
    (method, settings): (RepairMethod, RepairSettings),
) -> Result<Option<ObjectId>, String> {
    window
        .doc
        .scene
        .get(object)
        .ok_or_else(|| no_object(object))?;
    let state = RepairState {
        method,
        settings,
        ..RepairState::default()
    };
    let (repairs, scene) = (&mut window.doc.repairs, &window.doc.scene);
    repairs.start(scene, object, state, &mut window.machine.status);
    Ok(Some(object))
}

/// Starts a mend by hand as one job off the drawing thread, answering the object it reshapes.
fn start_mend(
    window: &mut Window,
    edit: &Edit,
    object: ObjectId,
) -> Result<Option<ObjectId>, String> {
    let asked = MendAsked::of(edit).ok_or_else(|| no_object(object))?;
    let (hand, scene) = (&mut window.doc.repairs.hand, &window.doc.scene);
    if !hand.mend(scene, object, asked, &mut window.machine.status) {
        return Err(no_object(object));
    }
    Ok(Some(object))
}

/// Starts pressing a picture into an object as the Relief tool's job, answering the object it
/// reshapes.
fn start_relief(
    window: &mut Window,
    edit: &Edit,
    object: ObjectId,
) -> Result<Option<ObjectId>, String> {
    let asked = ReliefAsked::of(edit).ok_or_else(|| no_object(object))?;
    let scene = &window.doc.scene;
    scene.get(object).ok_or_else(|| no_object(object))?;
    (window.tools.relief)
        .start_asked(scene, object, &asked)
        .map_err(|error| format!("{error:#}"))?;
    Ok(Some(object))
}

/// Starts hollowing one object by `settings`, which it keeps as its own, as the Hollow
/// tool's job, answering the object it builds the shell on.
fn start_hollow(
    window: &mut Window,
    object: ObjectId,
    (settings, (runs, reach_mm)): (&HollowState, (&[Vec<Vec3>], Option<f32>)),
) -> Result<Option<ObjectId>, String> {
    let model = (window.doc.scene.get_mut(object)).ok_or_else(|| no_object(object))?;
    if model.mesh.is_empty() {
        return Err(format!("object {object:?} has no surface to hollow"));
    }
    let surface = (&*model.mesh, &*model.bvh);
    let blockers = blockers_along(surface, model.transform, (runs, reach_mm), object)
        .map_err(|error| error.to_string())?;
    for blocker in blockers {
        model.hollow.place_blocker(blocker);
    }
    model.own.hollow = Some(settings.clone());
    let (scene, hollow) = (&window.doc.scene, &mut window.tools.hollow);
    let asked = scene.get(object).map(|model| hollow.settings_for(model));
    let asked = asked.ok_or_else(|| no_object(object))?;
    if !hollow.again(scene, &[(object, asked)]) {
        return Err("a hollowing is already under way".to_owned());
    }
    Ok(Some(object))
}

fn no_object(id: ObjectId) -> String {
    ApplyError::NoObject(id).to_string()
}

/// Starts an edit that drills into an object or stands supports under it.
fn start_holding(window: &mut Window, edit: &Edit) -> Result<Option<ObjectId>, String> {
    match *edit {
        Edit::Drain {
            object,
            ref at_mm,
            size,
            kind,
            auto,
            vent,
        } => start_drain(window, object, at_mm, (size, kind), (auto, vent)),
        Edit::Channel {
            object,
            ref points_mm,
            diameter_mm,
        } => start_channel(window, object, points_mm, diameter_mm),
        Edit::Plugs { object, gap_mm, .. } => start_plugs(window, object, gap_mm),
        Edit::Supports { object, .. } => start_supports(window, edit, object),
        Edit::MergeSupports {
            object,
            ref near_mm,
        } => start_merge(window, object, near_mm),
        _ => Ok(None),
    }
}

/// Carries out at once an edit that places, copies or takes off an object.
fn start_placing(window: &mut Window, edit: &Edit) -> Result<Option<ObjectId>, String> {
    match *edit {
        Edit::Move { object, by_mm } => {
            place(window, object, |transform, _| Ok(moved(transform, by_mm)))
        }
        Edit::Rotate { object, degrees } => place(window, object, |transform, mesh| {
            Ok(turned_to(transform, mesh, degrees))
        }),
        Edit::Scale { object, factors } => place(window, object, |transform, mesh| {
            sized_to(transform, mesh, factors)
                .ok_or_else(|| ApplyError::BadScale(object).to_string())
        }),
        Edit::Delete { object } => take_off(window, object),
        Edit::Duplicate { object, offset_mm } => {
            (window.doc.scene.duplicate(object, offset_mm)).ok_or_else(|| no_object(object))?;
            Ok(None)
        }
        Edit::Mirror { object, axis } => place(window, object, |transform, mesh| {
            Ok(mirrored(transform, mesh, axis))
        }),
        _ => Ok(None),
    }
}

/// Sets the transform of `id` to what `to` makes of it and the object's own mesh.
fn place(
    window: &mut Window,
    id: ObjectId,
    to: impl FnOnce(Transform, &Mesh) -> Result<Transform, String>,
) -> Result<Option<ObjectId>, String> {
    let object = (window.doc.scene.get_mut(id)).ok_or_else(|| no_object(id))?;
    object.transform = to(object.transform, &object.mesh)?;
    Ok(None)
}
