//! The update the window offers: a look at the release feed once a day, on until the user
//! turns it off, and an install that only a click starts. See `docs/design/updates.md`.

mod dev;
mod feed;
mod install;
mod progress;
#[cfg(windows)]
mod store;
mod verify;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use web_time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::i18n::tr;
use crate::status::Status;

pub use feed::{Note, Offer, Version};
pub use install::{from_store, sweep};
pub use progress::{Progress, Step};

/// How long one look at the feed holds before the next is due, seconds: a day.
const CHECK_EVERY_S: u64 = 24 * 60 * 60;

/// Where a release is read about in a browser, for a build this window cannot install.
pub const RELEASES: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases");

/// The app's page in the Microsoft Store, which updates the copy installed from it.
pub const STORE_PAGE: &str = "https://apps.microsoft.com/detail/9NB0ZMB3M8H8";
/// The same page opened in the Store app rather than a browser.
#[cfg(windows)]
const STORE_LINK: &str = "ms-windows-store://pdp/?ProductId=9NB0ZMB3M8H8";

/// Why a look at the feed or an install did not happen.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("{}", tr!("update-error-fetch", url = .url.clone()))]
    Fetch {
        url: String,
        #[source]
        source: Box<ureq::Error>,
    },
    #[error("{}", tr!("update-error-feed"))]
    Feed(#[source] serde_json::Error),
    #[error("{}", tr!("update-error-version", version = .0.clone()))]
    Version(String),
    #[error("{}", tr!("update-error-foreign", url = .0.clone()))]
    Foreign(String),
    #[error("{}", tr!("update-error-signature"))]
    Signature(#[source] minisign_verify::Error),
    #[error("{}", tr!(
        "update-error-signed",
        found = .found.clone(),
        expected = .expected.clone()
    ))]
    Signed { expected: String, found: String },
    #[error("{}", tr!("update-error-archive"))]
    Archive(#[source] std::io::Error),
    #[error("{}", tr!("update-error-missing", name = .0.clone()))]
    Missing(String),
    #[error("{}", tr!("update-error-replace", path = .path.display().to_string()))]
    Replace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The user stopped the download.
    #[error("{}", tr!("update-error-cancelled"))]
    Cancelled,
    #[cfg(windows)]
    #[error("{}", tr!("update-error-store"))]
    Store(#[source] windows::core::Error),
}

/// What the window remembers of updates between runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePrefs {
    /// Whether the window looks at the feed by itself. On until the user turns it off.
    #[serde(default = "looking")]
    pub check: bool,
    /// When the feed was last read, seconds since the Unix epoch.
    #[serde(default)]
    pub checked_at_s: Option<u64>,
    /// A version the user asked not to be shown in the top bar again.
    #[serde(default)]
    pub skipped: Option<String>,
    /// The version whose notice at the stage's foot was put by: it says so once a version.
    #[serde(default)]
    pub noticed: Option<String>,
}

impl Default for UpdatePrefs {
    fn default() -> Self {
        Self {
            check: looking(),
            checked_at_s: None,
            skipped: None,
            noticed: None,
        }
    }
}

/// The daily look is on by default, here and in a file written before it was; see ADR 0213.
fn looking() -> bool {
    true
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
        progress: Arc<Progress>,
        started: web_time::Instant,
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
    /// Whether the sheet that carries the update through is over the plate.
    pub sheet: bool,
}

impl Updates {
    /// The version of this build.
    pub fn running() -> Version {
        Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_default()
    }

    /// Looks once a day, if the user turned that on or the Store updates this copy. A failure
    /// here is quiet: the user did not ask, and it is tried again tomorrow.
    pub fn tick(&mut self) {
        let idle = matches!(
            self.stage,
            Stage::Idle | Stage::Current | Stage::Failed { .. }
        );
        let due = is_due(self.prefs.checked_at_s, now_s());
        let first_with_switch = matches!(self.stage, Stage::Idle) && dev::active();
        if first_with_switch || ((self.prefs.check || from_store()) && idle && due) {
            self.start_check(false);
        }
    }

    /// Looks now, because the user asked.
    pub fn check_now(&mut self) {
        if !self.is_busy() {
            self.start_check(true);
        }
    }

