use super::*;

/// A directory of its own under the system's temporary one, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-update-{name}-{}", std::process::id()));
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

fn tarred(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gzip);
    for (name, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, name, *bytes)
            .expect("an entry is written");
    }
    let gzip = tar.into_inner().expect("the archive closes");
    gzip.finish().expect("the gzip stream closes")
}

#[test]
fn the_tarball_release_yml_packs_holds_both_binaries() {
    let archive = tarred(&[
        (&format!("{DIR}/{APP}"), b"new app"),
        (&format!("{DIR}/{CLI}"), b"new cli"),
    ]);
    let binaries = unpack(&archive, "encrust-linux-x86_64.tar.gz").expect("the tarball unpacks");
    assert_eq!(binaries.app, b"new app");
    assert_eq!(binaries.cli.as_deref(), Some(&b"new cli"[..]));
}

#[test]
fn the_zip_release_yml_packs_holds_both_binaries() {
    let archive = zipped(&[
        (&format!("{DIR}/{APP}"), b"app"),
        (&format!("{DIR}/{CLI}"), b"cli"),
        (&format!("{DIR}/LICENSE"), b"text"),
    ]);
    let binaries = unpack(&archive, "encrust-windows-x86_64.zip").expect("the zip unpacks");
    assert_eq!(binaries.app, b"app");
    assert_eq!(binaries.cli.as_deref(), Some(&b"cli"[..]));
}

#[test]
fn an_appimage_is_not_replaced_from_the_window() {
    assert!(packaged(
        Path::new("/tmp/.mount_x/usr/bin/encrust-gui"),
        true
    ));
}

#[test]
fn a_store_package_is_found_on_any_drive_and_a_home_install_is_not() {
    let store = "C:/Program Files/WindowsApps/VladimirVarvorkin.Encrust_0.3.0.0_x64__ysasz88zzhe46";
    assert!(in_store_package(&Path::new(store).join(APP)));
    assert!(in_store_package(Path::new(
        "D:/windowsapps/Encrust/encrust-gui.exe"
    )));
    assert!(!in_store_package(Path::new(
        "C:/Users/me/AppData/Local/Encrust/encrust-gui.exe"
    )));
}

#[cfg(target_os = "linux")]
#[test]
fn a_deb_install_is_not_replaced_but_a_home_one_is() {
    assert!(packaged(Path::new("/usr/bin/encrust-gui"), false));
    assert!(!packaged(Path::new("/home/u/Encrust/encrust-gui"), false));
}

#[test]
fn an_archive_without_the_window_is_refused() {
    let archive = zipped(&[(&format!("{DIR}/LICENSE"), b"text")]);
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
fn a_window_installed_alone_gets_no_command_line_beside_it() {
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

#[test]
fn a_download_cancelled_before_it_starts_is_cancelled_not_failed() {
    let progress = Progress::default();
    progress.cancel();
    let fetched = fetch_counted("http://127.0.0.1:9/encrust.tar.gz", &progress);
    assert!(matches!(fetched, Err(UpdateError::Cancelled)));
    assert_eq!(progress.bytes(), (0, None), "nothing was read");
}

#[test]
fn an_install_walks_its_steps_from_the_download() {
    let progress = Progress::default();
    let download = Download {
        url: "http://127.0.0.1:9/encrust.tar.gz".to_owned(),
        archive: "encrust.tar.gz".to_owned(),
        signature: String::new(),
    };
    let exe = scratch("steps").join("encrust-app");
    let installed = install(&download, Version::default(), &exe, &progress);
    assert!(matches!(installed, Err(UpdateError::Fetch { .. })));
    assert_eq!(
        progress.step(),
        Step::Download,
        "it failed where it started"
    );
}
