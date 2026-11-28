//! Each edit started through the tool that carries it out for a hand.

use core_document::{
    ApplyError, CutAsked, LabelAsked, MendAsked, ReliefAsked, blockers_along, mirrored, moved,
    part_settings, repair_settings, sized_to, turned_to,
};
use core_edit::{Edit, ObjectId, Op};
use core_engine::project::HollowState;
use core_geometry::{Mesh, Transform, Vec3};
use core_volume::{PartSettings, RepairMethod, RepairSettings};

use super::arrange::start_arrange;
use super::drain::{start_channel, start_drain, start_plugs};
use super::orient::start_orient;
use super::supports::{start_merge, start_supports};
use crate::job::Method;
use crate::panels::Window;
use crate::repair::RepairState;

/// Carries `edit` out: one that runs a tool job answers the object it replaces, any other is
/// made at once and answers `None`.
pub(super) fn start_edit(edit: &Edit, window: &mut Window) -> Result<Option<ObjectId>, String> {
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
            start_cut(window, object, asked)
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

/// Starts cutting `object` as the Cut tool's job.
fn start_cut(
    window: &mut Window,
    object: ObjectId,
    asked: CutAsked,
) -> Result<Option<ObjectId>, String> {
    (window.tools.cut)
        .start_asked(&window.doc.scene, object, asked)
        .map_err(|error| format!("{error:#}"))?;
    Ok(Some(object))
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
        .start_asked(&window.doc.scene, objects, (op, method))
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

pub(super) fn no_object(id: ObjectId) -> String {
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
