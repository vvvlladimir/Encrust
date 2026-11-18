//! What a revision asks of the plate beyond the one before it, as edits of the job's model
//! (ADR 0466): repaired, mended, pressed, sized, turned, labelled, split, then cut.

use core_edit::{Edit, ObjectId};
use serde_json::{Value, json};

use crate::recipe::{Mend, Modelling, Pose, Recipe, Relief, Repair};

/// The edits `recipe` asks beyond `earlier` of `object`, in the order they are made; `None`
/// where something changed that no edit carries yet, so the revision is shown as before.
pub fn asked_edits(recipe: &Recipe, earlier: &Recipe, object: ObjectId) -> Option<Vec<Edit>> {
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
        let Some(Pose::Rotate { degrees }) = recipe.pose else {
            return None;
        };
        // A turn goes after a scale and before a label, which is laid where the model stands.
        let at = asked.iter().position(|edit| {
            matches!(
                edit,
                Edit::Label { .. } | Edit::Split { .. } | Edit::PartOff { .. } | Edit::Cut { .. }
            )
        });
        let turned = Edit::Rotate {
            object,
            degrees: degrees.into(),
        };
        asked.insert(at.unwrap_or(asked.len()), turned);
    }
    Some(asked)
}

/// Whether nothing changed between the two that no edit carries: the hollow, the drains,
/// the supports, the way steps are decided or a combination.
fn plate_alike(recipe: &Recipe, earlier: &Recipe) -> bool {
    // `arrange` comes with every cut and split the command line lays apart; the window lays
    // the pieces out itself, so only a combination is asked of the plate.
    let joined = |modelling: Option<&Modelling>| modelling.and_then(|m| m.combine.clone());
    recipe.hollow == earlier.hollow
        && recipe.drains == earlier.drains
        && recipe.supports == earlier.supports
        && recipe.prepare == earlier.prepare
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

/// An edit written as the window reads it; `None` for one this build's `Edit` does not take.
fn edit_of(written: Value) -> Option<Edit> {
    serde_json::from_value(written).ok()
}

#[cfg(test)]
mod tests;
