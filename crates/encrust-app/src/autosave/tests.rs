use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::panels::fixtures::Parts;
use crate::scene::fixtures::scene_of_boxes;
use crate::state::Doc;
use crate::workspace::Tool;
use core_geometry::Vec3;

/// A recovery folder of the test's own.
fn folder() -> PathBuf {
    static RUN: AtomicU32 = AtomicU32::new(0);
    let run = RUN.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("encrust-autosave-{}-{run}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn one_box() -> Project {
    let mut parts = Parts::of(Doc::default(), Tool::Supports);
    parts.doc.scene = scene_of_boxes(&[(Vec3::ZERO, Vec3::ONE)]);
    crate::project::unsaved(&parts.window())
        .expect("a box never saved is unsaved")
        .0
}

/// Copies one box as a run would, and waits for the write.
fn copied(autosave: &mut Autosave, digest: u64, belonged: Option<&str>) -> Option<ProjectError> {
    autosave.keep(Some((one_box(), digest)), belonged.map(PathBuf::from));
    while autosave
        .writing
        .as_ref()
        .is_some_and(|writing| !writing.is_finished())
    {
        std::thread::yield_now();
    }
    autosave.poll()
}

fn files_in(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, Iterator::count)
}

#[test]
fn a_copy_whose_run_ended_without_closing_is_offered_with_its_project() {
    let dir = folder();
    let mut crashed = Autosave::in_dir(&dir);
    assert!(copied(&mut crashed, 1, Some("ring.encrust")).is_none());
    drop(crashed);

    let next = Autosave::in_dir(&dir);

    let left = next.offered().expect("the copy is offered");
    assert_eq!(left.belonged, Some(PathBuf::from("ring.encrust")));
    let project = core_engine::project::load(&left.copy()).expect("the copy reads back");
    assert_eq!(project.models.len(), 1, "the box is in it");
}

#[test]
fn a_copy_whose_run_still_runs_is_not_offered() {
    let dir = folder();
    let mut running = Autosave::in_dir(&dir);
    assert!(copied(&mut running, 1, None).is_none());

    assert!(
        Autosave::in_dir(&dir).offered().is_none(),
        "its lock is held"
    );
}

#[test]
fn a_run_that_closes_leaves_nothing() {
    let dir = folder();
    let mut run = Autosave::in_dir(&dir);
    assert!(copied(&mut run, 1, None).is_none());

    run.finish();

    assert_eq!(files_in(&dir), 0, "copy, name and lock all go");
}

#[test]
fn a_plate_saved_again_drops_its_copy() {
    let dir = folder();
    let mut run = Autosave::in_dir(&dir);
    assert!(copied(&mut run, 1, None).is_none());

    run.keep(None, None);
    drop(run);

    assert!(
        Autosave::in_dir(&dir).offered().is_none(),
        "nothing left to restore"
    );
}

#[test]
fn an_unchanged_plate_is_not_written_again() {
    let dir = folder();
    let mut run = Autosave::in_dir(&dir);
    assert!(copied(&mut run, 7, None).is_none());

    run.keep(Some((one_box(), 7)), None);

    assert!(run.writing.is_none(), "the same digest starts no write");
}

#[test]
fn the_newest_copy_is_offered_first() {
    let dir = folder();
    for (digest, name) in [(1, "older.encrust"), (2, "newer.encrust")] {
        let mut crashed = Autosave::in_dir(&dir);
        assert!(copied(&mut crashed, digest, Some(name)).is_none());
        if name == "older.encrust" {
            let copy = crashed.slot.as_ref().expect("a slot").files().copy;
            let hour_ago = SystemTime::now() - Duration::from_secs(3_600);
            File::options()
                .write(true)
                .open(copy)
                .and_then(|file| file.set_modified(hour_ago))
                .expect("the copy can be dated");
        }
    }

    let next = Autosave::in_dir(&dir);

    let offered = next.offered().expect("a copy is offered");
    assert_eq!(offered.belonged, Some(PathBuf::from("newer.encrust")));
}

#[test]
fn a_restored_copy_becomes_this_runs_own() {
    let dir = folder();
    let mut crashed = Autosave::in_dir(&dir);
    assert!(copied(&mut crashed, 1, Some("ring.encrust")).is_none());
    drop(crashed);
    let mut restoring = Autosave::in_dir(&dir);

    let left = restoring.take_offered().expect("the copy is offered");
    restoring.adopt(left);

    assert!(
        Autosave::in_dir(&dir).offered().is_none(),
        "held by the run that restored it"
    );
    drop(restoring);
    let again = Autosave::in_dir(&dir);
    let left = again
        .offered()
        .expect("offered again when that run ends badly too");
    assert_eq!(
        left.belonged,
        Some(PathBuf::from("ring.encrust")),
        "still under its project"
    );
}

#[test]
fn a_discarded_copy_is_gone() {
    let dir = folder();
    let mut crashed = Autosave::in_dir(&dir);
    assert!(copied(&mut crashed, 1, None).is_none());
    drop(crashed);
    let mut next = Autosave::in_dir(&dir);

    next.take_offered().expect("the copy is offered").discard();
    next.finish();

    assert_eq!(files_in(&dir), 0);
}

#[test]
fn a_failed_write_is_said_once_until_one_succeeds() {
    let dir = folder();
    let mut run = Autosave::in_dir(&dir);
    std::fs::remove_dir_all(&dir).expect("the folder can be taken away");

    assert!(matches!(
        copied(&mut run, 1, None),
        Some(ProjectError::Io(_))
    ));
    assert!(copied(&mut run, 2, None).is_none(), "said once");
    std::fs::create_dir_all(&dir).expect("the folder comes back");
    assert!(copied(&mut run, 3, None).is_none());
    assert!(!run.failed, "a write that succeeds ends the failure");
}

#[test]
fn the_plate_is_looked_at_once_an_interval() {
    let mut run = Autosave::in_dir(&folder());
    let start = Instant::now();

    assert!(!run.due(start), "the first frame starts the count");
    assert!(!run.due(start + EVERY - Duration::from_secs(1)));
    assert!(run.due(start + EVERY));
    assert!(
        !run.due(start + EVERY + Duration::from_secs(1)),
        "counted again from there"
    );
}

#[test]
fn nothing_is_kept_without_a_folder() {
    let mut run = Autosave::default();

    assert!(!run.due(Instant::now() + EVERY * 2));
    run.keep(Some((one_box(), 1)), None);
    assert!(run.writing.is_none());
}

#[test]
fn a_slot_is_named_a_lock_only_once_held() {
    let dir = folder();
    let run = Autosave::in_dir(&dir);

    let slot = run.slot.as_ref().expect("a slot");
    assert!(file(&dir, &slot.stem, LOCK).exists());
    assert!(!file(&dir, &slot.stem, TAKING).exists());
}
