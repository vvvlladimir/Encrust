//! Fetching the feed and the archive, and putting the new binaries where the old ones are.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::feed::{self, Download, FEED_URL, Offer, Version};
use super::{UpdateError, verify};

/// The feed is a few kilobytes; anything near this is not the feed.
const FEED_LIMIT: u64 = 1 << 20;
/// A release archive is tens of megabytes; this is room for growth, not a target.
const ARCHIVE_LIMIT: u64 = 512 << 20;

const FEED_TIMEOUT: Duration = Duration::from_secs(20);
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// The two binaries of the archive, inside its `encrust/` directory as `release.yml` packs it.
const APP: &str = if cfg!(windows) {
    "encrust.exe"
} else {
    "encrust"
};
const CLI: &str = if cfg!(windows) { "slice.exe" } else { "slice" };

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
/// replaces `exe` with what it holds, and the `slice` beside `exe` when there is one.
pub fn install(download: &Download, version: Version, exe: &Path) -> Result<(), UpdateError> {
    let archive = fetch(&download.url, ARCHIVE_LIMIT, ARCHIVE_TIMEOUT)?;
    let expected = verify::trusted_comment(version, &download.archive);
    verify::verify(
        verify::RELEASE_KEY,
        &archive,
        &download.signature,
        &expected,
    )?;
    let binaries = unpack(&archive, &download.archive)?;
    swap(exe, &binaries)
}

/// The binary this window was started from, unless it is a development build: that one is
/// updated by building it, and replacing it would only confuse the next build.
pub fn current_exe() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        return None;
    }
    std::env::current_exe().ok()
}

/// Removes what a swap on Windows had to leave behind, the old binary it could not delete
/// while it ran. Called on start, when nothing runs it any more.
pub fn sweep() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = fs::remove_file(aside(&exe));
    }
}

fn fetch(url: &str, limit: u64, timeout: Duration) -> Result<Vec<u8>, UpdateError> {
    let failed = |source| UpdateError::Fetch {
        url: url.to_owned(),
        source: Box::new(source),
    };
    let agent: ureq::Agent = ureq::config::Config::builder()
        .timeout_global(Some(timeout))
        .user_agent(concat!("Encrust/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(failed)?;
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(failed)
}

fn unpack(archive: &[u8], name: &str) -> Result<Binaries, UpdateError> {
    let mut entries = if name.ends_with(".zip") {
        unzip(archive)?
    } else {
        untar(archive)?
    };
    let mut take = |file: &str| entries.remove(&format!("encrust/{file}"));
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
        let path = file.name().replace('\\', "/");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(UpdateError::Archive)?;
        entries.insert(path, bytes);
    }
    Ok(entries)
}

/// Puts the binaries beside `exe` in place of the ones there. `slice` is replaced only
/// where it was installed alongside.
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
mod tests {
    use super::*;

    #[cfg(not(windows))]
    fn fixture(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/update")
            .join(name);
        fs::read(path).expect("the update fixtures are checked in")
    }

    /// A directory of its own under the system's temporary one, emptied first.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("encrust-update-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the temporary directory is writable");
        dir
    }

    fn zipped(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .expect("an entry starts");
            std::io::Write::write_all(&mut zip, bytes).expect("and is written");
        }
        zip.finish().expect("the archive closes").into_inner()
    }

    /// The fixture holds Unix names; on Windows the zip test covers the same path.
    #[cfg(not(windows))]
    #[test]
    fn the_tarball_release_yml_packs_holds_both_binaries() {
        let binaries = unpack(&fixture("encrust-test.tar.gz"), "encrust-test.tar.gz")
            .expect("the fixture unpacks");
        assert_eq!(binaries.app, b"new app\n");
        assert_eq!(binaries.cli.as_deref(), Some(&b"new cli\n"[..]));
    }

    #[test]
    fn the_zip_release_yml_packs_holds_both_binaries() {
        let archive = zipped(&[
            (&format!("encrust/{APP}"), b"app"),
            (&format!("encrust/{CLI}"), b"cli"),
            ("encrust/LICENSE", b"text"),
        ]);
        let binaries = unpack(&archive, "encrust-windows-x86_64.zip").expect("the zip unpacks");
        assert_eq!(binaries.app, b"app");
        assert_eq!(binaries.cli.as_deref(), Some(&b"cli"[..]));
    }

    #[test]
    fn an_archive_without_the_window_is_refused() {
        let archive = zipped(&[("encrust/LICENSE", b"text")]);
        let unpacked = unpack(&archive, "a.zip");
        assert!(matches!(unpacked, Err(UpdateError::Missing(name)) if name == APP));
    }

    #[test]
    fn bytes_that_are_no_archive_are_refused() {
        assert!(matches!(
            unpack(b"not gzip", "a.tar.gz"),
            Err(UpdateError::Archive(_))
        ));
        assert!(matches!(
            unpack(b"not zip", "a.zip"),
            Err(UpdateError::Archive(_))
        ));
    }

    #[test]
    fn both_binaries_are_replaced_and_stay_runnable() {
        let dir = scratch("swap");
        let exe = dir.join(APP);
        let cli = dir.join(CLI);
        fs::write(&exe, b"old app").expect("written");
        fs::write(&cli, b"old cli").expect("written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).expect("set");
        }
        let binaries = Binaries {
            app: b"new app".to_vec(),
            cli: Some(b"new cli".to_vec()),
        };

        swap(&exe, &binaries).expect("both files are writable");

        assert_eq!(fs::read(&exe).expect("read"), b"new app");
        assert_eq!(fs::read(&cli).expect("read"), b"new cli");
        assert!(!staged(&exe).exists(), "nothing is left staged");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&exe).expect("read").permissions().mode();
            assert_eq!(
                mode & 0o777,
                0o755,
                "the new binary is as runnable as the old"
            );
        }
    }

    #[test]
    fn a_window_installed_alone_gets_no_slice_beside_it() {
        let dir = scratch("alone");
        let exe = dir.join(APP);
        fs::write(&exe, b"old app").expect("written");
        let binaries = Binaries {
            app: b"new app".to_vec(),
            cli: Some(b"new cli".to_vec()),
        };
        swap(&exe, &binaries).expect("the file is writable");
        assert!(!dir.join(CLI).exists());
    }

    #[test]
    fn a_binary_that_cannot_be_replaced_says_where() {
        let dir = scratch("missing");
        let exe = dir.join("gone").join(APP);
        let binaries = Binaries {
            app: b"new app".to_vec(),
            cli: None,
        };
        let swapped = swap(&exe, &binaries);
        assert!(matches!(swapped, Err(UpdateError::Replace { path, .. }) if path == exe));
    }

    #[test]
    fn a_feed_that_cannot_be_reached_is_a_fetch_failure() {
        // Port 9 is discard, closed on any machine that runs these tests.
        let fetched = fetch("http://127.0.0.1:9/latest.json", FEED_LIMIT, FEED_TIMEOUT);
        assert!(matches!(fetched, Err(UpdateError::Fetch { .. })));
    }
}
