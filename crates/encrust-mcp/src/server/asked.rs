//! What a revision asks of the plate beyond the one before it, as edits of the job's model
//! (ADR 0466): repaired, mended, pressed, sized, turned, labelled, split, cut, hollowed, drained, channelled, plugged.

use core_edit::{Edit, ObjectId, OrientAim, PLUG_GAP_MM};
use serde_json::{Value, json};

use crate::recipe::{
    Drains, Goal, HoleKind, Hollow, Mend, Modelling, Pose, Recipe, Relief, Repair, Supports,
};

/// What `--precision` is when not typed.
const PRECISION: f32 = 0.5;
/// What `--infill-cell` and `--infill-density` are when not typed.
const INFILL_CELL_MM: f32 = 8.0;
const INFILL_DENSITY: f32 = 0.25;
/// The radius `--shown-radius` brushes what shows at when not typed, mm.
const SHOWN_RADIUS_MM: f32 = 4.0;
/// How far past the surface `--drain` reaches when not typed, mm.
const DRAIN_DEPTH_MM: f32 = 3.0;

/// What the check of a revision reports for the edits: the turn its search chose, the resin's fit
/// drift and the printer's usable area as its low corner and size, mm.
pub type Checked = (Option<[f32; 3]>, Option<f32>, Option<Area>);

/// A plate area as its low corner and its size, mm.
pub type Area = ([f32; 2], [f32; 2]);

/// The edits `recipe` asks beyond `earlier` of `object`, `None` where one is no edit yet; the
/// check's turn and resin fit drift (`slicing.fit_drift_mm`) let a pose and a plug gap be one.
pub fn asked_after_check(
    recipe: &Recipe,
    earlier: &Recipe,
    object: ObjectId,
    (chosen_deg, fit_drift_mm, area_mm): Checked,
) -> Option<Vec<Edit>> {
    if !plate_alike(recipe, earlier) {
        return None;
    }
    let mut asked = Vec::new();
    if recipe.repair != earlier.repair {
        asked.push(repair(recipe.repair?, object)?);
    }
    if recipe.mend != earlier.mend {
        asked.push(mend(recipe.mend.as_ref()?, object)?);
    }
    if recipe.relief != earlier.relief {
        asked.push(relief(recipe.relief.as_ref()?, object)?);
    }
    asked.extend(modelled(
        recipe.modelling.as_ref(),
        earlier.modelling.as_ref(),
        object,
    )?);
    if recipe.pose != earlier.pose {
        let turned = match (&recipe.pose, catalogue_ids(recipe)) {
            (Some(Pose::Rotate { degrees }), _) => rotated(object, *degrees),
            (Some(pose @ Pose::Search { .. }), Some(ids)) if keeps_nothing_clear(pose) => {
                searched(pose, object, (ids, area_mm))?
            }
            (Some(Pose::Search { .. }), _) => rotated(object, chosen_deg?),
            (None, _) => return None,
        };
        // A turn goes after a scale and before a label, which is laid where the model stands.
        let at = asked.iter().position(|edit| {
            matches!(
                edit,
                Edit::Label { .. } | Edit::Split { .. } | Edit::PartOff { .. } | Edit::Cut { .. }
            )
        });
        asked.insert(at.unwrap_or(asked.len()), turned);
    }
    if recipe.hollow != earlier.hollow {
        asked.push(hollow(recipe.hollow.as_ref()?, object)?);
    }
    if recipe.drains != earlier.drains {
        asked.extend(drained(
            recipe.drains.as_ref()?,
            earlier.drains.as_ref(),
            (object, fit_drift_mm),
        )?);
    }
    if recipe.supports != earlier.supports {
        let then = earlier.supports.as_ref();
        asked.push(supported(recipe.supports.as_ref()?, then, object)?);
    }
    Some(asked)
}

fn rotated(object: ObjectId, degrees: [f32; 3]) -> Edit {
    Edit::Rotate {
        object,
        degrees: degrees.into(),
    }
}

/// The recipe's printer and resin where both are catalogue ids, not the paths of files.
fn catalogue_ids(recipe: &Recipe) -> Option<(&str, &str)> {
    fn id(name: &str) -> Option<&str> {
        let path = name.is_empty() || name.contains(['/', '\\', '.']);
        (!path).then_some(name)
    }
    Some((id(&recipe.printer)?, id(&recipe.resin)?))
}

/// Whether a search keeps no place clear, which an `Orient` edit cannot be told.
fn keeps_nothing_clear(pose: &Pose) -> bool {
    matches!(pose, Pose::Search { keep_clear_at_mm, .. } if keep_clear_at_mm.is_empty())
}

fn aim_of(goal: Goal) -> OrientAim {
    match goal {
        Goal::FewestSupports => OrientAim::FewestSupports,
        Goal::LeastPeel => OrientAim::LeastPeel,
        Goal::ShortestPrint => OrientAim::ShortestPrint,
        Goal::Auto => OrientAim::Auto,
    }
}

