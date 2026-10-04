//! The one JSON document `--json` prints to stdout, with the schema version it follows.

use anyhow::Result;
use serde::Serialize;

/// Raised whenever a field is removed or changes meaning; adding one does not raise it.
pub const SCHEMA: u32 = 1;

#[derive(Serialize)]
struct Document<'a, T> {
    schema: u32,
    #[serde(flatten)]
    body: &'a T,
}

/// Prints `body` as the run's one document, the schema version beside its own fields.
pub fn print<T: Serialize>(body: &T) -> Result<()> {
    println!("{}", document(body)?);
    Ok(())
}

fn document<T: Serialize>(body: &T) -> Result<String> {
    Ok(serde_json::to_string_pretty(&Document {
        schema: SCHEMA,
        body,
    })?)
}

#[derive(Serialize)]
struct Failure {
    error: Cause,
}

#[derive(Serialize)]
struct Cause {
    message: String,
    /// Every cause under the message, outermost first.
    chain: Vec<String>,
    cancelled: bool,
}

/// The document a failed run prints in place of its report.
pub fn error(error: &anyhow::Error) -> String {
    let failure = Failure {
        error: Cause {
            message: error.to_string(),
            chain: error.chain().skip(1).map(ToString::to_string).collect(),
            cancelled: crate::exit::is_cancelled(error),
        },
    };
    document(&failure).unwrap_or_else(|_| format!("{{\"schema\":{SCHEMA},\"error\":{{}}}}"))
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use super::*;

    #[test]
    fn an_error_document_carries_the_schema_and_every_cause() {
        let failed = Err::<(), _>(anyhow::anyhow!("no such file"))
            .context("cannot load cube.stl")
            .expect_err("the error was just made");
        let parsed: serde_json::Value =
            serde_json::from_str(&error(&failed)).expect("the document is JSON");
        assert_eq!(parsed["schema"], SCHEMA);
        assert_eq!(parsed["error"]["message"], "cannot load cube.stl");
        assert_eq!(parsed["error"]["chain"][0], "no such file");
        assert_eq!(parsed["error"]["cancelled"], false);
    }
}
