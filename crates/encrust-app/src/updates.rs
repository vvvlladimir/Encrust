//! The update the window offers: a look at the release feed once a day, off until the user
//! turns it on, and an install that only a click starts. See `docs/design/updates.md`.

mod feed;
mod install;
mod verify;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use web_time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::status::Status;

pub use feed::{Note, Offer, Version};
pub use install::sweep;

/// How long one look at the feed holds before the next is due, seconds: a day.
const CHECK_EVERY_S: u64 = 24 * 60 * 60;

/// Where a release is read about in a browser, for a build this window cannot install.
pub const RELEASES: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases");

/// Why a look at the feed or an install did not happen.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("cannot fetch {url}")]
    Fetch {
        url: String,
        #[source]
        source: Box<ureq::Error>,
    },
    #[error("the update feed is not readable")]
    Feed(#[source] serde_json::Error),
    #[error("`{0}` is not a release version")]
    Version(String),
    #[error("the feed points outside this project's releases: {0}")]
    Foreign(String),
    #[error("the download is not signed with the Encrust release key")]
    Signature(#[source] minisign_verify::Error),
    #[error("the signature is for `{found}`, not `{expected}`")]
    Signed { expected: String, found: String },
    #[error("the downloaded archive cannot be unpacked")]
    Archive(#[source] std::io::Error),
    #[error("the downloaded archive holds no `{0}`")]
    Missing(String),
    #[error("cannot replace {}", path.display())]
    Replace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// What the window remembers of updates between runs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePrefs {
    /// Whether the window looks at the feed by itself. Off until the user turns it on.
    #[serde(default)]
    pub check: bool,
    /// When the feed was last read, seconds since the Unix epoch.
    #[serde(default)]
    pub checked_at_s: Option<u64>,
    /// A version the user asked not to be shown in the title strip again.
    #[serde(default)]
    pub skipped: Option<String>,
}

/// Where the update is up to.
#[derive(Default)]
pub enum Stage {
    #[default]
    Idle,
    Checking {
        asked: bool,
        reply: Receiver<Result<Option<Offer>, UpdateError>>,
    },
    /// The feed was read and names nothing newer than this build.
    Current,
    Offered(Offer),
    Installing {
        offer: Offer,
        reply: Receiver<Result<(), UpdateError>>,
    },
    /// The new binary is in place and runs from the next start.
    Installed(Offer),
    Failed {
        offer: Option<Offer>,
        message: String,
    },
}

/// The update state of the window.
#[derive(Default)]
pub struct Updates {
    pub prefs: UpdatePrefs,
    pub stage: Stage,
    /// The binary being replaced, read before it was.
    exe: Option<PathBuf>,
    /// The binary to start once the window has closed, set by "Restart".
    restart: Option<PathBuf>,
    /// Whether the window has been asked to close for a restart and not yet told to.
    close: bool,
}

impl Updates {
    /// The version of this build.
    pub fn running() -> Version {
        Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_default()
    }

    /// Looks at the feed once a day, if the user turned that on. A failure here is quiet:
    /// the user did not ask for this look, and it is tried again tomorrow.
    pub fn tick(&mut self) {
        let idle = matches!(
            self.stage,
            Stage::Idle | Stage::Current | Stage::Failed { .. }
        );
        if self.prefs.check && idle && is_due(self.prefs.checked_at_s, now_s()) {
            self.start_check(false);
        }
    }

    /// Looks at the feed now, because the user asked.
    pub fn check_now(&mut self) {
        if !self.is_busy() {
            self.start_check(true);
        }
    }

    fn start_check(&mut self, asked: bool) {
        self.prefs.checked_at_s = Some(now_s());
        let (sender, reply) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(install::check(Self::running()));
        });
        self.stage = Stage::Checking { asked, reply };
    }

    /// Downloads the offered build, checks its signature and puts it in place of this one.
    /// A failed install is tried again from the same offer.
    pub fn install(&mut self) {
        let (Stage::Offered(offer)
        | Stage::Failed {
            offer: Some(offer), ..
        }) = &self.stage
        else {
            return;
        };
        let (Some(download), Ok(exe)) = (offer.download.clone(), install::current_exe()) else {
            return;
        };
        let offer = offer.clone();
        // Read before the swap: on Linux the path of a replaced binary reads "(deleted)".
        self.exe = Some(exe.clone());
        let version = offer.version;
        let (sender, reply) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(install::install(&download, version, &exe));
        });
        self.stage = Stage::Installing { offer, reply };
    }

    /// Stops the title strip offering this version; the Updates page still shows it.
    pub fn skip(&mut self) {
        if let Stage::Offered(offer) = &self.stage {
            self.prefs.skipped = Some(offer.version.to_string());
        }
    }

    /// The version the title strip offers, if one is waiting and was not skipped.
    pub fn waiting(&self) -> Option<&Offer> {
        match &self.stage {
            Stage::Offered(offer) | Stage::Installed(offer)
                if self.prefs.skipped != Some(offer.version.to_string()) =>
            {
                Some(offer)
            }
            _ => None,
        }
    }

    /// Why `offer` cannot be installed from the window, when it cannot; the release page
    /// is the way then.
    pub fn blocked(offer: &Offer) -> Option<&'static str> {
        if offer.download.is_none() {
            Some("This release has no build for this platform.")
        } else {
            install::current_exe().err()
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.stage,
            Stage::Checking { .. } | Stage::Installing { .. }
        )
    }

    /// Asks for the window to close and the installed build to start after it.
    pub fn restart(&mut self) {
        if let Stage::Installed(_) = self.stage {
            self.restart.clone_from(&self.exe);
            self.close = self.restart.is_some();
        }
    }

    /// The user kept the window open after all, so nothing starts when it later closes.
    pub fn cancel_restart(&mut self) {
        self.restart = None;
    }

    /// Whether the window should be told to close this frame, once.
    pub fn take_close(&mut self) -> bool {
        std::mem::take(&mut self.close)
    }

    /// Starts the installed build, called as the window goes away.
    pub fn on_exit(&mut self) {
        if let Some(exe) = self.restart.take()
            && let Err(error) = std::process::Command::new(&exe).spawn()
        {
            tracing::error!(exe = %exe.display(), %error, "cannot start the updated build");
        }
    }

    /// Takes what a worker thread finished. Whether one is still running is the answer.
    pub fn poll(&mut self, status: &mut Status) -> bool {
        match std::mem::take(&mut self.stage) {
            Stage::Checking { asked, reply } => match reply.try_recv() {
                Ok(outcome) => self.checked(asked, outcome, status),
                Err(TryRecvError::Empty) => self.stage = Stage::Checking { asked, reply },
                Err(TryRecvError::Disconnected) => self.stage = Stage::Idle,
            },
            Stage::Installing { offer, reply } => match reply.try_recv() {
                Ok(outcome) => self.installed(offer, outcome, status),
                Err(TryRecvError::Empty) => self.stage = Stage::Installing { offer, reply },
                Err(TryRecvError::Disconnected) => self.stage = Stage::Offered(offer),
            },
            stage => self.stage = stage,
        }
        self.is_busy()
    }

    fn checked(
        &mut self,
        asked: bool,
        outcome: Result<Option<Offer>, UpdateError>,
        status: &mut Status,
    ) {
        self.stage = match outcome {
            Ok(Some(offer)) => Stage::Offered(offer),
            Ok(None) => Stage::Current,
            Err(error) => {
                let error = anyhow::Error::new(error).context("cannot check for updates");
                tracing::info!("{error:#}");
                if asked {
                    *status = Status::failed(&error);
                }
                Stage::Failed {
                    offer: None,
                    message: format!("{error:#}"),
                }
            }
        };
    }

    fn installed(&mut self, offer: Offer, outcome: Result<(), UpdateError>, status: &mut Status) {
        match outcome {
            Ok(()) => {
                *status = Status::Info(format!("Encrust {} is installed", offer.version));
                self.stage = Stage::Installed(offer);
                self.restart();
            }
            Err(error) => {
                let error = anyhow::Error::new(error)
                    .context(format!("cannot install Encrust {}", offer.version));
                *status = Status::failed(&error);
                self.stage = Stage::Failed {
                    offer: Some(offer),
                    message: format!("{error:#}"),
                };
            }
        }
    }
}