/// The search `pose` as an edit proved on the job's printer and resin, as its check ran it.
fn searched(
    pose: &Pose,
    object: ObjectId,
    ((printer, resin), area_mm): ((&str, &str), Option<Area>),
) -> Option<Edit> {
    let Pose::Search {
        goal,
        max_tilt_deg,
        pick,
        ..
    } = pose
    else {
        return None;
    };
    Some(Edit::Orient {
        object,
        goal: aim_of(*goal),
        max_tilt_deg: *max_tilt_deg,
        clean_surface: false,
        pick: usize::from(pick.unwrap_or(1)).saturating_sub(1),
        plate_mm: area_mm.map(|(_, size)| size),
        low_mm: area_mm.map_or([0.0, 0.0], |(low, _)| low),
        printer: Some(printer.to_owned()),
        resin: Some(resin.to_owned()),
    })
}

/// Whether nothing changed between the two that no edit carries: the way steps are decided or
/// a combination.
fn plate_alike(recipe: &Recipe, earlier: &Recipe) -> bool {
    // `arrange` comes with every cut and split the command line lays apart; the window lays
    // the pieces out itself, so only a combination is asked of the plate.
    let joined = |modelling: Option<&Modelling>| modelling.and_then(|m| m.combine.clone());
    recipe.prepare == earlier.prepare
        && recipe.steps_by == earlier.steps_by
        && recipe.model == earlier.model
        && joined(recipe.modelling.as_ref()) == joined(earlier.modelling.as_ref())
}

/// The modelling edits, a part taken off before the cut; `None` where one asked earlier is
/// taken back, which no edit undoes.
fn modelled(
    now: Option<&Modelling>,
    then: Option<&Modelling>,
    object: ObjectId,
) -> Option<Vec<Edit>> {
    let blank = Modelling::default();
    let (now, then) = (now.unwrap_or(&blank), then.unwrap_or(&blank));
    let mut asked = Vec::new();
    if now.scale != then.scale {
        asked.push(Edit::Scale {
            object,
            factors: now.scale?.into(),
        });
    }
    if now.label != then.label {
        let label = now.label.as_ref()?;
        asked.push(edit_of(json!({
            "edit": "label", "object": object, "text": label.text, "at_mm": label.at_mm,
            "height_mm": label.height_mm, "depth_mm": label.depth_mm, "raised": !label.sunk,
            "follow_surface": label.follow, "angle_deg": label.angle_deg.unwrap_or(0.0),
        }))?);
    }
    if now.split_smallest_mm3 != then.split_smallest_mm3 {
        asked.push(Edit::Split {
            object,
            smallest_mm3: Some(now.split_smallest_mm3?),
        });
    }
    if now.part_off_at_mm != then.part_off_at_mm {
        asked.push(Edit::PartOff {
            object,
            at_mm: now.part_off_at_mm?.into(),
        });
    }
    if now.cut != then.cut {
        asked.push(now.cut.as_ref()?.edit(object));
    }
    Some(asked)
}

fn repair(repair: Repair, object: ObjectId) -> Option<Edit> {
    let method = match repair {
        Repair::Full => json!({"mend": "repair"}),
        Repair::CloseHoles => json!({"mend": "close-holes"}),
        Repair::CloseSmallHoles => json!({"mend": "close-small-holes"}),
        Repair::Turn => json!({"mend": "turn"}),
        Repair::Wrap => json!("wrap"),
        Repair::Deep => json!("deep"),
    };
    edit_of(json!({"edit": "repair", "object": object, "method": method}))
}

fn mend(mend: &Mend, object: ObjectId) -> Option<Edit> {
    edit_of(json!({
        "edit": "mend", "object": object, "flip_faces": mend.flip_faces,
        "drop_faces": mend.drop_faces, "drop_shells": mend.drop_shells,
    }))
}

/// A picture tiled over the model; the file's own texture is no edit yet (`None`).
fn relief(relief: &Relief, object: ObjectId) -> Option<Edit> {
    let image = relief.image.as_ref()?;
    let mut asked = json!({
        "edit": "relief", "object": object, "depth_mm": relief.depth_mm, "image": image,
        "invert": relief.invert, "symmetric": relief.symmetric,
    });
    let given = [
        ("tile_mm", relief.tile_mm),
        ("angle_deg", relief.tile_angle_deg),
        ("smoothing_px", relief.smooth_px),
        ("hold_bottom_deg", relief.hold_bottom_deg),
        ("hold_top_deg", relief.hold_top_deg),
    ];
    for (name, value) in given {
        if let (Some(value), Some(fields)) = (value, asked.as_object_mut()) {
            fields.insert(name.into(), json!(value));
        }
    }
    edit_of(asked)
}

