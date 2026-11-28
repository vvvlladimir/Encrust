//! The window link's lines: the window's file, a revision to show (schema 4), the plate asked for
//! or edits to apply (schema 3), and the window's answers (`docs/design/window-link.md`).

use std::path::PathBuf;

use core_edit::{Edit, ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The schema of a revision handed over as its plate (ADR 0462).
pub const SHOW_SCHEMA: u64 = 4;
/// The schema that asks for the plate and applies edits to it (ADR 0456).
pub const PLATE_SCHEMA: u64 = 3;

/// What the open window writes to say where it listens and the token a line must carry; its
/// schema is [`SHOW_SCHEMA`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowFile {
    pub schema: u64,
    pub port: u16,
    pub token: String,
    pub pid: u32,
    /// The window's version, which the server holds against its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// What every line starts with: its schema and the window's token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub schema: u64,
    pub token: String,
}

/// A line as the server sends it: its header, then its body's fields.
#[derive(Debug, Serialize)]
pub struct Signed<'a, T> {
    #[serde(flatten)]
    pub header: Header,
    #[serde(flatten)]
    pub body: &'a T,
}

/// One revision of a chat client's job to show: its model, its machine and the plate its
/// check wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Show {
    pub job: String,
    pub revision: u32,
    pub model: PathBuf,
    /// A printer id of the catalogue, or the path of a printer TOML; empty for the standard one.
    pub printer: String,
    /// A resin id, or the path of a resin TOML; empty for the stock numbers.
    pub resin: String,
    /// The `.encrust` its check wrote, which the window puts on its plate.
    pub plate: PathBuf,
    /// Shown whether the window follows its agents or not: the client asked for it, or the
    /// user did (ADR 0462).
    #[serde(default)]
    pub open: bool,
}

/// A schema 3 line: `want: "plate"` asks for the job's plate, `apply` hands edits built on
/// the plate at `base_version`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlateLine {
    pub job: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub want: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply: Option<Vec<Edit>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_version: Option<u64>,
}

impl PlateLine {
    /// Asks for `job`'s plate as it stands.
    pub fn plate_of(job: &str) -> Self {
        Self {
            job: job.to_owned(),
            want: Some("plate".into()),
            apply: None,
            base_version: None,
        }
    }

    /// Hands `edits` built on the plate at `base_version`.
    pub fn apply(job: &str, edits: &[Edit], base_version: u64) -> Self {
        Self {
            job: job.to_owned(),
            want: None,
            apply: Some(edits.to_vec()),
            base_version: Some(base_version),
        }
    }
}

/// What the window says of a revision to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowReply {
    pub shown: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why_not: Option<String>,
}

impl ShowReply {
    /// The revision is on its plate.
    pub fn taken() -> Self {
        Self {
            shown: true,
            why_not: None,
        }
    }

    /// The revision is not shown, for `why`.
    pub fn refused(why: impl Into<String>) -> Self {
        Self {
            shown: false,
            why_not: Some(why.into()),
        }
    }
}

/// The job's plate as the window wrote it for the server to build on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandedPlate {
    /// The `.encrust` file it wrote.
    #[serde(rename = "plate")]
    pub path: PathBuf,
    /// Raised by every recorded change of the plate, its undo and its redo.
    pub version: u64,
    /// Every object on the plate.
    pub objects: Vec<ObjectId>,
    /// The job's model where it stands whole on the plate.
    pub job_object: Option<ObjectId>,
}

/// What the window answers where it hands no plate.
pub fn plate_refused(why: impl Into<String>) -> Value {
    json!({"plate": null, "why_not": why.into()})
}

/// What the window says of edits handed over: all landed, none applied as the plate moved on to
/// `version`, or none left applied for `why_not` (ADR 0462).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReply {
    pub applied: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stale: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why_not: Option<String>,
}

impl ApplyReply {
    /// Every edit landed.
    pub fn landed() -> Self {
        Self {
            applied: true,
            stale: false,
            version: None,
            why_not: None,
        }
    }

    /// None is left applied, for `why`.
    pub fn refused(why: impl Into<String>) -> Self {
        Self {
            applied: false,
            stale: false,
            version: None,
            why_not: Some(why.into()),
        }
    }

    /// None was applied: the plate moved on to `version`.
    pub fn stale(version: u64) -> Self {
        Self {
            applied: false,
            stale: true,
            version: Some(version),
            why_not: None,
        }
    }

    /// Whether the window refused the edits for a reason of its own, the plate as before.
    pub fn refused_for_cause(&self) -> bool {
        !self.applied && !self.stale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signed_line_carries_its_header_beside_the_body_fields() {
        let body = PlateLine::plate_of("cube-1");
        let header = Header {
            schema: PLATE_SCHEMA,
            token: "abc".into(),
        };
        let line = serde_json::to_value(Signed {
            header,
            body: &body,
        })
        .expect("a line serialises");
        assert_eq!(
            line,
            json!({"schema": 3, "token": "abc", "job": "cube-1", "want": "plate"})
        );
    }

    #[test]
    fn a_stale_reply_reads_back_with_the_version_the_plate_moved_on_to() {
        let text = serde_json::to_string(&ApplyReply::stale(13)).expect("serialises");
        let read: ApplyReply = serde_json::from_str(&text).expect("reads");
        assert_eq!((read.stale, read.version), (true, Some(13)));
        assert!(!read.refused_for_cause());
    }
}
