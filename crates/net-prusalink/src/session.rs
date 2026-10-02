use std::fs::File;
use std::hash::{BuildHasher as _, RandomState};
use std::path::Path;
use std::time::Duration;

use ureq::http::{Request, Response, request::Builder};

use crate::digest::Challenge;
use crate::error::{PrusaLinkError, refusal};
use crate::link::{Auth, Link};

/// A printer on the local network answers a small request at once or not at all; the PUT
/// carries a whole stack, so it is given its own, longer window.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);
const UPLOAD_TIMEOUT: Duration = Duration::from_mins(20);

/// One conversation with one machine, holding the challenge it answered with.
///
/// Digest authentication costs a round trip to learn the nonce, so it is learnt on the
/// first refusal and spent on every request after. Each of those counts up from the last,
/// because a server is entitled to refuse a count it has already seen.
pub(crate) struct Session<'a> {
    link: &'a Link,
    calls: ureq::Agent,
    uploads: ureq::Agent,
    challenge: Option<Challenge>,
    cnonce: String,
    count: u32,
}

impl<'a> Session<'a> {
    pub(crate) fn new(link: &'a Link) -> Self {
        Self {
            link,
            calls: agent(CALL_TIMEOUT),
            uploads: agent(UPLOAD_TIMEOUT),
            challenge: None,
            cnonce: cnonce(),
            count: 0,
        }
    }

    /// Reads one JSON endpoint.
    pub(crate) fn get(&mut self, path: &str) -> Result<String, PrusaLinkError> {
        self.twice("GET", path, |session, builder| {
            let request = builder.header("Accept", JSON).body(())?;
            Ok(session.calls.run(request)?)
        })
    }

    /// Posts to a path with no body, which is how a print is started.
    pub(crate) fn post(&mut self, path: &str) -> Result<String, PrusaLinkError> {
        self.twice("POST", path, |session, builder| {
            let request = builder
                .header("Accept", JSON)
                .header("Content-Length", "0")
                .body(())?;
            Ok(session.calls.run(request)?)
        })
    }

    /// Sends the file itself as the body of one PUT.
    ///
    /// The file is the body rather than a reader over it because the API asks for a
    /// `Content-Length` and `ureq` takes that from the body it is given; a reader would go
    /// up chunked, which is what ADR 0152 is about.
    pub(crate) fn put_file(
        &mut self,
        path: &str,
        source: &Path,
        headers: &[(&str, &str)],
    ) -> Result<String, PrusaLinkError> {
        self.twice("PUT", path, |session, builder| {
            let mut builder = builder
                .header("Content-Type", OCTET_STREAM)
                .header("Overwrite", TRUE);
            for (name, value) in headers {
                builder = builder.header(*name, *value);
            }
            let request = builder.body(File::open(source)?)?;
            Ok(session.uploads.run(request)?)
        })
    }

    /// Sends a request, and sends it again once when the answer was a challenge.
    ///
    /// The first request of a digest login carries no `Authorization` at all: the nonce to
    /// answer with only exists in the refusal it comes back with.
    fn twice(
        &mut self,
        method: &str,
        path: &str,
        mut send: impl FnMut(&mut Self, Builder) -> Result<Response<ureq::Body>, PrusaLinkError>,
    ) -> Result<String, PrusaLinkError> {
        let builder = self.start(method, path);
        let response = send(self, builder)?;
        if response.status().as_u16() != 401 {
            return self.read(response);
        }
        self.learn(&response)?;
        let builder = self.start(method, path);
        let response = send(self, builder)?;
        self.read(response)
    }

    /// A request addressed at one path, carrying whatever lets us in.
    fn start(&mut self, method: &str, path: &str) -> Builder {
        let mut builder = Request::builder().method(method).uri(self.link.url(path));
        match &self.link.auth {
            Auth::ApiKey(key) => builder = builder.header("X-Api-Key", key),
            Auth::Digest { user, password } => {
                if let Some(challenge) = self.challenge.clone() {
                    self.count += 1;
                    let answer = challenge.answer(
                        user,
                        password,
                        method,
                        &format!("/{path}"),
                        &self.cnonce,
                        self.count,
                    );
                    builder = builder.header("Authorization", answer);
                }
            }
        }
        builder
    }

