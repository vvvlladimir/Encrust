//! Schema 3 of the window link: the job's plate handed to a server as it stands, and the edits
//! it built on it applied there while the window is still that version (ADR 0466, 0477).

use std::collections::VecDeque;
use std::sync::mpsc::Sender;

use agent_link::ApplyReply;
use core_edit::{Edit, ObjectId};
use core_geometry::Transform;
use serde_json::Value;

use crate::panels::Window;
use crate::tool_settings::ToolSettings;
use crate::undo::edited_fingerprint;

mod arrange;
mod drain;
mod hand;
mod orient;
mod start;
mod supports;

pub use hand::hand_plate;
use orient::stand_pick;
use start::{no_object, start_edit};
use supports::{inside_pass_of, start_inside};

/// Edits being applied one after another, each a job of the tool that carries it out.
#[derive(Debug)]
pub struct Applying {
    edits: VecDeque<Edit>,
    /// The job's plate, shown while each edit starts.
    plate: Option<u32>,
    reply: Sender<Value>,
    /// The object the edit under way replaces, there still if the edit did not land, and
    /// whether the edit may leave it in place and be done (a split of one body).
    under_way: Option<(ObjectId, bool)>,
    /// The object whose cavity a supports edit holds up once its outside run has landed, whether
    /// that run is sized by the automatic profile, and whether it keeps off a clean surface.
    inside: Option<(ObjectId, bool, bool)>,
    /// The object whose orient search, once landed, is stepped to the pose asked for, and its
    /// number from 0.
    picking: Option<(ObjectId, usize)>,
    /// Where each object stood once the last edit started, but those it may move itself: one
    /// moved since was moved by the user, and the edits left were built on another plate.
    placed: Vec<(ObjectId, Transform)>,
    /// The plate as the last edit started it, which only the user changes while its tool runs.
    started: u64,
}

/// Why edits are refused while another run, Prepare's, holds the undo history.
const HELD: &str = "the window is preparing a plate; hand the edits over once it is done";

/// Why the edits left stop once the user changed the plate under them.
const CHANGED_UNDER: &str = "the plate was changed in the window while the edits were applied";

fn refused(why: String) -> Value {
    reply_of(&ApplyReply::refused(why))
}

fn reply_of(reply: &impl serde::Serialize) -> Value {
    serde_json::to_value(reply).unwrap_or_default()
}

/// Starts applying `edits` on the job's `plate` if the window is still at `base_version`, all
/// held for one undo entry; otherwise answers `stale` with the version it is at, for the server
/// to build again.
pub fn start(
    (edits, plate): (Vec<Edit>, Option<u32>),
    base_version: u64,
    window: &mut Window,
    reply: Sender<Value>,
) -> Option<Applying> {
    let version = window.doc.history.plate_version();
    if base_version != version {
        let _ = reply.send(reply_of(&ApplyReply::stale(version)));
        return None;
    }
    if window.doc.history.is_held() {
        let _ = reply.send(refused(HELD.to_owned()));
        return None;
    }
    let settings = ToolSettings::of(window.tools, &window.machine.slicing);
    let chosen = window.machine.slicing.chosen();
    (window.doc.history).hold(&window.doc.scene, &settings, chosen);
    Some(Applying {
        edits: edits.into(),
        plate,
        reply,
        under_way: None,
        inside: None,
        picking: None,
        placed: placed(window, None),
        started: edited_fingerprint(&window.doc.scene),
    })
}

/// Moves the edits along once the tool carrying the last one is done, answering when all
/// have landed or one did not. Answers whether edits are still being applied.
pub fn advance(applying: &mut Option<Applying>, window: &mut Window) -> bool {
    let Some(work) = applying.as_mut() else {
        return false;
    };
    let busy = crate::prepare::tool_busy(window.doc, window.tools);
    let changed = busy && edited_fingerprint(&window.doc.scene) != work.started;
    if changed || moved_by_hand(&work.placed, window) {
        // The user took the plate over: it stays as they left it, one undo entry.
        window.doc.history.release();
        return finish(applying, refused(CHANGED_UNDER.to_owned()));
    }
    if busy {
        return true;
    }
    if let Err(why) = land_last(work, window) {
        return taken_back(applying, window, why);
    }
    if work.under_way.is_some() {
        return true;
    }
    let Some(edit) = work.edits.pop_front() else {
        window.doc.history.release();
        return finish(applying, reply_of(&ApplyReply::landed()));
    };
    match begin(work, &edit, window) {
        Ok(()) => true,
        Err(why) => taken_back(applying, window, why),
    }
}

/// Puts the plate back as the edits found it, recording nothing, and answers why (ADR 0483).
fn taken_back(applying: &mut Option<Applying>, window: &mut Window, why: String) -> bool {
    let doc = &mut *window.doc;
    (doc.history).abandon(&mut doc.scene, window.tools, &mut window.machine.slicing);
    finish(applying, refused(why))
}

/// Lands what the last edit left once its tool is done: refused where the object it replaces
/// still stands, its pick stood, its pass inside the cavity started.
fn land_last(work: &mut Applying, window: &mut Window) -> Result<(), String> {
    if let Some((id, may_stay)) = work.under_way.take()
        && window.doc.scene.get(id).is_some()
        && !(may_stay && !window.machine.status.is_error())
    {
        return Err(window.machine.status.text());
    }
    if let Some((object, pick)) = work.picking.take() {
        stand_pick(window, object, pick)?;
    }
    if let Some(inside) = work.inside.take() {
        start_inside(window, inside)?;
        work.under_way = Some((inside.0, true));
    }
    Ok(())
}

/// Starts `edit`, noting what landing it waits on.
fn begin(work: &mut Applying, edit: &Edit, window: &mut Window) -> Result<(), String> {
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
    if let Some(plate) = work.plate {
        window.doc.scene.show_plate(plate);
    }
    let id = start_edit(edit, window)?;
    work.under_way = id.map(|id| (id, may_stay));
    work.inside = inside_pass_of(edit);
    work.picking = match *edit {
        Edit::Orient { object, pick, .. } => Some((object, pick)),
        _ => None,
    };
    work.placed = placed(window, Some(edit));
    work.started = edited_fingerprint(&window.doc.scene);
    Ok(())
}

/// Where each object stands, but those `edit` may move as it lands.
fn placed(window: &Window, edit: Option<&Edit>) -> Vec<(ObjectId, Transform)> {
    let moving = match edit {
        Some(Edit::Arrange { .. }) => return Vec::new(),
        Some(
            Edit::Orient { object, .. }
            | Edit::Relief { object, .. }
            | Edit::Supports { object, .. },
        ) => Some(*object),
        _ => None,
    };
    (window.doc.scene.objects().iter())
        .filter(|object| Some(object.id) != moving)
        .map(|object| (object.id, object.transform))
        .collect()
}

/// Whether an object in `placed` stands anywhere else now; one gone was taken by an edit.
fn moved_by_hand(placed: &[(ObjectId, Transform)], window: &Window) -> bool {
    (placed.iter()).any(|(id, at)| {
        window
            .doc
            .scene
            .get(*id)
            .is_some_and(|object| object.transform != *at)
    })
}

fn finish(applying: &mut Option<Applying>, answer: Value) -> bool {
    if let Some(work) = applying.take() {
        let _ = work.reply.send(answer);
    }
    false
}
