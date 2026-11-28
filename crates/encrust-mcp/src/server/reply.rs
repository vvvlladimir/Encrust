//! What a tool answers, built from a run of the command line: a revision and its check, an
//! edit not kept, a report as it came, the pictures it drew.

use anyhow::{Context as _, Result};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value, json};

use crate::jobs::Revision;
use crate::runner::Outcome;

/// What a tool answers: a body, and whether the client should read it as a failure.
pub(super) struct Reply {
    pub(super) body: Value,
    pub(super) failed: bool,
}

/// How many risks an answer lists unless every one is asked: a long list costs the client more
/// than it tells it, and `N.json` keeps the whole.
const RISKS_SHOWN: usize = 12;

pub(super) fn revised(revision: &Revision, outcome: &Outcome, note: Option<&str>) -> Reply {
    revised_with(revision, outcome, note, false)
}

/// As `revised`, with every risk the check found where `every_risk`.
pub(super) fn revised_with(
    revision: &Revision,
    outcome: &Outcome,
    note: Option<&str>,
    every_risk: bool,
) -> Reply {
    let mut report = read_short(&outcome.report);
    if !every_risk {
        keep_first_risks(&mut report, RISKS_SHOWN);
    }
    Reply {
        body: json!({
            "job": revision.job,
            "revision": revision.number,
            "recipe": revision.recipe,
            "note": note,
            "compared": outcome.report.get("compared"),
            "exit_code": outcome.exit_code,
            "report": report,
        }),
        failed: false,
    }
}

/// Cuts `print.cured.risks` to the `most` that matter first — every risk but an island in the
/// check's order, then the largest islands — and says how many were left out.
fn keep_first_risks(report: &mut Value, most: usize) {
    let Some(cured) = report.pointer_mut("/print/cured") else {
        return;
    };
    let Some(risks) = cured.get_mut("risks").and_then(Value::as_array_mut) else {
        return;
    };
    if risks.len() <= most {
        return;
    }
    let is_island = |risk: &Value| risk["kind"] == "island";
    let area = |risk: &Value| risk["area_mm2"].as_f64().unwrap_or(0.0);
    let (mut islands, mut kept): (Vec<Value>, Vec<Value>) = risks.drain(..).partition(is_island);
    islands.sort_by(|a, b| area(b).total_cmp(&area(a)));
    kept.extend(islands);
    let left_out = kept.len().saturating_sub(most);
    kept.truncate(most);
    *risks = kept;
    cured["risks_left_out"] = json!(left_out);
    cured["risks_note"] = json!("the largest first; `check` with `every_risk` lists them all");
}

/// A check as a client reads it: `compared` once, above it, and each model's supports by their
/// count and the contacts left without one; `N.json` keeps the whole.
fn read_short(report: &Value) -> Value {
    let mut report = report.clone();
    if let Some(report) = report.as_object_mut() {
        report.remove("compared");
    }
    for model in report["models"].as_array_mut().into_iter().flatten() {
        if let Some(supports) = model["supports"].as_object_mut() {
            supports.remove("contacts_at_mm");
        }
    }
    report
}

pub(super) fn not_applied(latest: Option<&Revision>, outcome: &Outcome) -> Reply {
    Reply {
        body: json!({
            "applied": false,
            "why_not": "the command line refused the recipe",
            "latest_revision": latest.map(|revision| revision.number),
            "exit_code": outcome.exit_code,
            "report": outcome.report,
        }),
        failed: true,
    }
}

pub(super) fn reported(outcome: &Outcome, body: Value) -> Reply {
    Reply {
        body,
        failed: !outcome.reported(),
    }
}

/// Printers cut to what choosing one takes, and to the query; the catalogue is long.
pub(super) fn trim_profiles(mut listing: Value, query: Option<&str>) -> Value {
    let query = query.map(str::to_lowercase);
    if let Some(printers) = listing.get_mut("printers").and_then(Value::as_array_mut) {
        let kept: Vec<Value> = printers
            .iter()
            .filter(|printer| {
                query.as_deref().is_none_or(|query| {
                    ["id", "manufacturer", "name"].iter().any(|key| {
                        printer[*key].as_str().is_some_and(|text| text.to_lowercase().contains(query))
                    })
                })
            })
            .map(|printer| {
                json!({
                    "id": printer["id"],
                    "name": format!("{} {}", printer["manufacturer"].as_str().unwrap_or_default(), printer["name"].as_str().unwrap_or_default()),
                    "build_volume_mm": printer["build_volume_mm"],
                    "user": printer["user"],
                })
            })
            .collect();
        *printers = kept;
    }
    listing
}

/// The PNGs a `render` run reports, read in as images.
pub(super) async fn images(report: &serde_json::Value) -> Result<Vec<ContentBlock>> {
    let mut content = Vec::new();
    for picture in report["pictures"].as_array().map_or(&[][..], Vec::as_slice) {
        let path = picture["path"]
            .as_str()
            .context("a picture names its file")?;
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("reading {path}"))?;
        content.push(ContentBlock::image(BASE64.encode(bytes), "image/png"));
    }
    Ok(content)
}

pub(super) fn answer(reply: Result<Reply>) -> CallToolResult {
    let (body, failed) = match reply {
        Ok(reply) => (reply.body, reply.failed),
        Err(error) => (json!({ "error": format!("{error:#}") }), true),
    };
    let content = vec![ContentBlock::text(body.to_string())];
    if failed {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_list_of_risks_keeps_every_other_kind_then_the_largest_islands() {
        let island = |area: f64| json!({"kind": "island", "area_mm2": area});
        let mut report = json!({"print": {"cured": {"risks": [
            island(0.1), island(2.0), {"kind": "suction-cup", "area_mm2": 135.0},
            island(0.5), {"kind": "lever"},
        ]}}});

        keep_first_risks(&mut report, 3);

        let cured = &report["print"]["cured"];
        let kinds: Vec<&str> = (cured["risks"].as_array().expect("risks"))
            .iter()
            .map(|risk| risk["kind"].as_str().expect("a kind"))
            .collect();
        assert_eq!(kinds, ["suction-cup", "lever", "island"], "{cured}");
        assert_eq!(
            cured["risks"][2]["area_mm2"], 2.0,
            "the largest island: {cured}"
        );
        assert_eq!(cured["risks_left_out"], 2, "{cured}");
    }

    #[test]
    fn a_short_list_of_risks_is_left_as_it_came() {
        let mut report = json!({"print": {"cured": {"risks": [{"kind": "peel"}]}}});
        let before = report.clone();

        keep_first_risks(&mut report, 3);

        assert_eq!(report, before, "nothing to leave out");
    }

    #[test]
    fn a_check_is_answered_without_each_contact_or_a_second_comparison() {
        let report = json!({
            "compared": {"previous": {"standing": "beats"}},
            "models": [{"supports": {
                "contacts": 2,
                "contacts_at_mm": [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
                "no_room_at_mm": [[1.0, 2.0, 3.0]],
            }}],
        });

        let short = read_short(&report);

        assert!(
            short.get("compared").is_none(),
            "compared stands once, above the report"
        );
        let supports = &short["models"][0]["supports"];
        assert!(
            supports.get("contacts_at_mm").is_none(),
            "every contact is left in N.json"
        );
        assert_eq!(supports["contacts"], 2, "the count stays");
        assert_eq!(
            supports["no_room_at_mm"],
            json!([[1.0, 2.0, 3.0]]),
            "what to act on stays"
        );
    }
}
