//! What a revision asks of the plate beyond the one before it, as edits of the job's model
//! (ADR 0466): mended, repaired, pressed, sized, turned, labelled, split, cut, then finished.

use core_edit::{Edit, ObjectId, OrientAim, bridge_edges};
use serde_json::{Value, json};

use crate::recipe::{Goal, Mend, Modelling, Pose, Recipe, Relief, Repair};

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
    let (mend_now, mend_then) = (recipe.mend.as_ref(), earlier.mend.as_ref());
    let hand_now = by_hand(mend_now);
    if hand_now != by_hand(mend_then) {
        asked.push(mend(&hand_now?, object)?);
    }
    if recipe.repair != earlier.repair || limits(mend_now) != limits(mend_then) {
        if let Some(method) = recipe.repair {
            asked.push(repair(method, mend_now, object)?);
        } else if earlier.repair.is_some() {
            return None;
        }
    }
    let pressed = if recipe.relief == earlier.relief {
        None
    } else {
        let precision = (recipe.hollow.as_ref()).and_then(|hollow| hollow.precision);
        let precision = precision.unwrap_or_else(core_edit::defaults::lattice_precision);
        Some(relief(recipe.relief.as_ref()?, precision, object)?)
    };
    asked.extend(modelled(
        recipe.modelling.as_ref(),
        earlier.modelling.as_ref(),
        object,
    )?);
    // A turn and a relief go after a scale and before a label, laid where the model stands.
    let mut at = (asked.iter())
        .position(|edit| {
            matches!(
                edit,
                Edit::Label { .. } | Edit::Split { .. } | Edit::PartOff { .. } | Edit::Cut { .. }
            )
        })
        .unwrap_or(asked.len());
    if recipe.pose != earlier.pose {
        let turned = match (&recipe.pose, catalogue_ids(recipe)) {
            (Some(Pose::Rotate { degrees }), _) => rotated(object, *degrees),
            (Some(pose @ Pose::Search { .. }), Some(ids)) => {
                searched(pose, object, (ids, area_mm))?
            }
            (Some(Pose::Search { .. }), _) => rotated(object, chosen_deg?),
            (None, _) => return None,
        };
        asked.insert(at, turned);
        at += 1;
    }
    if let Some(pressed) = pressed {
        asked.insert(at, pressed);
    }
    asked.extend(finished(recipe, earlier, object, fit_drift_mm)?);
    if arrange_asked(recipe, earlier, &asked) {
        let (low_mm, plate_mm) = area_mm?;
        asked.push(Edit::Arrange {
            low_mm,
            plate_mm,
            clearance_mm: core_edit::defaults::arrange_clearance_mm(),
        });
    }
    Some(asked)
}

/// Whether `--arrange` is asked, which the command line runs last on the usable area: the
/// revision cuts the model into pieces, or asks it anew.
fn arrange_asked(recipe: &Recipe, earlier: &Recipe, asked: &[Edit]) -> bool {
    let wants = |recipe: &Recipe| (recipe.modelling.as_ref()).is_some_and(|m| m.arrange);
    let pieces = (asked.iter()).any(|edit| {
        matches!(
            edit,
            Edit::Cut { .. } | Edit::Split { .. } | Edit::PartOff { .. }
        )
    });
    wants(recipe) && (pieces || !wants(earlier))
}

fn rotated(object: ObjectId, degrees: [f32; 3]) -> Edit {
    Edit::Rotate {
        object,
        degrees: degrees.into(),
    }
}

/// The recipe's printer and resin where both are catalogue ids, not the paths of files; both
/// unnamed where the job runs on the standard ones.
fn catalogue_ids(recipe: &Recipe) -> Option<(Option<&str>, Option<&str>)> {
    fn id(name: &str) -> Option<&str> {
        (!name.contains(['/', '\\', '.'])).then_some(name)
    }
    if !recipe.names_a_machine() {
        return Some((None, None));
    }
    Some((Some(id(&recipe.printer)?), Some(id(&recipe.resin)?)))
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
    ((printer, resin), area_mm): ((Option<&str>, Option<&str>), Option<Area>),
) -> Option<Edit> {
    let Pose::Search {
        goal,
        max_tilt_deg,
        pick,
        clean_surface,
        keep_clear_at_mm,
        keep_clear_radius_mm,
        keep_clear_seen_deg,
    } = pose
    else {
        return None;
    };
    Some(Edit::Orient {
        object,
        goal: aim_of(*goal),
        max_tilt_deg: *max_tilt_deg,
        clean_surface: *clean_surface,
        pick: usize::from(pick.unwrap_or(1)).saturating_sub(1),
        plate_mm: area_mm.map(|(_, size)| size),
        low_mm: area_mm.map_or([0.0, 0.0], |(low, _)| low),
        printer: printer.map(str::to_owned),
        resin: resin.map(str::to_owned),
        keep_clear_mm: keep_clear_at_mm.iter().map(|&at| at.into()).collect(),
        keep_clear_radius_mm: *keep_clear_radius_mm,
        // As the command line reads them: seen at this turn, the model centred on the plate.
        keep_clear_seen_deg: (!keep_clear_at_mm.is_empty())
            .then(|| keep_clear_seen_deg.unwrap_or_default().into()),
        lift_mm: None,
    })
}

