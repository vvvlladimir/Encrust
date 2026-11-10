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

/// While Fix all holds the plate, neither the quick check nor Check's own read starts a cut.
#[test]
fn a_recipe_holding_the_plate_is_left_to_cut_and_measure_alone() {
    let mut app = app_with_a_cube();
    let (doc, tools, machine) = (&mut app.doc, &mut app.tools, &mut app.machine);
    crate::prepare::start_fixes(doc, tools, machine).expect("the cube is on the plate");
    assert!(app.tools.prepare.is_holding());
    app.tool = Tool::Check;

    let at = Instant::now();
    assert_eq!(
        app.check_in_background(at),
        None,
        "no quick check is waited for"
    );
    assert_eq!(app.check_in_background(at + SETTLE), None);
    app.refresh_preview();
    assert!(!app.machine.preview.is_building(), "nothing is cut");
}

/// A quick check under way when a recipe takes the plate is stopped.
#[test]
fn a_quick_check_under_way_stops_when_a_recipe_takes_the_plate() {
    let mut app = app_with_a_cube();
    let at = Instant::now();
    app.check_in_background(at);
    app.check_in_background(at + SETTLE);
    assert!(
        app.machine.preview.is_building(),
        "the quick check's cut started"
    );

    let (doc, tools, machine) = (&mut app.doc, &mut app.tools, &mut app.machine);
    crate::prepare::start_fixes(doc, tools, machine).expect("the cube is on the plate");
    app.check_in_background(at + SETTLE);
    assert!(!app.machine.preview.is_building(), "its cut is let go");
}
