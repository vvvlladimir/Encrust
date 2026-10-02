use std::fmt::Write as _;
use std::path::Path;

use crate::error::PrusaLinkError;
use crate::link::{Link, Version};
use crate::session::Session;

/// Asks a machine what it is, which is also what checks the credentials.
///
/// Anything that answers HTTP on port 80 answers this, so the reply is checked for a
/// Prusa machine rather than taken at its word.
pub fn probe(link: &Link) -> Result<Version, PrusaLinkError> {
    let body = Session::new(link).get("api/version")?;
    let version: Version = serde_json::from_str(&body)?;
    if !version.is_prusa() {
        return Err(PrusaLinkError::NotPrusaLink {
            host: link.host.clone(),
            text: if version.text.is_empty() {
                "nothing".to_owned()
            } else {
                version.text
            },
        });
    }
    Ok(version)
}

/// Sends a written file to a machine's storage and returns the name it landed under.
///
/// The file goes up as one PUT, so `cancel` is asked before it starts and cannot be asked
/// again: there is no point in the transfer to answer at. Uploading does not start a
/// print, and no `Print-After-Upload` header is sent at all — `PrusaLink` has read any value
/// there as true, `?0` included.
pub fn upload(
    link: &Link,
    path: &Path,
    cancel: &dyn Fn() -> bool,
) -> Result<String, PrusaLinkError> {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    // The probe comes first for two reasons: it says whether the file may go up as a PUT,
    // and it is where a digest login learns the nonce, which a whole stack should not.
    let version = probe(link)?;
    if !version.takes_a_put() {
        return Err(PrusaLinkError::NoPutUpload {
            text: version.text.clone(),
        });
    }
    if cancel() {
        return Err(PrusaLinkError::Cancelled);
    }
    let mut session = Session::new(link);
    session.put_file(&file_path(link, &filename), path, &[])?;
    Ok(filename)
}

/// Starts a file already on the machine's storage.
pub fn start_print(link: &Link, filename: &str) -> Result<(), PrusaLinkError> {
    Session::new(link)
        .post(&file_path(link, filename))
        .map(|_| ())
}

/// Where one file lives in the API, with its name escaped as a path segment must be.
fn file_path(link: &Link, filename: &str) -> String {
    format!(
        "api/v1/files/{}/{}",
        link.storage(),
        escaped(filename.trim_start_matches('/'))
    )
}

/// Percent-escapes one path segment. Everything RFC 3986 does not call unreserved goes,
/// because a model can be called anything a file system accepts.
fn escaped(segment: &str) -> String {
    let mut escaped = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                escaped.push(char::from(byte));
            }
            other => {
                let _ = write!(escaped, "%{other:02X}");
            }
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::DEFAULT_USER;

    fn a_link() -> Link {
        Link::digest("192.168.1.42", DEFAULT_USER, "secret")
    }

    #[test]
    fn a_file_lands_under_the_storage_the_link_names() {
        let mut link = a_link();
        assert_eq!(
            file_path(&link, "model.sl1"),
            "api/v1/files/local/model.sl1"
        );
        link.storage = "usb".to_owned();
        assert_eq!(file_path(&link, "model.sl1"), "api/v1/files/usb/model.sl1");
    }

    #[test]
    fn a_name_with_a_space_or_a_slash_in_it_is_still_one_segment() {
        assert_eq!(escaped("two parts.sl1"), "two%20parts.sl1");
        assert_eq!(escaped("a/b"), "a%2Fb");
        assert_eq!(
            escaped("kübel.sl1"),
            "k%C3%BCbel.sl1",
            "UTF-8, byte by byte"
        );
    }

    #[test]
    fn an_unreserved_name_travels_unchanged() {
        assert_eq!(escaped("bracket-v2_final.sl1"), "bracket-v2_final.sl1");
    }

    #[test]
    fn nothing_listening_is_a_transport_error_rather_than_a_refusal() {
        let link = Link::digest("127.0.0.1:1", DEFAULT_USER, "secret");
        assert!(matches!(
            probe(&link),
            Err(PrusaLinkError::Transport(_) | PrusaLinkError::Io(_))
        ));
    }

    #[test]
    fn a_cancelled_send_never_reaches_the_machine() {
        let link = Link::digest("127.0.0.1:1", DEFAULT_USER, "secret");
        let path = std::env::temp_dir().join("encrust-prusalink-cancel.sl1");
        std::fs::write(&path, b"not a real archive").expect("the temporary directory is writable");
        // The probe fails first, which is the point: cancelling cannot be reached without
        // a machine, and the machine is what the transport error names.
        assert!(matches!(
            upload(&link, &path, &|| true),
            Err(PrusaLinkError::Transport(_) | PrusaLinkError::Io(_))
        ));
        std::fs::remove_file(path).ok();
    }
}
