use std::fmt::Write as _;

use md5::{Digest as _, Md5};

use crate::error::PrusaLinkError;

/// What a printer's `WWW-Authenticate` header asked for.
///
/// Only the fields a reply needs are kept. `qop` is the one place `PrusaLink` and the oldest
/// form of the scheme differ: without it a reply has no client nonce and no count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Challenge {
    pub realm: String,
    pub nonce: String,
    pub qop: bool,
    pub opaque: Option<String>,
}

impl Challenge {
    /// Reads a `WWW-Authenticate: Digest ...` header.
    ///
    /// The parameters are comma-separated and each value may or may not be quoted, which is
    /// what makes this worth a function rather than a split.
    pub(crate) fn parse(header: &str) -> Result<Self, PrusaLinkError> {
        let rest =
            header
                .trim()
                .strip_prefix("Digest")
                .ok_or_else(|| PrusaLinkError::NotDigest {
                    scheme: header
                        .split_whitespace()
                        .next()
                        .unwrap_or("none")
                        .to_owned(),
                })?;

        let mut realm = None;
        let mut nonce = None;
        let mut qop = None;
        let mut opaque = None;
        for parameter in split_parameters(rest) {
            let Some((name, value)) = parameter.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"').to_owned();
            match name.trim().to_ascii_lowercase().as_str() {
                "realm" => realm = Some(value),
                "nonce" => nonce = Some(value),
                "qop" => qop = Some(value),
                "opaque" => opaque = Some(value),
                _ => {}
            }
        }

        Ok(Self {
            realm: realm.ok_or(PrusaLinkError::ChallengeIncomplete { field: "realm" })?,
            nonce: nonce.ok_or(PrusaLinkError::ChallengeIncomplete { field: "nonce" })?,
            // A server may offer several; `auth` is the only one this client does.
            qop: qop.is_some_and(|offered| {
                offered
                    .split(',')
                    .any(|one| one.trim().eq_ignore_ascii_case("auth"))
            }),
            opaque,
        })
    }

    /// The `Authorization` header that answers this challenge for one request.
    ///
    /// `cnonce` is the client's own nonce, and `count` numbers the requests made under
    /// this nonce: a server is entitled to refuse a count it has already seen. Both are
    /// parameters rather than generated here so a test can pin the header against a
    /// worked example.
    pub(crate) fn answer(
        &self,
        user: &str,
        password: &str,
        method: &str,
        uri: &str,
        cnonce: &str,
        count: u32,
    ) -> String {
        let ha1 = md5_hex(&format!("{user}:{}:{password}", self.realm));
        let ha2 = md5_hex(&format!("{method}:{uri}"));
        let count = format!("{count:08x}");

        let response = if self.qop {
            md5_hex(&format!("{ha1}:{}:{count}:{cnonce}:auth:{ha2}", self.nonce))
        } else {
            md5_hex(&format!("{ha1}:{}:{ha2}", self.nonce))
        };

        let mut header = format!(
            "Digest username=\"{user}\", realm=\"{}\", nonce=\"{}\", uri=\"{uri}\", response=\"{response}\"",
            self.realm, self.nonce
        );
        if self.qop {
            let _ = write!(header, ", qop=auth, nc={count}, cnonce=\"{cnonce}\"");
        }
        if let Some(opaque) = &self.opaque {
            let _ = write!(header, ", opaque=\"{opaque}\"");
        }
        header
    }
}

/// Splits a challenge's parameters, keeping a comma inside quotes out of it.
fn split_parameters(rest: &str) -> Vec<String> {
    let mut parameters = Vec::new();
    let mut current = String::new();
    let mut quoted = false;

    for character in rest.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                current.push(character);
            }
            ',' if !quoted => {
                parameters.push(std::mem::take(&mut current));
            }
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        parameters.push(current);
    }
    parameters
}

fn md5_hex(input: &str) -> String {
    let digest = Md5::digest(input.as_bytes());
    digest.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_challenge_is_read_whether_its_values_are_quoted_or_not() {
        let challenge = Challenge::parse(
            r#"Digest realm="Printer API", nonce=abc123, qop="auth", stale=false"#,
        )
        .expect("a well formed challenge");
        assert_eq!(challenge.realm, "Printer API");
        assert_eq!(challenge.nonce, "abc123");
        assert!(challenge.qop);
        assert_eq!(challenge.opaque, None);
    }

    #[test]
    fn a_comma_inside_a_quoted_value_does_not_split_it() {
        let challenge = Challenge::parse(r#"Digest realm="one, two", nonce="n""#)
            .expect("a well formed challenge");
        assert_eq!(challenge.realm, "one, two");
    }

    #[test]
    fn a_scheme_that_is_not_digest_is_named_rather_than_attempted() {
        let err = Challenge::parse("Basic realm=\"x\"").unwrap_err();
        assert!(matches!(err, PrusaLinkError::NotDigest { scheme } if scheme == "Basic"));
    }

    #[test]
    fn a_challenge_without_a_nonce_is_refused() {
        let err = Challenge::parse(r#"Digest realm="x""#).unwrap_err();
        assert!(matches!(
            err,
            PrusaLinkError::ChallengeIncomplete { field: "nonce" }
        ));
    }

    /// RFC 2617 section 3.5's worked example, so the arithmetic is checked against the
    /// standard rather than against itself. RFC 7616's own example is not usable here:
    /// its MD5 response does not follow from the parameters printed beside it.
    #[test]
    fn the_reply_matches_the_example_in_the_standard() {
        let challenge = Challenge {
            realm: "testrealm@host.com".to_owned(),
            nonce: "dcd98b7102dd2f0e8b11d0f600bfb0c093".to_owned(),
            qop: true,
            opaque: None,
        };
        let header = challenge.answer(
            "Mufasa",
            "Circle Of Life",
            "GET",
            "/dir/index.html",
            "0a4f113b",
            1,
        );
        assert!(
            header.contains("response=\"6629fae49393a05397450978507c4ef1\""),
            "got {header}"
        );
    }

    #[test]
    fn a_server_that_offers_no_qop_gets_the_older_reply() {
        let challenge = Challenge {
            realm: "testrealm@host.com".to_owned(),
            nonce: "dcd98b7102dd2f0e8b11d0f600bfb0c093".to_owned(),
            qop: false,
            opaque: None,
        };
        let header = challenge.answer("Mufasa", "Circle Of Life", "GET", "/dir/index.html", "x", 1);
        assert!(
            !header.contains("qop="),
            "an older server is not sent parameters it did not ask for"
        );
        // RFC 2069's worked example.
        assert!(
            header.contains("response=\"670fd8c2df070c60b045671b8b24ff02\""),
            "got {header}"
        );
    }
}
