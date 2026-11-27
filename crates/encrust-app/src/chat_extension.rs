//! The chat-app extension of this window's version and system: fetched from its release,
//! refused unless the release key signed it, and opened, which installs it (ADR 0455).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::i18n::tr;
use crate::status::Status;
use crate::updates::{
    Progress, UpdateError, Version, fetch_counted, trusted_comment, verify_release,
};

/// Why the extension was not opened.
#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error(transparent)]
    Update(#[from] UpdateError),
    #[error("{}", tr!("chat-extension-write", path = .path.display().to_string()))]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{}", tr!("chat-extension-open", path = .path.display().to_string()))]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// The fetch under way, when the user asked for the extension.
#[derive(Debug, Default)]
pub struct ChatExtension {
    running: Option<Receiver<Result<PathBuf, ExtensionError>>>,
}

impl ChatExtension {
    /// Fetches, checks and opens the extension on a thread of its own, unless one already runs.
    pub fn start(&mut self, status: &mut Status) {
        if self.running.is_some() {
            return;
        }
        let (sender, reply) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(fetch_and_open());
        });
        self.running = Some(reply);
        *status = Status::Info(tr!("chat-extension-fetching"));
    }

    /// Says on `status` how the fetch ended, once it has, and answers whether it still runs.
    pub fn poll(&mut self, status: &mut Status) -> bool {
        let Some(reply) = &self.running else {
            return false;
        };
        let ended = match reply.try_recv() {
            Ok(ended) => ended,
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => Err(UpdateError::Cancelled.into()),
        };
        self.running = None;
        *status = match ended {
            Ok(path) => Status::Info(tr!(
                "chat-extension-opened",
                path = path.display().to_string()
            )),
            Err(error) => Status::Error(tr!(
                "chat-extension-failed",
                error = error.to_string(),
                url = release_url(&file_name())
            )),
        };
        false
    }
}

/// The bundle's name on the release: `encrust-<os>-<arch>.mcpb`, as `cargo xtask mcpb` names it.
pub fn file_name() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    };
    format!("encrust-{os}-{}.mcpb", std::env::consts::ARCH)
}

/// Where this version's release serves `file`.
fn release_url(file: &str) -> String {
    format!(
        "{}/releases/download/v{}/{file}",
        env!("CARGO_PKG_REPOSITORY"),
        env!("CARGO_PKG_VERSION")
    )
}

fn fetch_and_open() -> Result<PathBuf, ExtensionError> {
    let file = file_name();
    let bundle = fetch_counted(&release_url(&file), &Progress::default())?;
    let signature = fetch_counted(
        &release_url(&format!("{file}.minisig")),
        &Progress::default(),
    )?;
    let signature = String::from_utf8_lossy(&signature);
    let version = Version::parse(env!("CARGO_PKG_VERSION"))?;
    verify_release(&bundle, &signature, &trusted_comment(version, &file))?;
    let path = std::env::temp_dir().join(&file);
    std::fs::write(&path, &bundle).map_err(|source| ExtensionError::Write {
        path: path.clone(),
        source,
    })?;
    open(&path)?;
    Ok(path)
}

/// Hands `path` to the program the system opens its kind with: the chat app, for a `.mcpb`.
fn open(path: &Path) -> Result<(), ExtensionError> {
    crate::files::open_with_system(path).map_err(|source| ExtensionError::Open {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_is_named_as_the_release_attaches_it() {
        let name = file_name();
        assert!(
            name.starts_with("encrust-") && name.ends_with(".mcpb"),
            "{name}"
        );
        assert!(name.contains(std::env::consts::ARCH), "{name}");
    }

    #[test]
    fn its_signature_is_fetched_beside_it_from_this_version() {
        let url = release_url("encrust-macos-aarch64.mcpb.minisig");
        let expected = format!(
            "/releases/download/v{}/encrust-macos-aarch64.mcpb.minisig",
            env!("CARGO_PKG_VERSION")
        );
        assert!(url.ends_with(&expected), "{url}");
    }
}
