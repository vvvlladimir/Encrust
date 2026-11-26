//! A line `encrust-mcp` sends: a revision to show by its plate (schema 4), or under schema 3
//! the plate asked for, or edits to apply on the version it was built on
//! (`docs/design/window-link.md`).

use agent_link::{Header, PLATE_SCHEMA, PlateLine, SHOW_SCHEMA, Show};
use core_edit::Edit;
use serde::Deserialize;
use serde_json::Value;

/// One line read, its token checked.
#[derive(Debug)]
pub enum Line {
    /// A revision, its plate there to read.
    Revision(Box<Show>),
    /// The plate as it stands, written for `job` to build its next edits on.
    Plate { job: String },
    /// `edits` built on the plate at `base_version`, to apply if it is still that version.
    Apply {
        job: String,
        edits: Vec<Edit>,
        base_version: u64,
    },
}

/// `text` as a line from a server holding `token`, or why it is refused.
pub fn read(text: &str, token: &str) -> Result<Line, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| format!("the line does not read: {error}"))?;
    let header =
        Header::deserialize(&value).map_err(|error| format!("the line does not read: {error}"))?;
    if header.token != token {
        return Err("the token is not this window's".into());
    }
    match header.schema {
        SHOW_SCHEMA => {
            let show = Show::deserialize(value)
                .map_err(|error| format!("the revision does not read: {error}"))?;
            if !show.plate.is_file() {
                return Err(format!(
                    "the revision's plate {} is not there",
                    show.plate.display()
                ));
            }
            Ok(Line::Revision(Box::new(show)))
        }
        PLATE_SCHEMA => plate_line(value),
        found => Err(format!(
            "schema {found} is neither {SHOW_SCHEMA} nor {PLATE_SCHEMA}"
        )),
    }
}

fn plate_line(value: Value) -> Result<Line, String> {
    let line = PlateLine::deserialize(value)
        .map_err(|error| format!("the line does not read: {error}"))?;
    match (line.want.as_deref(), line.apply, line.base_version) {
        (Some("plate"), None, _) => Ok(Line::Plate { job: line.job }),
        (None, Some(edits), Some(base_version)) => Ok(Line::Apply {
            job: line.job,
            edits,
            base_version,
        }),
        (None, Some(_), None) => Err("edits come with the base_version they were built on".into()),
        _ => Err("a schema 3 line wants the plate or applies edits".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "abc";

    #[test]
    fn a_line_asking_for_the_plate_names_its_job() {
        let line = read(
            r#"{"schema": 3, "token": "abc", "job": "j", "want": "plate"}"#,
            TOKEN,
        );
        assert!(matches!(line, Ok(Line::Plate { job }) if job == "j"));
    }

    #[test]
    fn edits_read_with_the_version_they_were_built_on() {
        let text = r#"{"schema": 3, "token": "abc", "job": "j", "base_version": 7,
            "apply": [{"edit": "cut", "object": 2, "axis": "z", "at_mm": 4.0}]}"#;
        let Ok(Line::Apply {
            edits,
            base_version,
            ..
        }) = read(text, TOKEN)
        else {
            panic!("the edits read");
        };
        assert_eq!((edits.len(), base_version), (1, 7));
    }

    #[test]
    fn edits_without_a_base_version_are_refused() {
        let text = r#"{"schema": 3, "token": "abc", "job": "j", "apply": []}"#;
        assert!(read(text, TOKEN).is_err_and(|why| why.contains("base_version")));
    }

    #[test]
    fn a_line_with_another_token_is_refused() {
        let text = r#"{"schema": 3, "token": "xyz", "job": "j", "want": "plate"}"#;
        assert!(read(text, TOKEN).is_err_and(|why| why.contains("token")));
    }
}
