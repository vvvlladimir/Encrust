//! What settings' tests write profiles into.

use printer_profiles::Catalogue;

/// The shipped catalogue over a directory of its own, which every write lands in.
pub(crate) fn writable(name: &str) -> Catalogue {
    let dir = std::env::temp_dir().join(format!("encrust-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Catalogue::with_root(&dir).expect("a missing directory reads as empty")
}