    fn start_check(&mut self, asked: bool) {
        // A page loads the latest release every time it is opened, and `ureq` reads a clock
        // wasm does not have: the look panicked and took the window down with it.
        if cfg!(target_arch = "wasm32") {
            return;
        }
        self.prefs.checked_at_s = Some(now_s());
        let (sender, reply) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(look());
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
        let Some(download) = offer.download.clone() else {
            return;
        };
        let exe = match install::current_exe() {
            Ok(exe) => Some(exe),
            Err(_) if dev::faked() => None,
            Err(_) => return,
        };
        let offer = offer.clone();
        // Read before the swap: on Linux the path of a replaced binary reads "(deleted)".
        self.exe.clone_from(&exe);
        let version = offer.version;
        let (sender, reply) = mpsc::channel();
        let progress = Arc::new(Progress::default());
        let counted = Arc::clone(&progress);
        crate::job::spawn(move || {
            let outcome = match &exe {
                Some(exe) => install::install(&download, version, exe, &counted),
                None => dev::rehearse(&counted),
            };
            let _ = sender.send(outcome);
        });
        let started = web_time::Instant::now();
        self.stage = Stage::Installing {
            offer,
            reply,
            progress,
            started,
        };
    }

    /// Stops the download under way; the offer stands again.
    pub fn cancel(&self) {
        if let Stage::Installing { progress, .. } = &self.stage {
            progress.cancel();
        }
    }

    /// How far the install under way has got, and seconds since it started.
    pub fn progress(&self) -> Option<(&Progress, f64)> {
        match &self.stage {
            Stage::Installing {
                progress, started, ..
            } => Some((progress, started.elapsed().as_secs_f64())),
            _ => None,
        }
    }

    /// The release the window holds: offered, being installed, installed or failed on.
    pub fn in_hand(&self) -> Option<&Offer> {
        match &self.stage {
            Stage::Offered(offer)
            | Stage::Installing { offer, .. }
            | Stage::Installed(offer)
            | Stage::Failed {
                offer: Some(offer), ..
            } => Some(offer),
            _ => None,
        }
    }

    /// The release the stage's foot announces: offered, not skipped, its notice not put by.
    pub fn notice(&self) -> Option<&Offer> {
        match &self.stage {
            Stage::Offered(offer) => {
                let version = Some(offer.version.to_string());
                (self.prefs.skipped != version && self.prefs.noticed != version).then_some(offer)
            }
            _ => None,
        }
    }

    /// Puts the notice by for this version; the pill in the top bar stays.
    pub fn put_notice_by(&mut self) {
        if let Stage::Offered(offer) = &self.stage {
            self.prefs.noticed = Some(offer.version.to_string());
        }
    }

    /// Stops the top bar offering this version; the Updates page still shows it.
    pub fn skip(&mut self) {
        if let Stage::Offered(offer) = &self.stage {
            self.prefs.skipped = Some(offer.version.to_string());
        }
    }

    /// The version the top bar offers, if one is waiting and was not skipped.
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
    pub fn blocked(offer: &Offer) -> Option<String> {
        if offer.download.is_none() {
            Some(tr!("update-no-build"))
        } else if dev::faked() {
            None
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
            Stage::Installing {
                offer,
                reply,
                progress,
                started,
            } => match reply.try_recv() {
                Ok(outcome) => self.installed(offer, outcome, status),
                Err(TryRecvError::Empty) => {
                    self.stage = Stage::Installing {
                        offer,
                        reply,
                        progress,
                        started,
                    };
                }
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
                let error = anyhow::Error::new(error).context(tr!("update-cannot-check"));
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
                *status =
                    Status::Info(tr!("update-installed", version = offer.version.to_string()));
                self.stage = Stage::Installed(offer);
                self.restart();
            }
            Err(UpdateError::Cancelled) => self.stage = Stage::Offered(offer),
            Err(error) => {
                let error = anyhow::Error::new(error).context(tr!(
                    "update-cannot-install",
                    version = offer.version.to_string()
                ));
                *status = Status::failed(&error);
                self.stage = Stage::Failed {
                    offer: Some(offer),
                    message: format!("{error:#}"),
                };
            }
        }
    }
}

/// What is newer than this build: the Store's package for a copy installed from it, the
/// release feed's for any other.
fn look() -> Result<Option<Offer>, UpdateError> {
    #[cfg(windows)]
    if from_store() {
        return store::check();
    }
    if let Some(fake) = dev::fake_offer() {
        return fake;
    }
    install::check(Updates::running())
}

/// Opens this app's page in the Store app, whose Update button installs what it offered.
pub fn open_store() {
    #[cfg(windows)]
    crate::job::spawn(|| {
        if let Err(error) = store::open() {
            let error = anyhow::Error::new(error).context(tr!("update-cannot-open-store"));
            tracing::error!("{error:#}");
        }
    });
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
    fn a_window_nobody_has_set_up_looks_by_itself() {
        assert!(UpdatePrefs::default().check);
    }

    #[test]
    fn the_window_looks_at_nothing_once_the_user_turns_the_check_off() {
        let mut updates = Updates {
            prefs: UpdatePrefs {
                check: false,
                ..UpdatePrefs::default()
            },
            ..Updates::default()
        };
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
