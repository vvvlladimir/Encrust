//! The drain edits as the window carries them out: holes, the search for more, channels and
//! plugs, each through the tool that does it for a hand.

use core_document::{ApplyError, channel_in, hole_near};
use core_edit::ObjectId;
use core_geometry::Vec3;
use core_volume::{HoleKind, HoleSize};

use super::no_object;
use crate::drain::DrainTool;
use crate::panels::Window;

/// Drills the holes at `at_mm` at once; with `auto` starts the Drain search on `object`
/// alone and answers it, else `None`.
pub(super) fn start_drain(
    window: &mut Window,
    object: ObjectId,
    at_mm: &[Vec3],
    (size, kind): (HoleSize, HoleKind),
    (auto, vent): (bool, bool),
) -> Result<Option<ObjectId>, String> {
    let model = (window.doc.scene.get_mut(object)).ok_or_else(|| no_object(object))?;
    if auto && !model.hollow.is_hollow() {
        return Err(ApplyError::NoCavity(object).to_string());
    }
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
    if !auto {
        return Ok(None);
    }
    let layer_mm = window.machine.slicing.layer_height_mm();
    let job = DrainTool::search_on(&window.doc.scene, &[object], layer_mm, (size, vent))
        .map_err(|error| format!("{error:#}"))?;
    window.tools.drain.auto = Some(job);
    Ok(Some(object))
}

/// Digs a channel of `diameter_mm` along `points_mm` through `object` at once.
pub(super) fn start_channel(
    window: &mut Window,
    object: ObjectId,
    points_mm: &[Vec3],
    diameter_mm: f32,
) -> Result<Option<ObjectId>, String> {
    let model = (window.doc.scene.get_mut(object)).ok_or_else(|| no_object(object))?;
    let channel = channel_in(model.transform, points_mm, diameter_mm, object)
        .map_err(|error| error.to_string())?;
    model.hollow.add_channel(channel);
    Ok(None)
}

/// Sets the gap of `object`'s plugs to `gap_mm` and starts cutting the plugs its holes ask
/// for, as the next frame would; the cut lands into the edit that asked for it.
pub(super) fn start_plugs(
    window: &mut Window,
    object: ObjectId,
    gap_mm: f32,
) -> Result<Option<ObjectId>, String> {
    let model = (window.doc.scene.get_mut(object)).ok_or_else(|| no_object(object))?;
    model.own.plug_gap_mm = Some(gap_mm);
    let drift_mm = window.machine.slicing.material.compensation.fit_drift_mm();
    let asked_by = window.doc.history.next_edit();
    window.tools.drain.lay_plugs(
        &mut window.doc.scene,
        drift_mm,
        true,
        asked_by,
        &mut window.doc.drilling.reads,
    );
    Ok(None)
}
