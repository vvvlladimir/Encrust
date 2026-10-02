use serde::{Deserialize, Serialize};

/// A Prusa machine on the local network, and what it takes to be let in.
///
/// There is no discovery here: `PrusaLink` announces itself over mDNS, which is a resolver
/// this workspace does not carry, so a machine is named rather than found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    /// Host as the user typed it: an address or a name, with or without a scheme.
    pub host: String,
    pub auth: Auth,
    /// Which storage the file lands on, as the API spells it: `local`, `usb`, `sdcard`.
    pub storage: String,
}

impl Link {
    /// A machine reached at `host` with a digest login, on its own storage.
    pub fn digest(host: &str, user: &str, password: &str) -> Self {
        Self {
            host: host.trim().to_owned(),
            auth: Auth::Digest {
                user: user.to_owned(),
                password: password.to_owned(),
            },
            storage: LOCAL_STORAGE.to_owned(),
        }
    }

    /// A machine reached at `host` with the single key it shows when digest is off.
    pub fn api_key(host: &str, key: &str) -> Self {
        Self {
            host: host.trim().to_owned(),
            auth: Auth::ApiKey(key.to_owned()),
            storage: LOCAL_STORAGE.to_owned(),
        }
    }

    /// The URL one API path is reached at. A host typed without a scheme gets `http://`,
    /// because a printer on the local network serves plain HTTP.
    pub(crate) fn url(&self, path: &str) -> String {
        let host = self.host.trim().trim_end_matches('/');
        let base = if host.starts_with("http://") || host.starts_with("https://") {
            host.to_owned()
        } else {
            format!("http://{host}")
        };
        format!("{base}/{path}")
    }

    /// The storage segment of a file path, without the slashes around it.
    pub(crate) fn storage(&self) -> &str {
        match self.storage.trim_matches('/') {
            "" => LOCAL_STORAGE,
            named => named,
        }
    }
}

/// The printer's own storage, and the one `PrusaLink` defaults to.
const LOCAL_STORAGE: &str = "local";

/// How a machine is asked to let us in. Digest is what the firmware generates; the key is
/// what it falls back to when its digest login has been turned off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Auth {
    ApiKey(String),
    Digest { user: String, password: String },
}

/// The username the SL1 and every bundled `PrusaLink` generate their password under.
pub const DEFAULT_USER: &str = "maker";

/// What `GET /api/version` said: which API the machine speaks, and whether it takes a PUT.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Version {
    /// API version, `1.0.0` on everything that answers at all.
    #[serde(default)]
    pub api: String,
    /// What the machine calls itself, `PrusaLink 0.7.0` or `Prusa SLA 1.8.0`.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub firmware: String,
    #[serde(default)]
    capabilities: Capabilities,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Capabilities {
    #[serde(default, rename = "upload-by-put")]
    upload_by_put: bool,
}

impl Version {
    /// Whether the file goes up as a PUT to the v1 API. An absent capability means no,
    /// which is what the specification says of every capability it does not list.
    pub fn takes_a_put(&self) -> bool {
        self.capabilities.upload_by_put
    }

    /// Whether this is a machine we know how to send to rather than something else
    /// answering on port 80: the three names a `PrusaLink` board answers with.
    pub fn is_prusa(&self) -> bool {
        ["PrusaLink", "Prusa SLA", "OctoPrint"]
            .iter()
            .any(|known| self.text.starts_with(known))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_without_a_scheme_is_reached_over_plain_http() {
        let link = Link::digest("192.168.1.42", DEFAULT_USER, "secret");
        assert_eq!(link.url("api/version"), "http://192.168.1.42/api/version");
    }

    #[test]
    fn a_host_that_carries_its_own_scheme_keeps_it() {
        let link = Link::api_key("https://sl1.local/", "key");
        assert_eq!(link.url("api/version"), "https://sl1.local/api/version");
    }

    #[test]
    fn a_storage_written_with_slashes_is_still_one_segment() {
        let mut link = Link::api_key("printer", "key");
        link.storage = "/usb/".to_owned();
        assert_eq!(link.storage(), "usb");
        link.storage = String::new();
        assert_eq!(link.storage(), "local");
    }

    #[test]
    fn a_version_without_capabilities_does_not_take_a_put() {
        let version: Version = serde_json::from_str(
            r#"{"api":"1.0.0","version":"0.6.0","text":"PrusaLink 0.6.0","firmware":"1.7.0"}"#,
        )
        .expect("the older machines answer without a capability object");
        assert!(!version.takes_a_put());
        assert!(version.is_prusa());
    }

    #[test]
    fn the_sl1_names_itself_and_takes_a_put() {
        let version: Version = serde_json::from_str(
            r#"{"api":"1.0.0","text":"Prusa SLA 1.8.0","firmware":"1.8.0",
                "capabilities":{"upload-by-put":true}}"#,
        )
        .expect("the reply is well formed");
        assert!(version.is_prusa());
        assert!(version.takes_a_put());
    }

    #[test]
    fn something_else_serving_http_is_not_a_prusa_machine() {
        let version = Version {
            text: "nginx".to_owned(),
            ..Version::default()
        };
        assert!(!version.is_prusa());
    }
}
