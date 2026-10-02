//! `latest.json`, the feed the release workflow hangs on each release, and what it offers
//! this build. Its shape is in `docs/design/updates.md`.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use super::UpdateError;

/// The feed of the newest published release. A draft has none, so nothing unreviewed is
/// ever offered.
pub const FEED_URL: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/releases/latest/download/latest.json"
);

/// Where every archive the feed may name is downloaded from.
const DOWNLOADS: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/download/");

/// A release version, `major.minor.patch`, the only shape release-please tags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    /// Reads `1.2.3`, or the tag `v1.2.3`.
    pub fn parse(text: &str) -> Result<Self, UpdateError> {
        let invalid = || UpdateError::Version(text.to_owned());
        let mut parts = text.strip_prefix('v').unwrap_or(text).split('.');
        let mut next = || -> Result<u64, UpdateError> {
            let part = parts.next().ok_or_else(invalid)?;
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid());
            }
            part.parse().map_err(|_| invalid())
        };
        let version = Self {
            major: next()?,
            minor: next()?,
            patch: next()?,
        };
        match parts.next() {
            None => Ok(version),
            Some(_) => Err(invalid()),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Deserialize)]
struct Feed {
    version: String,
    #[serde(default)]
    notes: String,
    /// One archive per platform, keyed by [`platform`].
    #[serde(default)]
    platforms: BTreeMap<String, Build>,
}

#[derive(Deserialize)]
struct Build {
    url: String,
    /// The archive's detached minisign signature, the whole `.minisig` file.
    signature: String,
}

/// A newer release than this build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub version: Version,
    pub notes: Vec<Note>,
    /// The archive for this platform, when the release has one.
    pub download: Option<Download>,
}

/// One archive of a release and the signature it has to carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub url: String,
    /// The archive's file name, which its signature names too.
    pub archive: String,
    pub signature: String,
}

/// One line of the release notes, as the Updates page sets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    Heading(String),
    Item(String),
    Text(String),
}

/// What a feed offers a build at `running` on `platform`: nothing when it is no newer.
pub fn read(
    text: &[u8],
    running: Version,
    platform: Option<&str>,
) -> Result<Option<Offer>, UpdateError> {
    let feed: Feed = serde_json::from_slice(text).map_err(UpdateError::Feed)?;
    let version = Version::parse(&feed.version)?;
    if version <= running {
        return Ok(None);
    }
    let download = platform
        .and_then(|platform| feed.platforms.get(platform))
        .map(|build| download(build, version))
        .transpose()?;
    Ok(Some(Offer {
        version,
        notes: notes_of(&feed.notes, version),
        download,
    }))
}

/// Only an archive of this very release, on this project's release page, is downloaded.
/// The signature would refuse anything else; this refuses it before a byte is fetched.
fn download(build: &Build, version: Version) -> Result<Download, UpdateError> {
    let tag = format!("v{version}/");
    let archive = build
        .url
        .strip_prefix(DOWNLOADS)
        .and_then(|rest| rest.strip_prefix(&tag))
        .filter(|name| !name.is_empty() && !name.contains(['/', '?', '#']))
        .ok_or_else(|| UpdateError::Foreign(build.url.clone()))?;
    Ok(Download {
        url: build.url.clone(),
        archive: archive.to_owned(),
        signature: build.signature.clone(),
    })
}

/// The key this build's archive has in the feed, the name `release.yml` gives it.
pub fn platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("macos-aarch64"),
        ("macos", "x86_64") => Some("macos-x86_64"),
        ("linux", "x86_64") => Some("linux-x86_64"),
        ("windows", "x86_64") => Some("windows-x86_64"),
        _ => None,
    }
}

/// The notes of `version`, without the heading naming it: the page titles the offer already.
fn notes_of(markdown: &str, version: Version) -> Vec<Note> {
    let named = version.to_string();
    let mut notes = notes(markdown);
    notes.retain(|note| !matches!(note, Note::Heading(text) if text.starts_with(&named)));
    notes
}

/// The release body release-please wrote, as lines of plain text.
pub fn notes(markdown: &str) -> Vec<Note> {
    markdown
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            if let Some(heading) = line.strip_prefix('#') {
                Note::Heading(plain(heading))
            } else if let Some(item) = line.strip_prefix("* ").or_else(|| line.strip_prefix("- ")) {
                Note::Item(plain(item))
            } else {
                Note::Text(plain(line))
            }
        })
        .filter(|note| match note {
            Note::Heading(text) | Note::Item(text) | Note::Text(text) => !text.is_empty(),
        })
        .collect()
}

/// A line of markdown without its markup: a link keeps its text, a commit hash is dropped,
/// and so is a leading emoji the window's fonts may not carry.
fn plain(line: &str) -> String {
    let mut text = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        text.push_str(&rest[..open]);
        let inner = &rest[open + 1..];
        let link = inner
            .find("](")
            .and_then(|close| Some((close, close + 2 + inner[close + 2..].find(')')?)));
        match link {
            Some((close, end)) => {
                text.push_str(&inner[..close]);
                rest = &inner[end + 1..];
            }
            None => {
                text.push('[');
                rest = inner;
            }
        }
    }
    text.push_str(rest);
    let text = without_commits(&text.replace("**", "").replace('`', ""));
    text.trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim()
        .to_owned()
}

/// Drops each `(4037d25)` a changelog line ends on: it means something on GitHub only.
fn without_commits(text: &str) -> String {
    let mut kept = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('(') {
        let close = rest[open..].find(')').map(|close| open + close);
        let Some(close) = close else { break };
        let inner = &rest[open + 1..close];
        let is_commit = inner.len() >= 7 && inner.bytes().all(|byte| byte.is_ascii_hexdigit());
        if is_commit {
            kept.push_str(rest[..open].trim_end());
        } else {
            kept.push_str(&rest[..=close]);
        }
        rest = &rest[close + 1..];
    }
    kept.push_str(rest);
    kept
}

