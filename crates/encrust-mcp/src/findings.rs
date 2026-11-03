//! The located findings of a check's report (ADR 0396), as points to mark on a picture.

use serde::Serialize;
use serde_json::Value;

/// How many findings a picture marks; the rest are counted, not drawn.
pub const MOST_MARKED: usize = 48;

/// One problem the check found, and where on the plate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    /// Its number on the pictures, from one.
    pub mark: usize,
    /// `island`, `suction-cup`, … from the report; `trapped-resin`; `unsupported-contact`.
    pub kind: String,
    pub at_mm: [f64; 3],
}

/// Every located finding of an `estimate` report, the trapped resin first.
pub fn findings(report: &Value) -> Vec<Finding> {
    let mut found = Vec::new();
    for pocket in list(&report["slicing"]["trapped"]) {
        push(&mut found, "trapped-resin", &pocket["at_mm"]);
    }
    let cured = if report["print"]["cured"].is_object() {
        &report["print"]["cured"]
    } else {
        &report["cured"]
    };
    for risk in list(&cured["risks"]) {
        push(
            &mut found,
            risk["kind"].as_str().unwrap_or("risk"),
            &risk["at_mm"],
        );
    }
    for model in list(&report["models"]) {
        for point in list(&model["supports"]["no_room_at_mm"]) {
            push(&mut found, "unsupported-contact", point);
        }
    }
    found
}

fn list(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

fn push(found: &mut Vec<Finding>, kind: &str, at: &Value) {
    let coordinate = |index: usize| at[index].as_f64();
    if let (Some(x), Some(y), Some(z)) = (coordinate(0), coordinate(1), coordinate(2)) {
        found.push(Finding {
            mark: found.len() + 1,
            kind: kind.to_owned(),
            at_mm: [x, y, z],
        });
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_estimate_report_yields_its_pockets_risks_and_bare_contacts_in_that_order() {
        let report = json!({
            "slicing": {"trapped": [{"at_mm": [20.0, 20.0, 7.0], "volume_mm3": 1720.0}]},
            "print": {"cured": {"risks": [{"kind": "island", "layer": 38, "at_mm": [19.9, 12.3, 1.8]}]}},
            "models": [{"supports": {"no_room_at_mm": [[15.0, 15.0, 8.0]]}}],
        });
        let found = findings(&report);
        let kinds: Vec<(usize, &str)> = found
            .iter()
            .map(|finding| (finding.mark, finding.kind.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                (1, "trapped-resin"),
                (2, "island"),
                (3, "unsupported-contact")
            ]
        );
    }

    #[test]
    fn a_clean_report_has_no_finding() {
        assert!(findings(&json!({"slicing": {"trapped": []}, "cured": {"risks": []}})).is_empty());
    }
}
