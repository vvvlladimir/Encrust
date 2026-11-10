use super::*;
use crate::files::Handed;
use crate::preview::SETTLE;

/// The window with a printer chosen and a cube on its plate, imported.
fn app_with_a_cube() -> SlicerApp {
    let fixture =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.stl");
    let mut app = SlicerApp::default();
    let printer = "elegoo-saturn-4-ultra";
    let profile = app.machine.slicing.catalogue.printer(printer);
    let profile = profile.expect("the printer ships").profile.clone();
    crate::profiles::apply_printer(&mut app.window(), profile, Some(printer.to_owned()));
    app.open_model(Handed::Path(fixture));
    while app.doc.imports.poll(
        &mut app.doc.scene,
        &app.doc.plate,
        &mut app.view.camera,
        &mut app.machine.status,
    ) {
        std::thread::yield_now();
    }
    app
}

#[test]
fn a_plate_left_still_is_cut_and_checked_quickly_without_a_word() {
    let mut app = app_with_a_cube();
    let at = Instant::now();
    assert_eq!(app.check_in_background(at), Some(SETTLE), "it waits first");
    assert!(!app.machine.preview.is_building());

    let settled = at + SETTLE;
    app.check_in_background(settled);
    assert!(app.machine.preview.is_building(), "then cuts the stack");
    assert!(!app.machine.preview.is_cutting_in_view(), "with no notice");
    while app.machine.preview.poll(&mut app.machine.status) {
        std::thread::yield_now();
    }
    while app.check_in_background(settled).is_some() {
        std::thread::yield_now();
    }
    let settings = app.machine.slicing.raster_settings().expect("a printer");
    let fold = app.machine.slicing.fold();
    assert!(
        app.machine
            .preview
            .quick_measured(&settings, fold)
            .is_some()
    );
    assert!(app.machine.preview.measured(&settings, fold).is_none());
    assert_eq!(app.mode, Mode::Prepare, "the view stays on the model");
}
