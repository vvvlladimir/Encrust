use serde::Deserialize;

use crate::error::PrusaLinkError;
use crate::link::Link;
use crate::session::Session;

/// What `GET /api/v1/job` says of the print under way.
#[derive(Debug, Clone, Deserialize)]
pub struct Printing {
    /// What pausing, resuming and stopping it are addressed by.
    pub id: u64,
    #[serde(default)]
    pub file: Option<PrintedFile>,
}

/// The file being printed, and where its pictures are.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PrintedFile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub refs: Refs,
}

/// Paths on the machine, each read with the same login as the API.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Refs {
    #[serde(default)]
    pub thumbnail: Option<String>,
}

/// The print under way, or `None` while the machine prints nothing.
pub fn job(link: &Link) -> Result<Option<Printing>, PrusaLinkError> {
    let body = Session::new(link).get("api/v1/job")?;
    // No print is a 204 with nothing in it.
    if body.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&body)?))
}

/// Pauses the print `id`.
pub fn pause(link: &Link, id: u64) -> Result<(), PrusaLinkError> {
    Session::new(link)
        .bare("PUT", &format!("api/v1/job/{id}/pause"))
        .map(|_| ())
}

/// Carries on with the paused print `id`.
pub fn resume(link: &Link, id: u64) -> Result<(), PrusaLinkError> {
    Session::new(link)
        .bare("PUT", &format!("api/v1/job/{id}/resume"))
        .map(|_| ())
}

/// Ends the print `id`; it cannot be resumed after.
pub fn stop(link: &Link, id: u64) -> Result<(), PrusaLinkError> {
    Session::new(link)
        .bare("DELETE", &format!("api/v1/job/{id}"))
        .map(|_| ())
}

/// Reads a picture the machine named in `refs`, as the bytes it serves.
pub fn picture(link: &Link, path: &str) -> Result<Vec<u8>, PrusaLinkError> {
    Session::new(link).fetch(path.trim_start_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_names_its_id_and_where_its_thumbnail_is() {
        let printing: Printing = serde_json::from_str(
            r#"{"id":297,"state":"PRINTING","progress":42.0,"time_remaining":520,
                "file":{"name":"DRAGON~1.SL1","display_name":"dragon.sl1","path":"/usb",
                "refs":{"icon":"/thumb/s/usb/DRAGON~1.SL1",
                "thumbnail":"/thumb/l/usb/DRAGON~1.SL1","download":"/usb/DRAGON~1.SL1"}}}"#,
        )
        .expect("the specification's example job");
        assert_eq!(printing.id, 297);
        let file = printing.file.expect("a job names its file");
        assert_eq!(file.display_name.as_deref(), Some("dragon.sl1"));
        assert_eq!(
            file.refs.thumbnail.as_deref(),
            Some("/thumb/l/usb/DRAGON~1.SL1")
        );
    }

    #[test]
    fn a_machine_that_is_not_there_fails_every_job_call() {
        let link = Link::api_key("127.0.0.1:1", "key");
        assert!(job(&link).is_err());
        assert!(pause(&link, 1).is_err());
        assert!(resume(&link, 1).is_err());
        assert!(stop(&link, 1).is_err());
        assert!(picture(&link, "/thumb/l/usb/A.SL1").is_err());
    }
}
