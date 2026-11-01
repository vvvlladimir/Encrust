//! Fetching the feed and the archive, and putting the new binaries where the old ones are.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use super::feed::{self, Download, FEED_URL, Offer, Version};
use super::progress::{Progress, Step};
use super::{UpdateError, verify};
use crate::i18n::tr;

/// The feed is a few kilobytes; anything near this is not the feed.
const FEED_LIMIT: u64 = 1 << 20;
/// A release archive is tens of megabytes; this is room for growth, not a target.
const ARCHIVE_LIMIT: u64 = 512 << 20;

const FEED_TIMEOUT: Duration = Duration::from_secs(20);
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// The archive's directory, renamed with the binaries so that a window that still reads
/// `encrust/encrust` as itself finds nothing rather than the command line (ADR 0176).
const DIR: &str = "Encrust";
/// The two binaries of the archive, inside [`DIR`] as `release.yml` packs it.
const APP: &str = if cfg!(windows) {
    "encrust-gui.exe"
} else {
    "encrust-gui"
};
const CLI: &str = if cfg!(windows) {
    "encrust.exe"
} else {
    "encrust"
};

/// The binaries a verified archive holds.
struct Binaries {
    app: Vec<u8>,
    cli: Option<Vec<u8>>,
}

/// What the feed offers a build at `running`.
pub fn check(running: Version) -> Result<Option<Offer>, UpdateError> {
    let text = fetch(FEED_URL, FEED_LIMIT, FEED_TIMEOUT)?;
    feed::read(&text, running, feed::platform())
}

/// Downloads `download`, refuses it unless the release key signed it for `version`, and
/// replaces `exe` with what it holds, and the `encrust` beside `exe` when there is one.
/// `progress` follows it, and stops the download when cancelled.
pub fn install(
    download: &Download,
    version: Version,
    exe: &Path,
    progress: &Progress,
) -> Result<(), UpdateError> {
    progress.enter(Step::Download);
    let archive = fetch_counted(&download.url, progress)?;
    progress.enter(Step::Verify);
    let expected = verify::trusted_comment(version, &download.archive);
    verify::verify(
        verify::RELEASE_KEY,
        &archive,
        &download.signature,
        &expected,
    )?;
    progress.enter(Step::Install);
    let binaries = unpack(&archive, &download.archive)?;
    swap(exe, &binaries)
}

/// The binary this window was started from, or why the window must not replace it.
pub fn current_exe() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        return Err(tr!("update-dev-build"));
    }
    let exe = std::env::current_exe().map_err(|_| tr!("update-no-exe"))?;
    if cfg!(windows) && in_store_package(&exe) {
        return Err(tr!("update-by-store"));
    }
    if packaged(&exe, std::env::var_os("APPIMAGE").is_some()) {
        return Err(tr!("update-by-package"));
    }
    Ok(exe)
}

/// Whether this window runs from the Store's package, which only the Store updates (ADR 0286).
pub fn from_store() -> bool {
    static STORE: LazyLock<bool> = LazyLock::new(|| {
        cfg!(windows) && std::env::current_exe().is_ok_and(|exe| in_store_package(&exe))
    });
    *STORE
}

/// Windows installs every MSIX package under a `WindowsApps` directory, on whichever drive.
fn in_store_package(exe: &Path) -> bool {
    exe.components()
        .any(|part| part.as_os_str().eq_ignore_ascii_case("WindowsApps"))
}

/// An AppImage runs from a read-only mount, and what a `.deb` put under `/usr` is the
/// package manager's to replace.
fn packaged(exe: &Path, appimage: bool) -> bool {
    appimage || (cfg!(target_os = "linux") && exe.starts_with("/usr"))
}

/// Removes what a swap on Windows had to leave behind, the old binary it could not delete
/// while it ran. Called on start, when nothing runs it any more.
pub fn sweep() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = fs::remove_file(aside(&exe));
    }
}

fn fetch(url: &str, limit: u64, timeout: Duration) -> Result<Vec<u8>, UpdateError> {
    let failed = |source| fetch_failed(url, source);
    let mut response = agent(timeout).get(url).call().map_err(failed)?;
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(failed)
}