    /// Turns one answer into its body, or into the refusal it states.
    fn read(&self, response: Response<ureq::Body>) -> Result<String, PrusaLinkError> {
        let status = response.status().as_u16();
        let body = response.into_body().read_to_string().unwrap_or_default();
        match status {
            200..=299 => Ok(body),
            401 => Err(PrusaLinkError::Unauthorized {
                host: self.link.host.clone(),
            }),
            _ => Err(refusal(status, &body)),
        }
    }

    /// Files the challenge a refusal carried, so the next request can answer it.
    fn learn(&mut self, response: &Response<ureq::Body>) -> Result<(), PrusaLinkError> {
        let refused = || PrusaLinkError::Unauthorized {
            host: self.link.host.clone(),
        };
        // A key is not a challenge: a machine refusing one has nothing more to offer.
        if matches!(self.link.auth, Auth::ApiKey(_)) {
            return Err(refused());
        }
        let header = response
            .headers()
            .get("WWW-Authenticate")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(refused)?;
        self.challenge = Some(Challenge::parse(header)?);
        Ok(())
    }
}

/// A refusal is an answer here: it carries the challenge and the reason.
fn agent(timeout: Duration) -> ureq::Agent {
    ureq::config::Config::builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// The client's own nonce, which only has to be unrepeatable between two sessions.
fn cnonce() -> String {
    format!("{:016x}", RandomState::new().hash_one(0u8))
}

const JSON: &str = "application/json";
const OCTET_STREAM: &str = "application/octet-stream";

/// A boolean in a header, as RFC 8941 spells one.
pub(crate) const TRUE: &str = "?1";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::DEFAULT_USER;

    fn challenged(link: &Link, qop: bool) -> Session<'_> {
        let mut session = Session::new(link);
        session.challenge = Some(Challenge {
            realm: "Printer API".to_owned(),
            nonce: "abc".to_owned(),
            qop,
            opaque: None,
        });
        session
    }

    fn header_of(builder: &Builder, name: &str) -> String {
        builder
            .headers_ref()
            .and_then(|headers| headers.get(name))
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    }

    #[test]
    fn a_digest_login_sends_nothing_until_it_has_been_challenged() {
        let link = Link::digest("printer", DEFAULT_USER, "secret");
        let builder = Session::new(&link).start("GET", "api/version");
        assert!(
            header_of(&builder, "Authorization").is_empty(),
            "there is no nonce to answer with"
        );
    }

    #[test]
    fn two_requests_under_one_challenge_count_up() {
        let link = Link::digest("printer", DEFAULT_USER, "secret");
        let mut session = challenged(&link, true);
        let first = header_of(&session.start("GET", "api/version"), "Authorization");
        let second = header_of(
            &session.start("PUT", "api/v1/files/local/model.sl1"),
            "Authorization",
        );
        assert!(first.contains("nc=00000001"), "got {first}");
        assert!(second.contains("nc=00000002"), "got {second}");
    }

    #[test]
    fn the_uri_in_the_answer_is_the_path_the_request_is_made_to() {
        let link = Link::digest("printer", DEFAULT_USER, "secret");
        let header = header_of(
            &challenged(&link, false).start("PUT", "api/v1/files/local/model.sl1"),
            "Authorization",
        );
        assert!(
            header.contains(r#"uri="/api/v1/files/local/model.sl1""#),
            "got {header}"
        );
    }

    #[test]
    fn a_key_travels_in_its_own_header_on_every_request() {
        let link = Link::api_key("printer", "0123456789abcdef");
        let builder = Session::new(&link).start("GET", "api/version");
        assert_eq!(header_of(&builder, "X-Api-Key"), "0123456789abcdef");
    }
}
