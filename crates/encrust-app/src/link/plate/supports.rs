//! The supports edit as the window carries it out: points stood, then the tool's run, each
//! through the tool that does it for a hand.

use core_document::{ApplyError, support_near};
use core_edit::{AUTO, Edit, ObjectId};
use core_engine::ShownAt;
use core_geometry::{Region, Vec3};
use printer_profiles::SupportProfile;

use super::no_object;
use crate::panels::{Window, merge_supports};
use crate::scene::Scene;
use crate::supports::{Only, Side, SupportTool};

/// Stands the edit's points in the group of the profile it names, then starts the tool's run
/// on `object` alone unless `only_at`; `auto` sizes it as the panel's Auto does.
pub(super) fn start_supports(
    window: &mut Window,
    edit: &Edit,
    object: ObjectId,
) -> Result<Option<ObjectId>, String> {
    let Edit::Supports {
        profile,
        at_mm,
        only_at,
        shown_at,
        clean_surface,
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
        take_up_named(tool, scene, name)?;
    }
    stand_points(scene, object, at_mm, tool.active)?;
    if *only_at {
        return Ok(None);
    }
    let clean = *clean_surface || !shown_at.is_empty();
    let before = brush_shown(&mut window.doc.scene, object, shown_at);
    let sized = profile.as_deref() == Some(AUTO);
    let started = run_on(window, object, (Side::Outside, sized), clean);
    if let (Some(before), Some(model)) = (before, window.doc.scene.get_mut(object)) {
        model.shown_painted = before;
    }
    started?;
    Ok(Some(object))
}

/// Makes the profile `name` names the group in hand: the plate's group of that name, else the
/// shipped preset as the panel's list takes it up.
fn take_up_named(tool: &mut SupportTool, scene: &mut Scene, name: &str) -> Result<(), String> {
    if let Some(group) = tool
        .groups
        .iter()
        .position(|group| group.profile.name == name)
    {
        tool.choose_group(group as u16);
        return Ok(());
    }
    let shipped = SupportProfile::shipped(name)
        .map_err(|_| ApplyError::NoProfile(name.to_owned()).to_string())?;
    tool.chosen = Some(shipped);
    tool.take_up_chosen(scene);
    Ok(())
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
        core_engine::ANALYSIS_LAYER_MM,
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
pub(super) fn inside_pass_of(edit: &Edit) -> Option<(ObjectId, bool)> {
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
pub(super) fn start_inside(
    window: &mut Window,
    object: ObjectId,
    sized: bool,
) -> Result<(), String> {
    let clean = window.tools.orient.settings.clean_surface;
    run_on(window, object, (Side::Inside, sized), clean)
}

/// Joins the supports of `object` whose contacts lie nearest `near_mm` into one at once, as
/// Merge into one joins those picked.
pub(super) fn start_merge(
    window: &mut Window,
    object: ObjectId,
    near_mm: &[Vec3],
) -> Result<Option<ObjectId>, String> {
    let model = (window.doc.scene.get_mut(object)).ok_or_else(|| no_object(object))?;
    let at_mm = near_mm.first().copied().unwrap_or_default();
    let mut trees = Vec::with_capacity(near_mm.len());
    for near in near_mm {
        let tree = support_near(model.supports.trees(), *near);
        let refused = ApplyError::NoSupportNear {
            object,
            at_mm: *near,
        };
        trees.push(tree.ok_or_else(|| refused.to_string())?);
    }
    trees.sort_unstable();
    trees.dedup();
    if trees.len() < 2 {
        return Err(ApplyError::OneSupportToMerge { object, at_mm }.to_string());
    }
    let frozen = model.supports.freeze_all(&trees, model.transform);
    let picked = frozen
        .into_iter()
        .flatten()
        .map(|at| (object, at))
        .collect();
    merge_supports(window, object, &picked)
        .map_err(|why| ApplyError::MergeRefused { object, at_mm, why }.to_string())?;
    Ok(None)
}