/// Whether a look made at `checked_at_s` is a day old at `now_s`. A clock set back counts
/// as due, so a wrong date cannot silence the check for good.
fn is_due(checked_at_s: Option<u64>, now_s: u64) -> bool {
    checked_at_s.is_none_or(|at| now_s < at || now_s - at >= CHECK_EVERY_S)
}

/// Seconds since the Unix epoch, now.
pub fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_that_never_looked_is_due() {
        assert!(is_due(None, 1_000));
    }

    #[test]
    fn one_look_holds_for_a_day() {
        let at = 1_000_000;
        assert!(!is_due(Some(at), at + CHECK_EVERY_S - 1));
        assert!(is_due(Some(at), at + CHECK_EVERY_S));
    }

    #[test]
    fn a_clock_set_back_does_not_silence_the_check() {
        assert!(is_due(Some(1_000_000), 10));
    }

    #[test]
    fn the_window_looks_at_nothing_until_the_user_turns_the_check_on() {
        let mut updates = Updates::default();
        updates.tick();
        assert!(matches!(updates.stage, Stage::Idle));
        assert_eq!(updates.prefs.checked_at_s, None);
    }

    #[test]
    fn this_build_knows_its_own_version() {
        assert_ne!(Updates::running(), Version::default());
    }

    #[test]
    fn a_skipped_version_leaves_the_title_strip() {
        let mut updates = Updates {
            stage: Stage::Offered(feed::tests::an_offer("9.0.0")),
            ..Updates::default()
        };
        assert!(updates.waiting().is_some());
        updates.skip();
        assert!(updates.waiting().is_none());
        assert!(
            matches!(updates.stage, Stage::Offered(_)),
            "the page still offers it"
        );
    }

    #[test]
    fn a_restart_the_user_backed_out_of_starts_nothing() {
        let mut updates = Updates {
            stage: Stage::Installed(feed::tests::an_offer("9.0.0")),
            exe: Some(PathBuf::from("encrust-gui")),
            ..Updates::default()
        };
        updates.restart();
        assert!(updates.take_close(), "the window is asked to close once");
        assert!(!updates.take_close());
        updates.cancel_restart();
        assert!(updates.restart.is_none());
    }
}
