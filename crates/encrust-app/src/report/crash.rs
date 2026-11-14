//! The panic the window last closed on, kept on disk for the next start's report sheet and
//! forgotten on a clean exit; see `docs/decisions/0446`.

use std::path::{Path, PathBuf};
/// Records each panic before the default hook prints it. The last one wins: a panic the
/// boolean engine raises is caught and lived through, the one that ends the run is not.
pub fn install() {
    let Some(path) = path() else {
        return;
    };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map_or_else(String::new, |at| format!(" at {at}"));
        let message = info
            .payload_as_str()
            .map_or_else(String::new, |said| super::scrub::scrubbed(said, &[]));
        // A failed write has nowhere to be reported: the process may already be going down.
        let _ = std::fs::write(&path, described(&location, &message));
        previous(info);
    }));
}

/// The panic the last run closed on, removed from disk so it is offered only once.
pub fn take() -> Option<String> {
    taken(&path()?)
}

/// Forgets this run's panic, if any: the window closed as it was asked to.
pub fn forget() {
    if let Some(path) = path() {
        let _ = std::fs::remove_file(path);
    }
}

fn described(location: &str, message: &str) -> String {
    let thread = std::thread::current();
    let thread = thread.name().unwrap_or("unnamed");
    let backtrace = std::backtrace::Backtrace::force_capture();
    format!(
        "Encrust {}, thread '{thread}' panicked{location}:\n{message}\n\n{backtrace}",
        env!("CARGO_PKG_VERSION")
    )
}

fn taken(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let _ = std::fs::remove_file(path);
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// A browser has no file to keep it in, and a tab that panics is reloaded by hand.
#[cfg(not(target_arch = "wasm32"))]
fn path() -> Option<PathBuf> {
    Some(printer_profiles::user_dir()?.with_file_name("last-crash.txt"))
}

#[cfg(target_arch = "wasm32")]
fn path() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crash_is_offered_once_then_gone() {
        let path = std::env::temp_dir().join(format!("encrust-crash-{}", std::process::id()));
        std::fs::write(&path, "thread 'main' panicked\n").expect("the file can be written");
        assert_eq!(taken(&path).as_deref(), Some("thread 'main' panicked"));
        assert_eq!(taken(&path), None, "a crash is offered only once");
    }

    #[test]
    fn a_crash_states_the_build_and_where_it_happened() {
        let text = described(" at src/a.rs:1:2", "index out of bounds");
        assert!(text.starts_with(concat!("Encrust ", env!("CARGO_PKG_VERSION"))));
        assert!(text.contains("panicked at src/a.rs:1:2:\nindex out of bounds"));
    }
}