/// The window's numbers for a hollow; walls kept solid at points are no edit yet (`None`).
fn hollow(hollow: &Hollow, object: ObjectId) -> Option<Edit> {
    if !hollow.keep_solid_at_mm.is_empty() {
        return None;
    }
    let pattern = hollow.infill.map(|infill| json!(infill));
    edit_of(json!({
        "edit": "hollow", "object": object, "settings": {
            "thickness_mm": hollow.wall_mm,
            "mode": hollow.mode.map_or(json!("internal"), |mode| json!(mode)),
            "precision": hollow.precision.unwrap_or(PRECISION),
            "infill_on": pattern.is_some(),
            "infill": {
                "pattern": pattern.unwrap_or(json!("hive")),
                "cell_mm": hollow.infill_cell_mm.unwrap_or(INFILL_CELL_MM),
                "density": hollow.infill_density.unwrap_or(INFILL_DENSITY),
            },
            "open_floor": hollow.open_floor,
        },
    }))
}

/// The holes, search, channels and plugs `now` asks beyond `then`; holes into the supports,
/// walled holes, a size changed or anything taken back are no edit yet (`None`).
fn drained(
    now: &Drains,
    then: Option<&Drains>,
    (object, fit_drift_mm): (ObjectId, Option<f32>),
) -> Option<Vec<Edit>> {
    let blank = Drains::default();
    let first = then.is_none();
    let then = then.unwrap_or(&blank);
    // What a later revision may only add to; the rest must stay as it was.
    let fixed = |drains: &Drains| Drains {
        at_mm: Vec::new(),
        auto: false,
        vent: true,
        plugs: false,
        channels: Vec::new(),
        ..drains.clone()
    };
    let alike = first || fixed(now) == fixed(then) && (!then.auto || now.vent == then.vent);
    let nothing_taken_back = now.at_mm.starts_with(&then.at_mm)
        && now.channels.starts_with(&then.channels)
        && now.auto >= then.auto
        && now.plugs >= then.plugs;
    let drain_kind = matches!(now.kind, None | Some(HoleKind::Drain));
    if !(alike && nothing_taken_back && drain_kind && now.support_at_mm.is_empty()) {
        return None;
    }
    let mut asked = Vec::new();
    let points = &now.at_mm[then.at_mm.len()..];
    let search = now.auto && !then.auto;
    if !points.is_empty() || search {
        asked.push(holes(now, points, search, object)?);
    }
    for points_mm in &now.channels[then.channels.len()..] {
        let mut channel = json!({"edit": "channel", "object": object, "points_mm": points_mm});
        if let Some(diameter_mm) = now.channel_mm {
            channel["diameter_mm"] = json!(diameter_mm);
        }
        asked.push(edit_of(channel)?);
    }
    if now.plugs && (!then.plugs || !asked.is_empty()) {
        // An edit's gap is the final one; `--plug-gap` takes the resin's drift off its own.
        let gap_mm = now.plug_gap_mm.unwrap_or(PLUG_GAP_MM) - fit_drift_mm?;
        asked.push(edit_of(
            json!({"edit": "plugs", "object": object, "gap_mm": gap_mm}),
        )?);
    }
    Some(asked)
}

/// A `Drain` edit drilling `points` of `drains`' size, the engine's search too where `search`.
fn holes(drains: &Drains, points: &[[f32; 3]], search: bool, object: ObjectId) -> Option<Edit> {
    let shape = drains
        .sides
        .map_or(json!("round"), |sides| json!({"polygon": {"sides": sides}}));
    edit_of(json!({
        "edit": "drain", "object": object, "at_mm": points,
        "auto": search, "vent": search && drains.vent,
        "size": {
            "diameter_mm": drains.diameter_mm,
            "depth_mm": drains.depth_mm.unwrap_or(DRAIN_DEPTH_MM),
            "taper": drains.taper.unwrap_or(1.0),
            "shape": shape,
            "turn_deg": drains.turn_deg.unwrap_or(0.0),
        },
    }))
}

/// Supports by a preset or sized automatically, a cavity's inside and what shows with them, and
/// points placed by hand beyond those `then` placed; a change of any of those is no edit yet.
fn supported(now: &Supports, then: Option<&Supports>, object: ObjectId) -> Option<Edit> {
    let (at_mm, only_at, whole_run) = match then {
        None => (now.at_mm.as_slice(), now.only_at, true),
        Some(then) => {
            let only_points_added = Supports {
                at_mm: then.at_mm.clone(),
                ..now.clone()
            } == *then;
            if !only_points_added || !now.at_mm.starts_with(&then.at_mm) {
                return None;
            }
            (&now.at_mm[then.at_mm.len()..], true, false)
        }
    };
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

/// An edit written as the window reads it; `None` for one this build's `Edit` does not take.
fn edit_of(written: Value) -> Option<Edit> {
    serde_json::from_value(written).ok()
}

#[cfg(test)]
mod tests;
