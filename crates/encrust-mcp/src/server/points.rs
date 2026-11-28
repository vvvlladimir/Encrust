//! Points a client gave with no `objects`, handed to the object each lies at on the plate the
//! latest revision's check stood (`recipe::split_by_object`).

use anyhow::{Result, bail};
use serde_json::Value;

use crate::recipe::{ObjectBox, PlatePoints, Recipe, split_by_object};

/// What the client is told when its points went to the objects they lie at.
const HANDED: &str = "each point went to the object it lies at; `objects` names one \
    outright";

/// Each object's box on the plate as `report` lists it.
pub(super) fn boxes_of(report: &Value) -> Vec<ObjectBox> {
    let models = report["models"].as_array().map_or(&[][..], Vec::as_slice);
    (models.iter())
        .filter_map(|model| {
            let id = model["id"].as_u64()?;
            let bounds: [[f32; 3]; 2] =
                serde_json::from_value(model["fit"]["bounds_mm"].clone()).ok()?;
            Some((id, bounds))
        })
        .collect()
}

/// Asks `asked` of `recipe` by `ask`, its points handed to the objects of the plate `report`
/// lists; the note for the client where they were.
pub(super) fn ask_by_object<T: PlatePoints>(
    recipe: &mut Recipe,
    asked: Option<T>,
    report: Option<&Value>,
    ask: fn(&mut Recipe, Option<T>),
) -> Result<Option<&'static str>> {
    let Some(asked) = asked else {
        ask(recipe, None);
        return Ok(None);
    };
    let boxes = report.map(boxes_of).unwrap_or_default();
    let steps = split_by_object(asked, &boxes)?;
    let handed = steps.len() > 1;
    for mut step in steps {
        if recipe.points_in_object {
            into_object(&mut step, report, &boxes)?;
        }
        ask(recipe, Some(step));
    }
    Ok(handed.then_some(HANDED))
}

/// Carries `step`'s points from the plate, as `report`'s check stood it, into the frame of the
/// object they lie on, where they stay whatever moves it later (`--points-in-object`).
fn into_object<T: PlatePoints>(
    step: &mut T,
    report: Option<&Value>,
    boxes: &[ObjectBox],
) -> Result<()> {
    if step.anchors().is_empty() {
        return Ok(());
    }
    let named = step.objects().first().copied();
    let Some(id) = named.or_else(|| boxes.first().map(|(id, _)| *id)) else {
        bail!("the latest check stands no model for the points to lie on");
    };
    let frame = report.and_then(|report| matrix_of(report, id, "from_plate"));
    let Some(from_plate) = frame else {
        bail!("the latest check names no frame for object {id}; check the job again first");
    };
    step.carry_points(&|point| carried_by(&from_plate, point));
    Ok(())
}

/// `recipe` with its finishing points carried back onto the plate as `report`'s check stood each
/// object, for edits of a plate, which take plate points.
pub(super) fn on_plate(recipe: &Recipe, report: &Value) -> Recipe {
    let mut plate = recipe.clone();
    if !recipe.points_in_object {
        return plate;
    }
    let models = report["models"].as_array().map_or(&[][..], Vec::as_slice);
    let alone = (models.len() == 1)
        .then(|| models[0]["id"].as_u64())
        .flatten();
    let carry = (report, alone);
    plate
        .hollow
        .iter_mut()
        .chain(&mut plate.more_hollow)
        .for_each(|step| to_plate(step, carry));
    plate
        .drains
        .iter_mut()
        .chain(&mut plate.more_drains)
        .for_each(|step| to_plate(step, carry));
    let supports = plate.supports.iter_mut().chain(&mut plate.more_supports);
    supports.for_each(|step| to_plate(step, carry));
    plate.points_in_object = false;
    plate
}

/// `step`'s points carried from its object's frame onto the plate `report` stands it on.
fn to_plate<T: PlatePoints>(step: &mut T, (report, alone): (&Value, Option<u64>)) {
    let id = step.objects().first().copied().or(alone);
    if let Some(to_plate) = id.and_then(|id| matrix_of(report, id, "to_plate")) {
        step.carry_points(&|point| carried_by(&to_plate, point));
    }
}

/// Object `id`'s matrix `which` of the two `report` lists for it, in column order.
pub(super) fn matrix_of(report: &Value, id: u64, which: &str) -> Option<[f32; 16]> {
    let models = report["models"].as_array()?;
    let model = models
        .iter()
        .find(|model| model["id"].as_u64() == Some(id))?;
    serde_json::from_value(model["frame"][which].clone()).ok()
}

/// `point` carried by the 4x4 `matrix`, in column order.
pub(super) fn carried_by(matrix: &[f32; 16], [x, y, z]: [f32; 3]) -> [f32; 3] {
    let row =
        |at: usize| matrix[at] * x + matrix[at + 4] * y + matrix[at + 8] * z + matrix[at + 12];
    [row(0), row(1), row(2)]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn each_object_s_box_is_read_from_its_check() {
        let report = json!({"models": [
            {"id": 2, "fit": {"bounds_mm": [[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]]}},
            {"id": 3, "fit": null},
        ]});

        assert_eq!(boxes_of(&report), [(2, [[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]])]);
    }

    /// A check of two halves, the second moved 100 mm along X from its own frame.
    fn halves() -> Value {
        let moved = |dx: f32| -> Vec<f32> {
            vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, dx, 0.0, 0.0, 1.0,
            ]
        };
        json!({"models": [
            {"id": 2, "fit": {"bounds_mm": [[0.0, 0.0, 0.0], [50.0, 50.0, 50.0]]},
             "frame": {"to_plate": moved(0.0), "from_plate": moved(0.0)}},
            {"id": 3, "fit": {"bounds_mm": [[100.0, 0.0, 0.0], [150.0, 50.0, 50.0]]},
             "frame": {"to_plate": moved(100.0), "from_plate": moved(-100.0)}},
        ]})
    }

    #[test]
    fn a_point_is_kept_in_the_frame_of_the_half_it_lies_on_and_carried_back_alike() {
        let report = halves();
        let mut recipe = Recipe::bare("/m.stl".into(), String::new(), String::new());
        let drains = crate::recipe::Drains {
            diameter_mm: 3.0,
            at_mm: vec![[120.0, 10.0, 5.0]],
            ..Default::default()
        };
        ask_by_object(&mut recipe, Some(drains), Some(&report), Recipe::ask_drains)
            .expect("the point lies on the second half");
        let kept = &recipe.more_drains[0];
        assert_eq!(kept.objects, [3]);
        assert_eq!(kept.at_mm, [[20.0, 10.0, 5.0]], "in the half's own frame");
        let plate = on_plate(&recipe, &report);
        assert_eq!(
            plate.more_drains[0].at_mm,
            [[120.0, 10.0, 5.0]],
            "back where it was given"
        );
        assert!(!plate.points_in_object);
    }

    #[test]
    fn a_point_on_a_half_with_no_frame_asks_for_a_check_first() {
        let mut report = halves();
        report["models"][1]["frame"] = Value::Null;
        let mut recipe = Recipe::bare("/m.stl".into(), String::new(), String::new());
        let drains = crate::recipe::Drains {
            diameter_mm: 3.0,
            at_mm: vec![[120.0, 10.0, 5.0]],
            ..Default::default()
        };
        let refused = ask_by_object(&mut recipe, Some(drains), Some(&report), Recipe::ask_drains);
        assert!(refused.is_err());
    }
}
