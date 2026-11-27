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

pub(super) fn revised(revision: &Revision, outcome: &Outcome, note: Option<&str>) -> Reply {
    Reply {
        body: json!({
            "job": revision.job,
            "revision": revision.number,
            "recipe": revision.recipe,
            "note": note,
            "compared": outcome.report.get("compared"),
            "exit_code": outcome.exit_code,
            "report": read_short(&outcome.report),
        }),
        failed: false,
    }
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

/// Edits the open window did not take, so the revision built of them is not kept: the window's
/// plate and the latest revision stay as they were (ADR 0462).
pub(super) fn not_taken(latest: &Revision, window: &Value) -> Reply {
    Reply {
        body: json!({
            "applied": false,
            "why_not": "the open window did not take the edits, so nothing was kept",
            "latest_revision": latest.number,
            "window": window,
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
