//! What a revision asks of a plate its cut, split or part-off left in pieces, as edits of each
//! piece, so a change to one step is checked on the plate kept rather than staged anew.

use core_edit::{Edit, ObjectId};

use super::finished::finished;
use super::{Area, Stood, catalogue_ids, rotated, searched};
use crate::recipe::{Drains, Hollow, Pose, Recipe, Supports};

/// The edits `recipe` asks beyond `earlier` of every piece on the plate, each with how it stood
/// at the check; `None` where one is no edit yet, or the pieces themselves changed.
pub fn asked_of_pieces(
    recipe: &Recipe,
    earlier: &Recipe,
    pieces: &[(ObjectId, Stood)],
    (fit_drift_mm, area_mm): (Option<f32>, Option<Area>),
) -> Option<Vec<Edit>> {
    if pieces.is_empty() || !pieces_alike(recipe, earlier) {
        return None;
    }
    let turned = !Pose::same_ask(recipe.pose.as_ref(), earlier.pose.as_ref());
    let mut asked = Vec::new();
    for &(piece, stood) in pieces {
        if turned {
            asked.push(turn(recipe, piece, area_mm)?);
        }
        let (now, then) = (view_of(recipe, piece), view_of(earlier, piece));
        let turned_from = turned.then_some(stood);
        asked.extend(finished(&now, &then, piece, (fit_drift_mm, turned_from))?);
    }
    if !asked.is_empty() && recipe.modellings().any(|modelling| modelling.arrange) {
        let (low_mm, plate_mm) = area_mm?;
        asked.push(Edit::Arrange {
            low_mm,
            plate_mm,
            clearance_mm: core_edit::defaults::arrange_clearance_mm(),
        });
    }
    Some(asked)
}

/// Whether the two leave the same pieces and differ in nothing but what edits of a piece carry.
fn pieces_alike(recipe: &Recipe, earlier: &Recipe) -> bool {
    let modellings = |recipe: &Recipe| recipe.modellings().cloned().collect::<Vec<_>>();
    let lifts = |recipe: &Recipe| {
        let supports = recipe.supports.iter().chain(&recipe.more_supports);
        supports
            .filter_map(|supports| supports.lift_mm)
            .collect::<Vec<_>>()
    };
    recipe.makes_pieces()
        && modellings(recipe) == modellings(earlier)
        && lifts(recipe) == lifts(earlier)
        && recipe.mend == earlier.mend
        && recipe.repair == earlier.repair
        && recipe.relief == earlier.relief
        && recipe.fixes == earlier.fixes
        && recipe.prepare == earlier.prepare
        && recipe.steps_by == earlier.steps_by
        && recipe.model == earlier.model
}

/// The pose `recipe` asks as an edit of `piece`; a search turn the check chose is no edit.
fn turn(recipe: &Recipe, piece: ObjectId, area_mm: Option<Area>) -> Option<Edit> {
    match (&recipe.pose, catalogue_ids(recipe)) {
        (Some(Pose::Rotate { degrees }), _) => Some(rotated(piece, *degrees)),
        (Some(pose @ Pose::Search { .. }), Some(ids)) => searched(pose, piece, (ids, area_mm)),
        _ => None,
    }
}

/// `recipe` as it shapes `piece` alone: the hollow, holes and supports asked last for it, none
/// naming objects.
fn view_of(recipe: &Recipe, piece: ObjectId) -> Recipe {
    Recipe {
        hollow: step_for(
            recipe.hollow.as_ref(),
            &recipe.more_hollow,
            piece,
            |h: &Hollow| &h.objects,
        )
        .map(|hollow| Hollow {
            objects: Vec::new(),
            ..hollow
        }),
        more_hollow: Vec::new(),
        drains: step_for(
            recipe.drains.as_ref(),
            &recipe.more_drains,
            piece,
            |d: &Drains| &d.objects,
        )
        .map(|drains| Drains {
            objects: Vec::new(),
            ..drains
        }),
        more_drains: Vec::new(),
        supports: step_for(
            recipe.supports.as_ref(),
            &recipe.more_supports,
            piece,
            |s: &Supports| &s.objects,
        )
        .map(|supports| Supports {
            objects: Vec::new(),
            ..supports
        }),
        more_supports: Vec::new(),
        ..recipe.clone()
    }
}

