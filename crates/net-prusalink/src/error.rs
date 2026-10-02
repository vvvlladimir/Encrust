/// Why a Prusa machine could not be reached, loaded or started.
#[derive(Debug, thiserror::Error)]
pub enum PrusaLinkError {
    #[error("network call failed")]
    Io(#[from] std::io::Error),

    #[error("the printer sent something this API version does not describe")]
    Malformed(#[from] serde_json::Error),

    #[error("the request failed")]
    Transport(#[from] Box<ureq::Error>),

    #[error("that is not a host a URL can be built from")]
    BadHost(#[from] ureq::http::Error),

    #[error("the printer asked for {scheme} authentication, which this client cannot answer")]
    NotDigest { scheme: String },

    #[error("the printer's authentication challenge carries no {field}")]
    ChallengeIncomplete { field: &'static str },

    #[error("{host} rejected the credentials")]
    Unauthorized { host: String },

    #[error("{host} answers HTTP but is not a Prusa machine: it calls itself {text}")]
    NotPrusaLink { host: String, text: String },

    #[error(
        "{text} does not take uploads by PUT; PrusaLink 0.7 or SL1 firmware 1.8.0 is what this needs"
    )]
    NoPutUpload { text: String },

    #[error("the printer refused the request with HTTP {status}: {reason}")]
    Refused { status: u16, reason: String },

    #[error("cancelled")]
    Cancelled,
}

impl From<ureq::Error> for PrusaLinkError {
    fn from(error: ureq::Error) -> Self {
        Self::Transport(Box::new(error))
    }
}

/// What a refusal means, in the words of the API's own error table.
///
/// The body carries a `title` and a `text` when the printer has one; these stand in when
/// it answers a bare status code.
pub(crate) fn refusal(status: u16, body: &str) -> PrusaLinkError {
    PrusaLinkError::Refused {
        status,
        reason: stated(body).unwrap_or_else(|| meaning(status).to_owned()),
    }
}

/// The `Error` object the API documents, flattened into one line.
fn stated(body: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Reported {
        title: Option<String>,
        text: Option<String>,
    }
    let reported: Reported = serde_json::from_str(body).ok()?;
    match (reported.title, reported.text) {
        (Some(title), Some(text)) => Some(format!("{title}: {text}")),
        (Some(one), None) | (None, Some(one)) => Some(one),
        (None, None) => None,
    }
}

fn meaning(status: u16) -> &'static str {
    match status {
        403 => "the credentials are not allowed to do this",
        404 => "the storage or the file is not there",
        409 => "it is printing, or a file of that name is already there",
        415 => "it does not take files of this type",
        507 => "there is no room left on the storage",
        _ => "it gave no reason",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_prefers_what_the_printer_said() {
        let body = r#"{"code":"10108","title":"RESIN TOO LOW","text":"Refill the tank."}"#;
        let PrusaLinkError::Refused { status, reason } = refusal(409, body) else {
            panic!("a refusal keeps its status");
        };
        assert_eq!(status, 409);
        assert_eq!(reason, "RESIN TOO LOW: Refill the tank.");
    }

    #[test]
    fn a_bare_status_is_spelled_out_instead() {
        let PrusaLinkError::Refused { reason, .. } = refusal(409, "") else {
            panic!("a refusal keeps its status");
        };
        assert!(reason.contains("already there"), "got {reason}");
    }

    #[test]
    fn an_unlisted_status_still_reports_the_number() {
        let PrusaLinkError::Refused { status, reason } = refusal(418, "not json") else {
            panic!("a refusal keeps its status");
        };
        assert_eq!(status, 418);
        assert_eq!(reason, "it gave no reason");
    }
}