/// Whether nothing changed between the two that no edit carries: the way steps are decided, a
/// combination, the weld the model is read with, the lift it stands at, its copies, Check's
/// answers, or a step naming some objects, which runs by `--only`.
fn plate_alike(recipe: &Recipe, earlier: &Recipe) -> bool {
    // `arrange` is an edit of its own, asked after the rest; only a combination is not.
    let joined = |modelling: Option<&Modelling>| modelling.and_then(|m| m.combine.clone());
    let weld = |recipe: &Recipe| recipe.mend.as_ref().and_then(|m| m.weld_tolerance_mm);
    let copies = |recipe: &Recipe| recipe.modelling.as_ref().and_then(|m| m.copies);
    let lift = |recipe: &Recipe| recipe.supports.as_ref().and_then(|s| s.lift_mm);
    !recipe.scoped()
        && !earlier.scoped()
        && weld(recipe) == weld(earlier)
        && lift(recipe) == lift(earlier)
        && recipe.fixes == earlier.fixes
        && copies(recipe) == copies(earlier)
        && recipe.prepare == earlier.prepare
        && recipe.steps_by == earlier.steps_by
        && recipe.model == earlier.model
        && joined(recipe.modelling.as_ref()) == joined(earlier.modelling.as_ref())
}

/// The modelling edits in the command line's order: scale, label, cut, then split or part-off;
/// `None` where one asked earlier is taken back, which no edit undoes.
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
    if now.mirror != then.mirror {
        // Only a mirror asked anew is an edit; one taken back or changed is checked by flags.
        then.mirror.is_none().then_some(())?;
        asked.push(Edit::Mirror {
            object,
            axis: now.mirror?.into(),
        });
    }
    if now.move_mm != then.move_mm {
        let [to, from] = [now.move_mm, then.move_mm].map(Option::unwrap_or_default);
        asked.push(Edit::Move {
            object,
            by_mm: [to[0] - from[0], to[1] - from[1], to[2] - from[2]].into(),
        });
    }
    if now.label != then.label {
        let label = now.label.as_ref()?;
        asked.push(edit_of(json!({
            "edit": "label", "object": object, "text": label.text, "at_mm": label.at_mm,
            "height_mm": label.height_mm, "depth_mm": label.depth_mm, "raised": !label.sunk,
            "follow_surface": label.follow, "angle_deg": label.angle_deg.unwrap_or(0.0),
            "font": label.font,
        }))?);
    }
    if now.cut != then.cut {
        asked.push(now.cut.as_ref()?.edit(object));
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
    Some(asked)
}

/// The faces turned, bridged and dropped by hand, which a `Mend` edit carries; `None` for none.
fn by_hand(mend: Option<&Mend>) -> Option<Mend> {
    let hand = Mend {
        seam_tolerance_mm: None,
        smallest_shell_mm2: None,
        ..mend?.clone()
    };
    (hand != Mend::default()).then_some(hand)
}

/// The lengths a repair runs to, which a `Repair` edit carries.
fn limits(mend: Option<&Mend>) -> (Option<f32>, Option<f32>) {
    mend.map_or((None, None), |mend| {
        (mend.seam_tolerance_mm, mend.smallest_shell_mm2)
    })
}

fn repair(repair: Repair, mend: Option<&Mend>, object: ObjectId) -> Option<Edit> {
    let method = match repair {
        Repair::Full => json!({"mend": "repair"}),
        Repair::CloseHoles => json!({"mend": "close-holes"}),
        Repair::CloseSmallHoles => json!({"mend": "close-small-holes"}),
        Repair::Turn => json!({"mend": "turn"}),
        Repair::Wrap => json!("wrap"),
        Repair::Deep => json!("deep"),
    };
    let (seam_tolerance_mm, smallest_shell_mm2) = limits(mend);
    edit_of(json!({
        "edit": "repair", "object": object, "method": method,
        "seam_tolerance_mm": seam_tolerance_mm, "smallest_shell_mm2": smallest_shell_mm2,
    }))
}

/// The faces `mend` names by hand; a bridge not written `A-B:C-D` is no edit (`None`).
fn mend(mend: &Mend, object: ObjectId) -> Option<Edit> {
    let bridge: Option<Vec<[[u32; 2]; 2]>> =
        mend.bridge.iter().map(|text| bridge_edges(text)).collect();
    edit_of(json!({
        "edit": "mend", "object": object, "flip_faces": mend.flip_faces,
        "drop_faces": mend.drop_faces, "drop_shells": mend.drop_shells, "bridge": bridge?,
    }))
}

/// A picture tiled over the model; the file's own texture is no edit yet (`None`).
/// The relief `relief` asks, on the lattice `--precision` gives the command line's check.
fn relief(relief: &Relief, precision: f32, object: ObjectId) -> Option<Edit> {
    let image = relief.image.as_ref()?;
    let mut asked = json!({
        "edit": "relief", "object": object, "depth_mm": relief.depth_mm, "image": image,
        "invert": relief.invert, "symmetric": relief.symmetric, "precision": precision,
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

/// An edit written as the window reads it; `None`, said on stderr, for one this build's `Edit`
/// does not take, which leaves the revision to be shown by its check's plate.
fn edit_of(written: Value) -> Option<Edit> {
    serde_json::from_value(written)
        .map_err(|error| eprintln!("encrust-mcp: an edit this build cannot read: {error}"))
        .ok()
}

mod finished;

use finished::finished;
#[cfg(test)]
mod tests;
