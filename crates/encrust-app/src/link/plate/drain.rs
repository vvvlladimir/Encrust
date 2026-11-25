//! The drain edits as the window carries them out: holes, the search for more, channels and
//! plugs, each through the tool that does it for a hand.

use core_document::{ApplyError, channel_in, hole_near};
use core_edit::ObjectId;
use core_geometry::{Bvh, Vec3};
use core_volume::{DrainHole, HoleKind, HoleSize, hole_at};

use super::no_object;
use crate::drain::{DrainTool, PlugsLaid};
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
    if kind == HoleKind::Support {
        return drill_raft(window, object, at_mm, size).map(|()| None);
    }
    let mut holes = Vec::with_capacity(at_mm.len());
    for plate in at_mm {
        let on = (model.mesh.as_ref(), model.bvh.as_ref());
        let near = (*plate, Vec3::ZERO);
        let hole = hole_near(on, model.transform, near, (size, kind), object);
        holes.push(hole.map_err(|error| error.to_string())?);
    }
    for hole in holes {
        model.drill_hole(hole);
    }
    if !auto {
        return Ok(None);
    }
    let layer_mm = core_engine::ANALYSIS_LAYER_MM;
    let job = DrainTool::search_on(&window.doc.scene, &[object], layer_mm, (size, vent))
        .map_err(|error| format!("{error:#}"))?;
    window.tools.drain.auto = Some(job);
    Ok(Some(object))
}

/// Drills a hole of `size` into the raft under `object` nearest each of `at_mm`, kept with the
/// model's holes, as `--drain-support-at` drills it.
fn drill_raft(
    window: &mut Window,
    object: ObjectId,
    at_mm: &[Vec3],
    size: HoleSize,
) -> Result<(), String> {
    let model = (window.doc.scene.get_mut(object)).ok_or_else(|| no_object(object))?;
    let raft = (model.supports.raft()).ok_or_else(|| ApplyError::NoRaft(object).to_string())?;
    let bvh = Bvh::build(raft);
    let mut holes = Vec::with_capacity(at_mm.len());
    for near in at_mm {
        let hole = hole_at(raft, &bvh, *near, size)
            .ok_or_else(|| ApplyError::NoSurface(object).to_string())?;
        let hole = DrainHole {
            kind: HoleKind::Support,
            ..hole
        };
        let own = hole.into_model(model.transform);
        holes.push(own.ok_or_else(|| ApplyError::BadTransform(object).to_string())?);
    }
    for hole in holes {
        model.hollow.add_hole(hole);
    }
    Ok(())
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
    let drift_mm = window.machine.slicing.material.compensation.fit_drift_mm();
    model.own.plug_gap_mm = Some(gap_mm + drift_mm);
    // A model read from a plate keeps its plugs as read; an edit asking for plugs lays them anew.
    model.plugs_laid.get_or_insert_with(PlugsLaid::default);
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
