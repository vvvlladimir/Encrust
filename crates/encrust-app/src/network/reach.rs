use crate::i18n::tr;
use printer_link::SendError;

/// Whether a machine answers: what its watch heard last, else the last scan's word
/// (`docs/decisions/0262`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// It answered the last scan.
    Answered,
    /// It was asked and said nothing.
    Silent,
    /// Nothing has been asked since the window opened.
    Unasked,
}

/// Why a printer could not be asked, as the window words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trouble {
    /// The failure with every cause under it, on one line.
    pub message: String,
    /// Whether the system found no route to it, which on macOS may be its privacy setting.
    pub unreachable: bool,
}

impl Trouble {
    pub(super) fn of(error: SendError) -> Self {
        let unreachable = error.is_unreachable();
        Self {
            message: format!("{:#}", anyhow::Error::new(error)),
            unreachable,
        }
    }

    /// What the user can do about it, when there is something.
    pub fn advice(&self) -> Option<String> {
        advice(self.unreachable && cfg!(target_os = "macos"), *IN_A_BUNDLE)
    }
}

/// Whether the window runs from an app bundle, which macOS asks about the local network
/// itself, rather than as a binary that takes the permission of whatever started it.
pub(super) static IN_A_BUNDLE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    std::env::current_exe().is_ok_and(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
});

/// What to tell a user whose printer has no route, when macOS may be the one `blocked`.
pub(super) fn advice(blocked: bool, bundled: bool) -> Option<String> {
    match (blocked, bundled) {
        (false, _) => None,
        (true, true) => Some(tr!("network-advice-app")),
        (true, false) => Some(tr!("network-advice-terminal")),
    }
}