/// The archive at `url`, a chunk at a time so `progress` counts it and can stop it.
fn fetch_counted(url: &str, progress: &Progress) -> Result<Vec<u8>, UpdateError> {
    let failed = |source| fetch_failed(url, source);
    if progress.is_cancelled() {
        return Err(UpdateError::Cancelled);
    }
    let mut response = agent(ARCHIVE_TIMEOUT).get(url).call().map_err(failed)?;
    if let Some(total) = response.body().content_length() {
        progress.expect(total);
    }
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(ARCHIVE_LIMIT)
        .reader();
    let (mut archive, mut chunk) = (Vec::new(), vec![0; CHUNK]);
    loop {
        if progress.is_cancelled() {
            return Err(UpdateError::Cancelled);
        }
        let read = reader
            .read(&mut chunk)
            .map_err(|error| failed(ureq::Error::Io(error)))?;
        if read == 0 {
            return Ok(archive);
        }
        archive.extend_from_slice(&chunk[..read]);
        progress.add(read as u64);
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::config::Config::builder()
        .timeout_global(Some(timeout))
        .user_agent(concat!("Encrust/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn fetch_failed(url: &str, source: ureq::Error) -> UpdateError {
    UpdateError::Fetch {
        url: url.to_owned(),
        source: Box::new(source),
    }
}

/// How much of the archive is read between two looks at whether it was cancelled, bytes.
const CHUNK: usize = 64 << 10;

fn unpack(archive: &[u8], name: &str) -> Result<Binaries, UpdateError> {
    let mut entries = if name.ends_with(".zip") {
        unzip(archive)?
    } else {
        untar(archive)?
    };
    let mut take = |file: &str| entries.remove(&format!("{DIR}/{file}"));
    let app = take(APP).ok_or_else(|| UpdateError::Missing(APP.to_owned()))?;
    Ok(Binaries {
        app,
        cli: take(CLI),
    })
}

type Entries = std::collections::BTreeMap<String, Vec<u8>>;

fn untar(archive: &[u8]) -> Result<Entries, UpdateError> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut entries = Entries::new();
    for entry in tar.entries().map_err(UpdateError::Archive)? {
        let mut entry = entry.map_err(UpdateError::Archive)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(UpdateError::Archive)?;
        let path = path.to_string_lossy().into_owned();
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(UpdateError::Archive)?;
        entries.insert(path, bytes);
    }
    Ok(entries)
}

fn unzip(archive: &[u8]) -> Result<Entries, UpdateError> {
    let invalid = |error: zip::result::ZipError| UpdateError::Archive(error.into());
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).map_err(invalid)?;
    let mut entries = Entries::new();
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(invalid)?;
        if !file.is_file() {
            continue;
        }
        let path = file.name().map_err(invalid)?.replace('\\', "/");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(UpdateError::Archive)?;
        entries.insert(path, bytes);
    }
    Ok(entries)
}

/// Puts the binaries beside `exe` in place of the ones there. The command line is replaced
/// only where it was installed alongside.
fn swap(exe: &Path, binaries: &Binaries) -> Result<(), UpdateError> {
    replace_running(exe, &binaries.app)?;
    let cli = exe.with_file_name(CLI);
    if let Some(bytes) = &binaries.cli
        && cli.is_file()
    {
        replace(&cli, bytes)?;
    }
    Ok(())
}

/// Windows refuses to overwrite a binary that is running but lets it be renamed, so it is
/// moved aside first and swept away on the next start. Elsewhere a rename over it is enough.
fn replace_running(exe: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    if cfg!(windows) {
        let old = aside(exe);
        let failed = |source| UpdateError::Replace {
            path: exe.to_path_buf(),
            source,
        };
        let _ = fs::remove_file(&old);
        fs::rename(exe, &old).map_err(failed)?;
        if let Err(error) = replace_moved(exe, &old, bytes) {
            let _ = fs::rename(&old, exe);
            return Err(error);
        }
        return Ok(());
    }
    replace(exe, bytes)
}

/// Writes `bytes` to `target` with the permissions `from` has.
fn replace_moved(target: &Path, from: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    let failed = |source| UpdateError::Replace {
        path: target.to_path_buf(),
        source,
    };
    let permissions = fs::metadata(from).map_err(failed)?.permissions();
    let staged = staged(target);
    fs::write(&staged, bytes).map_err(failed)?;
    let placed =
        fs::set_permissions(&staged, permissions).and_then(|()| fs::rename(&staged, target));
    if let Err(error) = placed {
        let _ = fs::remove_file(&staged);
        return Err(failed(error));
    }
    Ok(())
}

/// Writes beside `target` and renames over it, so a failure halfway leaves the old binary
/// whole rather than a truncated one.
fn replace(target: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    replace_moved(target, target, bytes)
}

fn staged(target: &Path) -> PathBuf {
    target.with_extension("new")
}

fn aside(exe: &Path) -> PathBuf {
    exe.with_extension("old")
}

#[cfg(test)]
mod tests;
