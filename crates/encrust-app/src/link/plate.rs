//! Schema 3 of the window link: the plate handed to a server as it stands, and the edits it
//! built on it applied while the plate is still that version (ADR 0466).

use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::Sender;

use core_document::{
    AUTO, ApplyError, CutAsked, LabelAsked, MendAsked, ReliefAsked, boolean_of, hole_near,
    mirrored, moved, part_settings, sized_to, turned_to,
};
use core_edit::{Edit, ObjectId, Op};
use core_engine::ShownAt;
use core_engine::project::HollowState;
use core_geometry::{Mesh, Region, Transform, Vec3};
use core_volume::{HoleKind, HoleSize, PartSettings, RepairMethod};
use printer_profiles::SupportProfile;
use serde_json::{Value, json};

use crate::job::Method;
use crate::panels::Window;
use crate::repair::RepairState;
use crate::scene::Scene;
use crate::supports::{Only, Side};

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
            | Edit::Supports { .. }
    );
    match start_edit(&edit, window) {
        Ok(id) => {
            work.under_way = id.map(|id| (id, may_stay));
            work.inside = inside_pass_of(&edit);
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
}

fn finish(applying: &mut Option<Applying>, answer: Value) -> bool {
    if let Some(work) = applying.take() {
        let _ = work.reply.send(answer);
    }
    false
}

/// Carries `edit` out: a cut, combine, label, repair, mend, relief, hollow or supports run
/// starts a tool job, answering the object it replaces; any other edit is made at once,
/// answering `None`.
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
        Edit::Repair { object, method } => start_repair(window, object, method),
        Edit::Mend { object, .. } => start_mend(window, edit, object),
        Edit::Relief { object, .. } => start_relief(window, edit, object),
        Edit::Hollow {
            object,
            ref settings,
        } => start_hollow(window, object, settings),
        Edit::Drain {
            object,
            ref at_mm,
            size,
            kind,
        } => start_drain(window, object, at_mm, (size, kind)),
        Edit::Supports { object, .. } => start_supports(window, edit, object),
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
    method: RepairMethod,
) -> Result<Option<ObjectId>, String> {
    window
        .doc
        .scene
        .get(object)
        .ok_or_else(|| no_object(object))?;
    let state = RepairState {
        method,
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
    settings: &HollowState,
) -> Result<Option<ObjectId>, String> {
    let model = (window.doc.scene.get_mut(object)).ok_or_else(|| no_object(object))?;
    if model.mesh.is_empty() {
        return Err(format!("object {object:?} has no surface to hollow"));
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

/// Drills a hole into the surface of `object` nearest each of `at_mm` at once, answering
/// `None`; the window's drilling job cuts them on its next frame.
fn start_drain(
    window: &mut Window,
    object: ObjectId,
    at_mm: &[Vec3],
    (size, kind): (HoleSize, HoleKind),
) -> Result<Option<ObjectId>, String> {
    let model = (window.doc.scene.get_mut(object)).ok_or_else(|| no_object(object))?;
    let inverse = (model.transform.inverse_matrix())
        .ok_or_else(|| ApplyError::BadTransform(object).to_string())?;
    let mut holes = Vec::with_capacity(at_mm.len());
    for plate in at_mm {
        let own = inverse.transform_point3(*plate);
        let near = (own, Vec3::ZERO);
        let hole = hole_near(&model.mesh, &model.bvh, near, (size, kind), object);
        holes.push(hole.map_err(|error| error.to_string())?);
    }
    for hole in holes {
        model.hollow.add_hole(hole);
    }
    Ok(None)
}

/// Stands a support at the surface nearest each of the edit's points at once, in the group of
/// the preset it names, the active one where unset; unless `only_at` is set, starts the
/// Supports tool's automatic run on `object` alone, answering it. `auto` sizes that run as the
/// panel's Auto does; strokes of `shown_at` count as the side meant to be seen for it alone.
/// The growth job grows the trees.
fn start_supports(
    window: &mut Window,
    edit: &Edit,
    object: ObjectId,
) -> Result<Option<ObjectId>, String> {
    let Edit::Supports {
        profile,
        at_mm,
        only_at,
        shown_at,
        ..
    } = edit
    else {
        return Err(no_object(object));
    };
    let scene = &mut window.doc.scene;
    scene.get(object).ok_or_else(|| no_object(object))?;
    let tool = &mut window.tools.supports;
    let named = profile.as_deref().filter(|name| *name != AUTO);
    if let Some(name) = named {
        let shipped = SupportProfile::shipped(name)
            .map_err(|_| ApplyError::NoProfile(name.to_owned()).to_string())?;
        tool.chosen = Some(shipped);
        tool.take_up_chosen(scene);
    }
    stand_points(scene, object, at_mm, tool.active)?;
    if *only_at {
        return Ok(None);
    }
    let clean = window.tools.orient.settings.clean_surface || !shown_at.is_empty();
    let before = brush_shown(&mut window.doc.scene, object, shown_at);
    let sized = profile.as_deref() == Some(AUTO);
    let started = run_on(window, object, (Side::Outside, sized), clean);
    if let (Some(before), Some(model)) = (before, window.doc.scene.get_mut(object)) {
        model.shown_painted = before;
    }
    started?;
    Ok(Some(object))
}

/// Stands a support in `group` at the surface of `object` nearest each of `at_mm`.
fn stand_points(
    scene: &mut Scene,
    object: ObjectId,
    at_mm: &[Vec3],
    group: u16,
) -> Result<(), String> {
    let model = scene.get_mut(object).ok_or_else(|| no_object(object))?;
    let inverse = (model.transform.inverse_matrix())
        .ok_or_else(|| ApplyError::BadTransform(object).to_string())?;
    for plate in at_mm {
        let near = inverse.transform_point3(*plate);
        let closest = (model.support_surface().1.closest(near))
            .ok_or_else(|| ApplyError::NoSurface(object).to_string())?;
        let contact = model.transform.to_matrix().transform_point3(closest.point);
        model.supports.add(contact, model.transform, group);
    }
    Ok(())
}

/// Paints the strokes `shown_at` on `object` where it has no painted side of its own, as the
/// engine does, and answers what it was before, to put back once the run has started.
fn brush_shown(scene: &mut Scene, object: ObjectId, shown_at: &[ShownAt]) -> Option<Region> {
    let model = scene.get_mut(object)?;
    if shown_at.is_empty() || !model.shown_painted.is_empty() {
        return None;
    }
    let before = model.shown_painted.clone();
    for stroke in shown_at {
        let on = (&*model.mesh, &*model.bvh, model.transform);
        model
            .shown_painted
            .brush(on, stroke.at_mm, stroke.radius_mm, true);
    }
    Some(before)
}

/// Starts the Supports tool's automatic run on `object` alone, adding to what stands. A sized
/// run takes the plate's automatic profile, else the catalogue's first, without keeping it.
fn run_on(
    window: &mut Window,
    object: ObjectId,
    (side, sized): (Side, bool),
    clean: bool,
) -> Result<(), String> {
    let (layer_mm, catalogue) = (
        window.machine.slicing.layer_height_mm(),
        &window.machine.slicing.catalogue,
    );
    let tool = &mut window.tools.supports;
    let kept = tool.auto.clone();
    if sized && kept.is_none() {
        let held = tool.sizing().map(|sizing| sizing.profile);
        let first = || catalogue.autos().next().map(|entry| entry.profile.clone());
        tool.auto = Some(
            held.or_else(first)
                .ok_or_else(|| "the catalogue has no automatic profile".to_owned())?,
        );
    }
    let only = Only {
        ids: &[object],
        sized,
    };
    let started = tool.start_for(
        &mut window.doc.scene,
        layer_mm,
        catalogue,
        (side, clean),
        Some(only),
    );
    tool.auto = kept;
    started.map_err(|error| format!("{error:#}"))
}

/// The run holding up the cavity that a supports edit asking for it follows its outside run
/// with: the object and whether `auto` sizes it.
fn inside_pass_of(edit: &Edit) -> Option<(ObjectId, bool)> {
    match edit {
        Edit::Supports {
            object,
            profile,
            only_at: false,
            cavity: true,
            ..
        } => Some((*object, profile.as_deref() == Some(AUTO))),
        _ => None,
    }
}

/// Starts the panel's inside pass on `object`, which holds up its cavity.
fn start_inside(window: &mut Window, object: ObjectId, sized: bool) -> Result<(), String> {
    let clean = window.tools.orient.settings.clean_surface;
    run_on(window, object, (Side::Inside, sized), clean)
}

fn no_object(id: ObjectId) -> String {
    ApplyError::NoObject(id).to_string()
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
