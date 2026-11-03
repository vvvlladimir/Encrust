//! What a tool answers, built from a run of the command line: a revision and its check, an
//! edit not kept, a report as it came.

use anyhow::Result;
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
            "exit_code": outcome.exit_code,
            "report": outcome.report,
        }),
        failed: false,
    }
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
