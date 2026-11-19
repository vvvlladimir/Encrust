//! The orient edit as the window carries it out: the Orient tool's own search on one model,
//! then the pose asked for stood.

use core_document::{ApplyError, preset_of};
use core_edit::{Edit, ObjectId};
use core_plate::Asked;

use super::no_object;
use crate::orient::Turning;
use crate::panels::Window;

/// Starts the Orient search on `object` alone and answers it; [`stand_pick`] stands the pose
/// the edit picks once it lands.
pub(super) fn start_orient(window: &mut Window, edit: &Edit) -> Result<Option<ObjectId>, String> {
    let &Edit::Orient {
        object,
        goal,
        max_tilt_deg,
        clean_surface,
        ..
    } = edit
    else {
        return Ok(None);
    };
    window
        .doc
        .scene
        .get(object)
        .ok_or_else(|| no_object(object))?;
    let wanted = Asked {
        preset: preset_of(goal),
        clean_surface,
    };
    let catalogue = &window.machine.slicing.catalogue;
    let plans = (window.tools.supports.orient_plans(catalogue)).map_err(|e| format!("{e:#}"))?;
    let tool = &mut window.tools.orient;
    tool.forget();
    let asked = (wanted, max_tilt_deg, plans);
    let started = tool.start_on(&window.doc.scene, &[object], asked, None);
    if let Turning::Search(job) = started.map_err(|e| format!("{e:#}"))? {
        tool.job = Some(job);
    }
    Ok(Some(object))
}

/// Stands `object` the `pick`th of every pose the landed search ranked, from 0; a search that
/// ended with none landed was cancelled.
pub(super) fn stand_pick(window: &mut Window, object: ObjectId, pick: usize) -> Result<(), String> {
    let (doc, tool) = (&mut window.doc, &mut window.tools.orient);
    tool.stand_ranked(&mut doc.scene, &doc.plate, object, pick)
        .map_err(|found| match found {
            0 => ApplyError::Cancelled.to_string(),
            found => ApplyError::NoChoice {
                object,
                pick,
                found,
            }
            .to_string(),
        })
}