/// The step that shapes `piece`: the last asked for some objects naming it, else the one asked
/// for every object or for some naming it.
fn step_for<T: Clone>(
    first: Option<&T>,
    more: &[T],
    piece: ObjectId,
    objects: impl Fn(&T) -> &Vec<u64>,
) -> Option<T> {
    let names = |step: &&T| objects(step).contains(&piece.0);
    if let Some(step) = more.iter().rev().find(names) {
        return Some(step.clone());
    }
    first
        .filter(|step| objects(step).is_empty() || names(step))
        .cloned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::recipe::Modelling;

    const HALVES: [(ObjectId, Stood); 2] =
        [(ObjectId(2), (false, false)), (ObjectId(3), (false, false))];

    const AREA: Area = ([0.0, 0.0], [218.0, 122.0]);

    fn cut_in_half() -> Recipe {
        let cut = serde_json::from_value(serde_json::json!({"axis": "x", "at_mm": 100.0}))
            .expect("a cut reads");
        Recipe {
            modelling: Some(Modelling {
                cut: Some(cut),
                arrange: true,
                ..Modelling::default()
            }),
            hollow: Some(Hollow {
                wall_mm: 2.0,
                ..Hollow::default()
            }),
            ..Recipe::bare(
                PathBuf::from("/models/statue.3mf"),
                String::new(),
                String::new(),
            )
        }
    }

    fn objects_of(edits: &[Edit]) -> Vec<ObjectId> {
        edits.iter().filter_map(Edit::object).collect()
    }

    #[test]
    fn supports_asked_for_every_piece_are_an_edit_of_each_then_the_spread() {
        let earlier = cut_in_half();
        let recipe = Recipe {
            supports: Some(Supports {
                profile: "auto".into(),
                cavity: true,
                ..Supports::default()
            }),
            ..earlier.clone()
        };

        let edits = asked_of_pieces(&recipe, &earlier, &HALVES, (Some(0.0), Some(AREA)))
            .expect("supports are edits of each piece");

        assert_eq!(objects_of(&edits), [ObjectId(2), ObjectId(3)], "{edits:?}");
        assert!(
            matches!(edits.last(), Some(Edit::Arrange { .. })),
            "{edits:?}"
        );
    }

    #[test]
    fn a_hole_asked_for_one_piece_is_drilled_into_that_piece_alone() {
        let auto = Drains {
            diameter_mm: 3.0,
            auto: true,
            ..Drains::default()
        };
        let mut earlier = cut_in_half();
        earlier.ask_drains(Some(auto.clone()));
        let mut recipe = earlier.clone();
        recipe.ask_drains(Some(Drains {
            at_mm: vec![[30.0, 80.0, 8.0]],
            objects: vec![2],
            ..auto
        }));

        let edits = asked_of_pieces(&recipe, &earlier, &HALVES, (Some(0.0), Some(AREA)))
            .expect("a hole added is an edit");

        assert_eq!(objects_of(&edits), [ObjectId(2)], "{edits:?}");
    }

    #[test]
    fn a_cut_moved_is_staged_anew() {
        let earlier = cut_in_half();
        let mut recipe = earlier.clone();
        recipe.modelling = Some(Modelling {
            cut: serde_json::from_value(serde_json::json!({"axis": "x", "at_mm": 90.0})).ok(),
            arrange: true,
            ..Modelling::default()
        });

        let edits = asked_of_pieces(&recipe, &earlier, &HALVES, (Some(0.0), Some(AREA)));

        assert!(edits.is_none(), "other pieces: {edits:?}");
    }

    #[test]
    fn a_plate_of_one_model_is_left_to_its_own_edits() {
        let earlier = Recipe::bare(
            PathBuf::from("/models/bust.stl"),
            String::new(),
            String::new(),
        );
        let recipe = Recipe {
            hollow: Some(Hollow {
                wall_mm: 2.0,
                ..Hollow::default()
            }),
            ..earlier.clone()
        };

        let edits = asked_of_pieces(&recipe, &earlier, &[HALVES[0]], (Some(0.0), Some(AREA)));

        assert!(edits.is_none(), "no pieces: {edits:?}");
    }
}
