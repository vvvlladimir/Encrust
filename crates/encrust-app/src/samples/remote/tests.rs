use std::sync::Arc;
use std::time::{Duration, Instant};

use core_geometry::Transform;
use core_mesh_io::{MeshLoader, StlLoader};

use super::*;
use crate::recent::picture;

/// A folder of the test's own under the system's temporary one.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-remote-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Shade and coverage pixels of a kept picture's PNG.
fn shades(png: &[u8]) -> Vec<u8> {
    image::load_from_memory(png)
        .expect("a kept picture decodes")
        .to_luma_alpha8()
        .into_raw()
}

#[test]
fn each_picture_is_rendered_from_its_model_in_the_repository() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for remote in &REMOTES {
        let model = root.join("test-prints").join(remote.file);
        let mesh = StlLoader.load(&model).expect("the test print reads").mesh;
        let parts = vec![(Arc::new(mesh), Transform::default())];
        let rendered = picture::png(&parts)
            .expect("the picture encodes")
            .expect("the model has faces");
        // As the panel snapshots are retaken: see docs/decisions/0215.
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            let kept = root
                .join("assets/samples")
                .join(remote.file)
                .with_extension("png");
            std::fs::write(kept, &rendered).expect("the picture is written");
            continue;
        }
        let (now, kept) = (shades(&rendered), shades(remote.picture_png));
        assert_eq!(now.len(), kept.len(), "{} changed size", remote.file);
        // A renderer's last bits may differ between machines; a changed model moves far more.
        let moved = now
            .iter()
            .zip(&kept)
            .filter(|(a, b)| a.abs_diff(**b) > 16)
            .count();
        assert!(
            moved * 200 < now.len(),
            "{} differs from its picture in {moved} of {} bytes; retake it with UPDATE_SNAPSHOTS=1",
            remote.file,
            now.len()
        );
    }
}

#[test]
fn a_kept_file_is_whole_and_leaves_nothing_beside_it() {
    let dir = scratch("kept");
    let path = dir.join("model.stl");
    keep(&path, b"solid").expect("a fresh folder takes the file");
    assert_eq!(std::fs::read(&path).expect("it is there"), b"solid");
    assert!(!path.with_extension("part").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_that_cannot_be_written_is_a_write_error() {
    let dir = scratch("blocked");
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    let blocker = dir.join("taken");
    std::fs::write(&blocker, b"").expect("a plain file is written");
    let error = keep(&blocker.join("model.stl"), b"solid").expect_err("a file is no folder");
    assert!(matches!(error, FetchError::Write { .. }), "{error:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_server_that_does_not_answer_is_a_fetch_error() {
    let dir = scratch("refused");
    let fetching = Fetching::start("http://127.0.0.1:9/model.stl".into(), dir.join("model.stl"));
    let deadline = Instant::now() + Duration::from_secs(20);
    let ended = loop {
        if let Some(ended) = fetching.ended() {
            break ended;
        }
        assert!(
            Instant::now() < deadline,
            "a refused connection ends at once"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let error = ended.expect_err("nothing listens on the discard port");
    assert!(matches!(error, FetchError::Fetch(_)), "{error:?}");
}

#[test]
fn a_failed_fetch_is_said_on_the_status_line_and_frees_its_place() {
    let dir = scratch("polled");
    let mut fetches = Fetches::default();
    fetches.running[0] = Some(Fetching::start(
        "http://127.0.0.1:9/model.stl".into(),
        dir.join("model.stl"),
    ));
    let mut status = Status::default();
    let deadline = Instant::now() + Duration::from_secs(20);
    while fetches.poll(&mut status) {
        assert!(
            Instant::now() < deadline,
            "a refused connection ends at once"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(status.is_error(), "{status:?}");
    assert!(fetches.get(0).is_none(), "the tile can be clicked again");
}
