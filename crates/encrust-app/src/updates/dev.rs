//! Switches a development build reads to show an update with no release out: a fake one, or
//! a feed from another address. See `docs/design/updates.md`.

use std::thread::sleep;
use std::time::Duration;

use super::feed::{self, Offer, Version};
use super::progress::{Progress, Step};
use super::{UpdateError, Updates, now_s};

/// A file of release notes; the window offers a fake 9.9.9 carrying them.
const FAKE: &str = "ENCRUST_FAKE_UPDATE";
/// An address read in place of the release feed.
const FEED: &str = "ENCRUST_UPDATE_FEED";

/// The archive size a fake offer states and its rehearsed download counts, bytes.
const FAKE_SIZE: u64 = 39_845_888;
/// How long before now a fake release went out, seconds: three days.
const FAKE_AGE_S: u64 = 3 * 24 * 60 * 60;
/// Steps of the rehearsed download, and how long each takes: ten seconds in all.
const TICKS: u64 = 125;
const TICK: Duration = Duration::from_millis(80);
const STEP: Duration = Duration::from_secs(1);

fn var(name: &str) -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// Whether either switch is set, so the window looks once on start whatever the day.
pub fn active() -> bool {
    var(FAKE).is_some() || var(FEED).is_some()
}

/// Whether the offer is fake, so its install is rehearsed rather than refused.
pub fn faked() -> bool {
    var(FAKE).is_some()
}

/// The address to read in place of the release feed.
pub fn feed_url() -> Option<String> {
    var(FEED)
}

/// The fake offer, when it is asked for. Notes that cannot be read leave it without notes.
pub fn fake_offer() -> Option<Result<Option<Offer>, UpdateError>> {
    let path = var(FAKE)?;
    let notes = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        tracing::warn!(path, %error, "the fake update's notes cannot be read");
        String::new()
    });
    Some(fake(&notes, Updates::running(), now_s()))
}

/// A 9.9.9 with `notes`, read through the feed's own reader, published three days before `now_s`.
fn fake(notes: &str, running: Version, now_s: u64) -> Result<Option<Offer>, UpdateError> {
    let platform = feed::platform().unwrap_or("macos-aarch64");
    let url = concat!(
        env!("CARGO_PKG_REPOSITORY"),
        "/releases/download/v9.9.9/encrust-fake.tar.gz"
    );
    let text = serde_json::json!({
        "version": "9.9.9",
        "notes": notes,
        "platforms": { platform: { "url": url, "signature": "", "size": FAKE_SIZE } },
    });
    let offer = feed::read(text.to_string().as_bytes(), running, Some(platform))?;
    Ok(offer.map(|offer| Offer {
        published_s: now_s.checked_sub(FAKE_AGE_S),
        ..offer
    }))
}

/// Plays an install through its steps over ten seconds, touching neither network nor disk.
pub fn rehearse(progress: &Progress) -> Result<(), UpdateError> {
    progress.enter(Step::Download);
    progress.expect(FAKE_SIZE);
    for _ in 0..TICKS {
        if progress.is_cancelled() {
            return Err(UpdateError::Cancelled);
        }
        sleep(TICK);
        progress.add(FAKE_SIZE / TICKS);
    }
    progress.enter(Step::Verify);
    sleep(STEP);
    progress.enter(Step::Install);
    sleep(STEP);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fake_update_offers_its_notes_with_a_date_and_a_size() {
        let offer = fake("### Slicing\n\n* faster", Version::default(), 1_000_000)
            .expect("the fake feed reads")
            .expect("9.9.9 is newer than 0.0.0");
        assert_eq!(offer.version.to_string(), "9.9.9");
        assert_eq!(offer.published_s, Some(1_000_000 - FAKE_AGE_S));
        let download = offer.download.expect("the fake names an archive");
        assert_eq!(download.size, Some(FAKE_SIZE));
        assert!(
            !offer.notes.is_empty(),
            "the notes are read as a feed's are"
        );
    }

    #[test]
    fn a_cancelled_rehearsal_stops_before_the_download() {
        let progress = Progress::default();
        progress.cancel();
        assert!(matches!(rehearse(&progress), Err(UpdateError::Cancelled)));
        assert_eq!(progress.bytes().0, 0);
    }
}
