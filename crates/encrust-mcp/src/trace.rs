//! `--trace FILE`: a JSON line per tool call — the tool, its milliseconds, whether it failed,
//! and the job and revision it answered with — for a bench to count what a model asked.

use std::path::PathBuf;
use std::time::Duration;

use rmcp::model::{CallToolResponse, ContentBlock};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;

/// Where the lines go, if anywhere.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    file: Option<PathBuf>,
}

impl Trace {
    pub fn new(file: Option<PathBuf>) -> Self {
        Self { file }
    }

    /// Appends the line of one call. A trace that cannot be written is said on stderr; the
    /// call it describes has already been answered.
    pub async fn record(
        &self,
        tool: &str,
        took: Duration,
        response: &Result<CallToolResponse, rmcp::ErrorData>,
    ) {
        let Some(path) = &self.file else {
            return;
        };
        let mut line = line_of(tool, took, response).to_string();
        line.push('\n');
        let written = async {
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await?;
            file.write_all(line.as_bytes()).await
        };
        if let Err(error) = written.await {
            eprintln!(
                "encrust-mcp: the trace {} is not written: {error}",
                path.display()
            );
        }
    }
}

fn line_of(
    tool: &str,
    took: Duration,
    response: &Result<CallToolResponse, rmcp::ErrorData>,
) -> Value {
    let result = match response {
        Ok(CallToolResponse::Complete(result)) => Some(result),
        _ => None,
    };
    let body = result
        .and_then(|result| {
            result.content.iter().find_map(|block| match block {
                ContentBlock::Text(text) => serde_json::from_str::<Value>(&text.text).ok(),
                _ => None,
            })
        })
        .unwrap_or_default();
    json!({
        "tool": tool,
        "ms": u64::try_from(took.as_millis()).unwrap_or(u64::MAX),
        "failed": result.is_none_or(|result| result.is_error == Some(true)),
        "job": body.get("job"),
        "revision": body.get("revision"),
    })
}

#[cfg(test)]
mod tests {
    use rmcp::model::CallToolResult;

    use super::*;

    #[test]
    fn a_call_is_traced_with_the_job_and_revision_it_answered_with() {
        let answered = CallToolResult::success(vec![ContentBlock::text(
            json!({ "job": "cube-1", "revision": 2, "report": {} }).to_string(),
        )]);
        let line = line_of(
            "set_pose",
            Duration::from_millis(1500),
            &Ok(CallToolResponse::Complete(answered)),
        );
        assert_eq!(
            line,
            json!({ "tool": "set_pose", "ms": 1500, "failed": false, "job": "cube-1", "revision": 2 })
        );
    }

    #[test]
    fn a_refused_call_is_traced_as_failed_with_no_job() {
        let refused = CallToolResult::error(vec![ContentBlock::text(
            json!({ "error": "there is no job x" }).to_string(),
        )]);
        let line = line_of(
            "check",
            Duration::ZERO,
            &Ok(CallToolResponse::Complete(refused)),
        );
        assert_eq!(line["failed"], true);
        assert_eq!(line["job"], Value::Null);
    }
}
