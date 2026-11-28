//! The hollow, holes and supports a revision asks beyond the one before, in the command line's
//! order: walled holes, the hollow, other holes, channels, plugs, supports, merges, raft holes.

use core_edit::defaults::{
    SHOWN_RADIUS_MM, drain_depth_mm, infill_cell_and_density, lattice_precision,
};
use core_edit::{AUTO, Edit, ObjectId, PLUG_GAP_MM};
use serde_json::{Value, json};

use super::{Stood, edit_of};
use crate::recipe::{Drains, HoleKind, Hollow, Recipe, Supports};

/// The edits finishing `object` that `recipe` asks beyond `earlier`, `None` where one is no
/// edit yet; `turned_from`, how the model stood before a new pose, if one is asked.
pub(super) fn finished(
    recipe: &Recipe,
    earlier: &Recipe,
    object: ObjectId,
    (fit_drift_mm, turned_from): (Option<f32>, Option<Stood>),
) -> Option<Vec<Edit>> {
    let drilled = if recipe.drains == earlier.drains {
        Drilled::default()
    } else {
        let then = earlier.drains.as_ref();
        drained(recipe.drains.as_ref()?, then, (object, fit_drift_mm))?
    };
    let mut asked = drilled.walled;
    // A walled hole is part of the shape the cavity is grown in, so the hollow follows it.
    let shape_changed = !asked.is_empty() && recipe.hollow.is_some();
    if recipe.hollow != earlier.hollow || shape_changed {
        asked.push(hollow(recipe.hollow.as_ref()?, object)?);
    }
    asked.extend(drilled.after_hollow);
    if let Some(stood) = turned_from {
        asked.extend(supported_anew(recipe, stood, object)?);
    } else if recipe.supports != earlier.supports {
        let then = earlier.supports.as_ref();
        asked.extend(supported(recipe.supports.as_ref()?, then, object)?);
    }
    asked.extend(drilled.into_raft);
    Some(asked)
}

/// The window's numbers for a hollow, each wall kept solid at a point a blocker there.
fn hollow(hollow: &Hollow, object: ObjectId) -> Option<Edit> {
    let pattern = hollow.infill.map(|infill| json!(infill));
    let keep_solid: Vec<Vec<[f32; 3]>> =
        hollow.keep_solid_at_mm.iter().map(|at| vec![*at]).collect();
    edit_of(json!({
        "edit": "hollow", "object": object, "settings": {
            "thickness_mm": hollow.wall_mm,
            "mode": hollow.mode.map_or(json!("internal"), |mode| json!(mode)),
            "precision": hollow.precision.unwrap_or_else(lattice_precision),
            "infill_on": pattern.is_some(),
            "infill": {
                "pattern": pattern.unwrap_or(json!("hive")),
                "cell_mm": hollow.infill_cell_mm.unwrap_or(infill_cell_and_density().0),
                "density": hollow.infill_density.unwrap_or(infill_cell_and_density().1),
            },
            "open_floor": hollow.open_floor,
            "solid_under_mm": hollow.keep_solid_under_mm,
        },
        "keep_solid": keep_solid, "keep_solid_reach_mm": hollow.keep_solid_reach_mm,
    }))
}

/// The holes a revision drills, by when each goes in.
#[derive(Default)]
struct Drilled {
    /// Into the solid model, before the hollow.
    walled: Vec<Edit>,
    /// Into the shell, with the channels and plugs.
    after_hollow: Vec<Edit>,
    /// Into the raft, once the supports stand.
    into_raft: Vec<Edit>,
}

/// The holes, search, channels and plugs `now` asks beyond `then`; a size changed or anything
/// taken back is no edit yet (`None`).
fn drained(
    now: &Drains,
    then: Option<&Drains>,
    (object, fit_drift_mm): (ObjectId, Option<f32>),
) -> Option<Drilled> {
    let blank = Drains::default();
    let then = match then {
        Some(then) if !only_added_to(now, then) => return None,
        then => then.unwrap_or(&blank),
    };
    let mut drilled = Drilled::default();
    let points = &now.at_mm[then.at_mm.len()..];
    let search = now.auto && !then.auto;
    let walled = now.kind == Some(HoleKind::Walled);
    if walled && !points.is_empty() {
        let hole = holes(now, points, (false, "walled"), object)?;
        drilled.walled.push(hole);
    }
    let points = if walled { &[] } else { points };
    if !points.is_empty() || search {
        let hole = holes(now, points, (search, "at-depth"), object)?;
        drilled.after_hollow.push(hole);
    }
    drilled.after_hollow.extend(channels(now, then, object)?);
    let any_hole = !drilled.walled.is_empty() || !drilled.after_hollow.is_empty();
    if now.plugs && (!then.plugs || any_hole) {
        // An edit's gap is the final one; `--plug-gap` takes the resin's drift off its own.
        let gap_mm = now.plug_gap_mm.unwrap_or(PLUG_GAP_MM) - fit_drift_mm?;
        let plugs = json!({"edit": "plugs", "object": object, "gap_mm": gap_mm});
        drilled.after_hollow.push(edit_of(plugs)?);
    }
    let raft = &now.support_at_mm[then.support_at_mm.len()..];
    if !raft.is_empty() {
        let hole = holes(now, raft, (false, "support"), object)?;
        drilled.into_raft.push(hole);
    }
    Some(drilled)
}