#[cfg(test)]
pub mod tests {
    use super::*;

    const REPO: &str = env!("CARGO_PKG_REPOSITORY");

    /// An offer of `version` with an archive for this platform, as a feed would make it.
    pub fn an_offer(version: &str) -> Offer {
        let text = feed(
            version,
            &format!("{REPO}/releases/download/v{version}/a.tar.gz"),
        );
        read(text.as_bytes(), Version::default(), Some("macos-aarch64"))
            .expect("the feed reads")
            .expect("and offers something newer than 0.0.0")
    }

    fn feed(version: &str, url: &str) -> String {
        serde_json::json!({
            "version": version,
            "notes": "### ✨ New\n\n* **format:** write `.cws` ([4037d25](https://x/commit/4037d25))",
            "platforms": { "macos-aarch64": { "url": url, "signature": "sig" } },
        })
        .to_string()
    }

    fn version(text: &str) -> Version {
        Version::parse(text).expect("a release version")
    }

    #[test]
    fn versions_compare_as_numbers_not_as_text() {
        assert!(version("0.10.0") > version("0.9.3"));
        assert!(version("v1.0.0") > version("0.99.99"));
        assert_eq!(version("v0.2.1"), version("0.2.1"));
    }

    #[test]
    fn a_tag_that_is_not_three_numbers_is_refused() {
        for text in ["", "1.2", "1.2.3.4", "1.2.x", "1.2.3-rc.1", "+1.2.3", "v"] {
            assert!(
                matches!(Version::parse(text), Err(UpdateError::Version(_))),
                "{text:?} is not a release version"
            );
        }
    }

    #[test]
    fn a_newer_release_is_offered_with_this_platforms_archive() {
        let text = feed(
            "0.3.0",
            &format!("{REPO}/releases/download/v0.3.0/a.tar.gz"),
        );
        let offer = read(text.as_bytes(), version("0.1.0"), Some("macos-aarch64"))
            .expect("the feed reads")
            .expect("0.3.0 is newer than 0.1.0");
        assert_eq!(offer.version, version("0.3.0"));
        assert_eq!(offer.notes.first(), Some(&Note::Heading("New".to_owned())));
        let download = offer.download.expect("the platform has an archive");
        assert_eq!(download.archive, "a.tar.gz");
        assert_eq!(download.signature, "sig");
    }

    #[test]
    fn the_running_version_or_an_older_one_is_not_offered() {
        let text = feed(
            "0.3.0",
            &format!("{REPO}/releases/download/v0.3.0/a.tar.gz"),
        );
        for running in ["0.3.0", "0.4.0"] {
            let offer = read(text.as_bytes(), version(running), Some("macos-aarch64"));
            assert!(matches!(offer, Ok(None)), "nothing is offered to {running}");
        }
    }

    /// A platform the release has no archive for still hears about it, to fetch by hand.
    #[test]
    fn a_platform_without_an_archive_is_told_about_the_release() {
        let text = feed(
            "0.3.0",
            &format!("{REPO}/releases/download/v0.3.0/a.tar.gz"),
        );
        let offer = read(text.as_bytes(), version("0.1.0"), Some("linux-riscv64"))
            .expect("the feed reads")
            .expect("and offers 0.3.0");
        assert_eq!(offer.download, None);
    }

    #[test]
    fn an_archive_from_anywhere_else_is_refused() {
        let elsewhere = [
            "https://example.com/v0.3.0/a.tar.gz".to_owned(),
            format!("{REPO}/releases/download/v0.2.0/a.tar.gz"),
            format!("{REPO}/releases/download/v0.3.0/../a.tar.gz"),
            format!("{REPO}/releases/download/v0.3.0/"),
        ];
        for url in elsewhere {
            let text = feed("0.3.0", &url);
            let read = read(text.as_bytes(), version("0.1.0"), Some("macos-aarch64"));
            assert!(
                matches!(read, Err(UpdateError::Foreign(_))),
                "{url} is not this release's archive"
            );
        }
    }

    #[test]
    fn a_feed_that_is_not_json_is_refused() {
        let read = read(b"<html>", version("0.1.0"), None);
        assert!(matches!(read, Err(UpdateError::Feed(_))));
    }

    #[test]
    fn a_feed_naming_no_version_is_refused() {
        let read = read(br#"{"version":"latest"}"#, version("0.1.0"), None);
        assert!(matches!(read, Err(UpdateError::Version(_))));
    }

    #[test]
    fn this_platform_has_an_archive_in_the_release() {
        let built = cfg!(any(
            all(
                target_os = "macos",
                any(target_arch = "aarch64", target_arch = "x86_64")
            ),
            all(
                any(target_os = "linux", target_os = "windows"),
                target_arch = "x86_64"
            ),
        ));
        assert_eq!(platform().is_some(), built);
    }

    #[test]
    fn release_notes_lose_their_markup_and_keep_their_words() {
        let body = "## [0.3.0](https://x/compare/v0.2.0...v0.3.0) (2026-10-02)\n\n\
                    ### ✨ New\n\n\
                    * **format:** write `.cws` ([4037d25](https://x/commit/4037d25))\n\
                    * keep a note (see #12)\n";
        assert_eq!(
            notes(body),
            vec![
                Note::Heading("0.3.0 (2026-10-02)".to_owned()),
                Note::Heading("New".to_owned()),
                Note::Item("format: write .cws".to_owned()),
                Note::Item("keep a note (see #12)".to_owned()),
            ]
        );
    }
}
