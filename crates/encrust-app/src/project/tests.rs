use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use printer_profiles::Catalogue;

use super::*;
use crate::panels::fixtures::Parts;
use crate::plate::BuildPlate;
use crate::scene::fixtures::scene_of_boxes;
use crate::state::Doc;
use crate::workspace::Tool;
use core_geometry::Vec3;

const SATURN: &str = "elegoo-saturn-4-ultra";
const MARS: &str = "elegoo-mars-4-ultra";

/// The shipped catalogue over a profile directory of its own, with the machines `ids` added
/// to the user's, as Machine and resin adds them.
pub(super) fn with_machines(ids: &[&str]) -> Catalogue {
    static RUN: AtomicU32 = AtomicU32::new(0);
    let run = RUN.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("encrust-project-{}-{run}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut catalogue = Catalogue::with_root(&dir).expect("a missing directory reads as empty");
    for id in ids {
        let profile = catalogue.printer(id).expect("ships").profile.clone();
        catalogue
            .save_printer(id, &profile)
            .expect("the directory is writable");
    }
    catalogue
}

fn stand_on(window: &mut Window, id: &str) {
    let profile = window
        .machine
        .slicing
        .catalogue
        .printer(id)
        .expect("the printer ships")
        .profile
        .clone();
    crate::profiles::apply_printer(window, profile, Some(id.to_owned()));
}

/// The plate as a project file handed over in memory, as a drop or a browser hands it.
fn saved(window: &Window) -> Handed {
    let mut bytes = Cursor::new(Vec::new());
    core_engine::project::write_to(&mut bytes, &captured(window))
        .expect("a project in memory writes");
    Handed::bytes("saved.encrust", Arc::from(bytes.into_inner()))
}

/// A project saved on `saved_on`, opened by a user who has `yours` and works on `working_on`.
fn opened(saved_on: &str, yours: &[&str], working_on: &str) -> Parts {
    let mut parts = Parts::of(Doc::default(), Tool::Supports);
    parts.machine.slicing.catalogue = with_machines(&[saved_on]);
    let file = saved(&{
        let mut window = parts.window();
        stand_on(&mut window, saved_on);
        window
    });
    parts.machine.slicing.catalogue = with_machines(yours);
    let mut window = parts.window();
    stand_on(&mut window, working_on);
    open(&mut window, &file);
    parts
}

fn plate_of(id: &str) -> BuildPlate {
    let catalogue = Catalogue::bundled().expect("the shipped profiles read");
    BuildPlate::from_profile(&catalogue.printer(id).expect("ships").profile)
}

#[test]
fn a_project_saved_on_another_machine_of_the_users_stands_the_plate_under_it() {
    let parts = opened(SATURN, &[SATURN, MARS], MARS);

    assert_eq!(parts.machine.slicing.printer_id.as_deref(), Some(SATURN));
    assert_eq!(
        parts.doc.plate,
        plate_of(SATURN),
        "the plate is the {SATURN}'s, the one its layers are cut for"
    );
}

#[test]
fn a_project_from_a_machine_the_user_does_not_have_opens_on_the_one_in_hand() {
    let parts = opened(SATURN, &[MARS], MARS);

    assert_eq!(parts.machine.slicing.printer_id.as_deref(), Some(MARS));
    assert_eq!(parts.doc.plate, plate_of(MARS), "the {MARS}'s plate stays");
}

#[test]
fn a_project_dropped_over_unsaved_models_asks_before_it_replaces_them() {
    let mut parts = opened(SATURN, &[SATURN], SATURN);
    let file = saved(&parts.window());
    parts.doc.scene = scene_of_boxes(&[(Vec3::ZERO, Vec3::ONE)]);
    let mut window = parts.window();

    open(&mut window, &file);

    assert_eq!(window.doc.scene.objects().len(), 1, "the models stay");
    let Some(asking @ Asking::Open(_)) = window.doc.project.asking.take() else {
        panic!("the window asks before opening");
    };
    go_on(&egui::Context::default(), &mut window, asking);
    assert!(window.doc.scene.is_empty(), "opened without saving");
}

#[test]
fn a_project_dropped_on_a_plate_without_models_opens_at_once() {
    let mut parts = opened(SATURN, &[SATURN, MARS], MARS);
    let file = saved(&parts.window());
    let mut window = parts.window();
    stand_on(&mut window, SATURN);

    open(&mut window, &file);

    assert!(
        window.doc.project.asking.is_none(),
        "nothing to lose, nothing asked"
    );
}