/// Whether `now` only adds points, channels, a search or plugs to `then`, the rest as it was.
fn only_added_to(now: &Drains, then: &Drains) -> bool {
    let fixed = |drains: &Drains| Drains {
        at_mm: Vec::new(),
        auto: false,
        vent: true,
        plugs: false,
        channels: Vec::new(),
        support_at_mm: Vec::new(),
        ..drains.clone()
    };
    let alike = fixed(now) == fixed(then) && (!then.auto || now.vent == then.vent);
    alike
        && now.at_mm.starts_with(&then.at_mm)
        && now.channels.starts_with(&then.channels)
        && now.support_at_mm.starts_with(&then.support_at_mm)
        && now.auto >= then.auto
        && now.plugs >= then.plugs
}

/// A `Channel` edit for each channel `now` lays beyond `then`.
fn channels(now: &Drains, then: &Drains, object: ObjectId) -> Option<Vec<Edit>> {
    let laid = &now.channels[then.channels.len()..];
    laid.iter()
        .map(|points_mm| {
            let mut channel = json!({"edit": "channel", "object": object, "points_mm": points_mm});
            if let Some(diameter_mm) = now.channel_mm {
                channel["diameter_mm"] = json!(diameter_mm);
            }
            edit_of(channel)
        })
        .collect()
}

/// A `Drain` edit drilling `points` of `drains`' size as `kind`, the engine's search too where
/// `search`.
fn holes(
    drains: &Drains,
    points: &[[f32; 3]],
    (search, kind): (bool, &str),
    object: ObjectId,
) -> Option<Edit> {
    let shape = drains
        .sides
        .map_or(json!("round"), |sides| json!({"polygon": {"sides": sides}}));
    edit_of(json!({
        "edit": "drain", "object": object, "at_mm": points, "kind": kind,
        "auto": search, "vent": search && drains.vent,
        "size": {
            "diameter_mm": drains.diameter_mm,
            "depth_mm": drains.depth_mm.unwrap_or_else(drain_depth_mm),
            "taper": drains.taper.unwrap_or(1.0),
            "shape": shape,
            "turn_deg": drains.turn_deg.unwrap_or(0.0),
        },
    }))
}

/// Supports, and merges beyond those `then` asked, as edits; a change of anything but the points
/// placed by hand and the merges is no edit yet.
fn supported(now: &Supports, then: Option<&Supports>, object: ObjectId) -> Option<Vec<Edit>> {
    let (at_mm, only_at, merges, whole_run) = match then {
        None => (&now.at_mm[..], now.only_at, &now.merge_at_mm[..], true),
        Some(then) => {
            let only_added = Supports {
                at_mm: then.at_mm.clone(),
                merge_at_mm: then.merge_at_mm.clone(),
                ..now.clone()
            } == *then;
            let extended = now.at_mm.starts_with(&then.at_mm)
                && now.merge_at_mm.starts_with(&then.merge_at_mm);
            if !only_added || !extended {
                return None;
            }
            let merges = &now.merge_at_mm[then.merge_at_mm.len()..];
            (&now.at_mm[then.at_mm.len()..], true, merges, false)
        }
    };
    let mut asked = Vec::new();
    if whole_run || !at_mm.is_empty() {
        asked.push(supports(now, (at_mm, only_at), whole_run, object)?);
    }
    for near_mm in merges {
        let merge = json!({"edit": "merge-supports", "object": object, "near_mm": near_mm});
        asked.push(edit_of(merge)?);
    }
    Some(asked)
}

/// Supports stood anew after a new pose as the flags would stand them for `recipe`; `None`
/// where only the flags can: a recipe's own steps, or points given on the old pose.
fn supported_anew(
    recipe: &Recipe,
    (stood, hollowed): Stood,
    object: ObjectId,
) -> Option<Vec<Edit>> {
    let cavity_held = hollowed || recipe.hollow.is_some();
    let (profile, cavity) = match &recipe.supports {
        Some(asked) if by_hand(asked) => return None,
        Some(asked) => (Some(asked.profile.as_str()), asked.cavity),
        None if recipe.steps_by.is_some() => return None,
        None if recipe.prepare => (Some(AUTO), cavity_held),
        None if stood => (None, cavity_held),
        None => return Some(Vec::new()),
    };
    let anew = json!({
        "edit": "supports", "object": object, "profile": profile, "cavity": cavity, "anew": true,
    });
    Some(vec![edit_of(anew)?])
}

/// Whether `asked` names points on the plate or asks for those alone.
fn by_hand(asked: &Supports) -> bool {
    !asked.at_mm.is_empty()
        || !asked.shown_at_mm.is_empty()
        || !asked.merge_at_mm.is_empty()
        || asked.only_at
}

/// A `Supports` edit by `now`'s profile at `at_mm`; a `whole_run` holds up the cavity and paints
/// what shows as well.
fn supports(
    now: &Supports,
    (at_mm, only_at): (&[[f32; 3]], bool),
    whole_run: bool,
    object: ObjectId,
) -> Option<Edit> {
    // What shows is brushed round each point at the radius asked, 4 mm when not named.
    let radius_mm = now.shown_radius_mm.unwrap_or(SHOWN_RADIUS_MM);
    let shown_at: Vec<Value> = (now.shown_at_mm.iter())
        .filter(|_| whole_run)
        .map(|at| json!({"at_mm": at, "radius_mm": radius_mm}))
        .collect();
    edit_of(json!({
        "edit": "supports", "object": object, "profile": now.profile,
        "at_mm": at_mm, "only_at": only_at, "cavity": now.cavity && whole_run,
        "shown_at": shown_at,
    }))
}
