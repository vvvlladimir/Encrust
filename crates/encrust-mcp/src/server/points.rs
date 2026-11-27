//! Points a client gave with no `objects`, handed to the object each lies at on the plate the
//! latest revision's check stood (`recipe::split_by_object`).

use anyhow::Result;
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
    for step in steps {
        ask(recipe, Some(step));
    }
    Ok(handed.then_some(HANDED))
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
}
