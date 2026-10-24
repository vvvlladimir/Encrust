use super::*;

#[test]
fn the_window_opens_on_the_prepare_mode_with_nothing_selected() {
    let app = SlicerApp::default();
    assert_eq!(app.mode, Mode::Prepare);
    assert_eq!(app.tool, Tool::Position);
    assert!(
        app.view.options.grid,
        "the plate grid is on until it is turned off"
    );
}

/// BUG: the gizmo of a picked model took every press on the stage, the layer strip's
/// included, because it listened wherever the stage did.
#[test]
fn the_layer_strip_takes_a_press_while_a_model_is_picked() {
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
    use egui_kittest::kittest::Queryable as _;

    let mesh = Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, 0.0, 10.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    );
    let mut app = SlicerApp::default();
    let id = app.doc.scene.insert(crate::scene::Imported::new(
        "tetra".to_owned(),
        std::sync::Arc::new(mesh.clone()),
        Transform::default(),
        crate::scene::ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ));
    app.doc.scene.select(Some(id));

    // The faces a context is given reach it a frame later, so the first frame only
    // dresses it.
    let mut dressed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            move |ui, app: &mut SlicerApp| {
                if dressed {
                    app.window().show(ui);
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            app,
        );
    harness.run_steps(3);
    harness.get_by_label("Model and layer side by side").click();
    harness.run_steps(3);

    assert_eq!(harness.state().mode, Mode::Preview);
}

/// A screen reader finds every tool on the rail by its name, and Enter on the one in
/// focus picks it, which is the whole of driving the rail without a pointer.
#[test]
fn every_tool_on_the_rail_is_named_and_picked_from_the_keyboard() {
    use egui_kittest::kittest::Queryable as _;

    let mut app = SlicerApp::default();
    app.view.options.started = true;
    let mut dressed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut SlicerApp| {
                if dressed {
                    app.window().show(ui);
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            app,
        );
    harness.run_steps(3);
    for tool in Tool::on_rail() {
        let button = egui::accesskit::Role::Button;
        harness.get_by_role_and_label(button, &tool.label()).focus();
        harness.run_steps(2);
        harness.key_press(egui::Key::Enter);
        harness.run_steps(2);
        assert_eq!(harness.state().tool, tool, "Enter on {}", tool.label());
    }
}

#[test]
fn taking_every_support_off_a_rafted_model_is_undone_in_one_step() {
    let mut app = super::states::app(true);
    app.tools.supports.raft.enabled = true;
    let resting = Frame {
        settled: true,
        typing: false,
    };
    let ctx = egui::Context::default();
    app.record(resting, &ctx);
    let object = &mut app.doc.scene.objects_mut()[0];
    object.transform.translation.z = 10.0;
    let transform = object.transform;
    object
        .supports
        .add(core_geometry::Vec3::new(5.0, 5.0, 10.0), transform, 0);
    app.record(resting, &ctx);
    let stood_mm = app.doc.scene.objects()[0].transform.translation.z;
    assert!(stood_mm > 10.0, "the raft stood it up");

    app.doc.scene.objects_mut()[0].supports.clear();
    app.record(resting, &ctx);
    app.record(resting, &ctx);
    assert!(
        app.doc
            .history
            .undo(&mut app.doc.scene, &mut app.tools, &mut app.machine.slicing)
    );
    app.record(resting, &ctx);
    app.record(resting, &ctx);

    let object = &app.doc.scene.objects()[0];
    assert_eq!(object.supports.point_count(), 1, "the support is back");
    assert_eq!(object.transform.translation.z, stood_mm, "on its raft");
    assert!(
        app.doc.history.can_redo(),
        "and nothing was recorded over it"
    );
}

#[test]
fn the_opening_camera_looks_at_the_plate() {
    let app = SlicerApp::default();
    assert_eq!(app.view.camera.target, app.doc.plate.center());
    assert!(app.doc.scene.is_empty());
}

#[test]
fn a_file_that_cannot_be_read_leaves_the_scene_alone() {
    let mut app = SlicerApp::default();
    app.open_model(Handed::Path("does-not-exist.stl".into()));

    // The read is on a worker thread, so the failure lands on a later frame.
    while app.doc.imports.poll(
        &mut app.doc.scene,
        &app.doc.plate,
        &mut app.view.camera,
        &mut app.machine.status,
    ) {
        std::thread::yield_now();
    }

    assert!(app.doc.scene.is_empty());
    assert!(app.machine.status.is_error());
}

#[test]
fn a_sliced_file_opens_as_a_model_where_its_pixels_stood() {
    // A 0.8 x 0.4 mm block lit at pixels 4..12 by 2..6 of 0.1 mm, three layers of 0.05.
    let path = crate::preview::tests::written_goo("app-startup", 3);
    let mut app = SlicerApp::default();
    app.open_model(Handed::Path(path));

    while app.doc.imports.poll(
        &mut app.doc.scene,
        &app.doc.plate,
        &mut app.view.camera,
        &mut app.machine.status,
    ) {
        std::thread::yield_now();
    }

    assert!(!app.machine.status.is_error(), "{:?}", app.machine.status);
    let object = &app.doc.scene.objects()[0];
    assert_eq!(object.name, "encrust-preview-app-startup.goo");
    let bounds = object.world_bounds().expect("a block has bounds");
    assert!(
        (bounds.mins.x - 0.4).abs() < 0.11 && (bounds.maxs.z - 0.15).abs() < 0.11,
        "the block stands where its pixels and layers were: {bounds:?}"
    );
}

#[test]
fn a_file_no_reader_claims_is_opened_as_a_model() {
    assert!(!core_pipeline::reads_sliced_file(Path::new("cube.stl")));
    assert!(core_pipeline::reads_sliced_file(Path::new("plate.ctb")));
}

#[test]
fn a_model_opened_from_the_command_line_reaches_the_plate() {
    let fixture =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.stl");
    let mut app = SlicerApp::default();
    app.open_model(Handed::Path(fixture));

    while app.doc.imports.poll(
        &mut app.doc.scene,
        &app.doc.plate,
        &mut app.view.camera,
        &mut app.machine.status,
    ) {
        std::thread::yield_now();
    }

    assert_eq!(app.doc.scene.objects().len(), 1);
    assert!(!app.machine.status.is_error(), "a sound cube opens");
}

#[test]
fn the_view_key_runs_through_the_three_views_and_back() {
    let mut app = SlicerApp::default();
    let turn = |app: &mut SlicerApp, step| {
        shortcuts::act(&mut app.window(), shortcuts::Action::View(step));
        (app.mode, app.view.options.mask_only)
    };
    assert_eq!(turn(&mut app, 1), (Mode::Preview, false), "side by side");
    assert_eq!(turn(&mut app, 1), (Mode::Preview, true), "the layer alone");
    assert_eq!(
        turn(&mut app, 1),
        (Mode::Prepare, false),
        "round to the model"
    );
    assert_eq!(
        turn(&mut app, -1),
        (Mode::Preview, true),
        "and back the other way"
    );
}

/// Only the layer strip switches the view; a tool opens in the inspector beside it.
#[test]
fn picking_a_tool_leaves_the_view_alone() {
    let mut app = SlicerApp {
        mode: Mode::Preview,
        ..SlicerApp::default()
    };
    shortcuts::act(&mut app.window(), shortcuts::Action::Pick(Tool::Hollow));
    assert_eq!(app.tool, Tool::Hollow);
    assert_eq!(app.mode, Mode::Preview);
}

#[test]
fn picking_the_open_tool_again_folds_the_inspector() {
    let mut app = SlicerApp::default();
    let pick = |app: &mut SlicerApp, tool| {
        shortcuts::act(&mut app.window(), shortcuts::Action::Pick(tool));
    };
    pick(&mut app, Tool::Hollow);
    assert!(app.view.options.inspector);
    pick(&mut app, Tool::Hollow);
    assert!(!app.view.options.inspector, "a second press folds it");
    pick(&mut app, Tool::Hollow);
    assert!(app.view.options.inspector, "a third unfolds it");
    pick(&mut app, Tool::Hollow);
    pick(&mut app, Tool::Cut);
    assert!(app.view.options.inspector, "another tool always opens");
    assert_eq!(app.tool, Tool::Cut);
}

/// The sheet is modal: a key pressed under it moves nothing on the plate.
#[test]
fn the_sheet_holds_the_keyboard_until_it_is_closed() {
    let mut app = SlicerApp::default();
    shortcuts::act(&mut app.window(), shortcuts::Action::Sheet);
    assert!(app.view.options.sheet);

    shortcuts::act(&mut app.window(), shortcuts::Action::Pick(Tool::Hollow));
    assert_eq!(
        app.tool,
        Tool::Position,
        "the tool is not reached under the sheet"
    );
    assert!(app.view.options.sheet);

    shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
    assert!(!app.view.options.sheet, "Escape closes it");
}

/// The report sheet is modal too, and Escape closes it: `shortcuts::take` reads the
/// key before egui's pass, so the sheet would never see it otherwise.
#[test]
fn the_report_sheet_holds_the_keyboard_until_it_is_closed() {
    let mut app = SlicerApp::default();
    app.machine.report.open = true;

    shortcuts::act(&mut app.window(), shortcuts::Action::Pick(Tool::Hollow));
    assert_eq!(app.tool, Tool::Position, "the tool is not reached under it");
    assert!(app.machine.report.open);

    shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
    assert!(!app.machine.report.open, "Escape closes it");
}

/// The start page is left once and for good: a tool picked and put back does not
/// bring it back over an empty plate.
#[test]
fn the_start_page_is_left_for_good() {
    let mut app = SlicerApp::default();
    assert!(crate::panels::still_starting(&mut app.window()));
    app.tool = Tool::Measure;
    assert!(!crate::panels::still_starting(&mut app.window()));
    app.tool = Tool::default();
    assert!(!crate::panels::still_starting(&mut app.window()));
}

/// A plate with nothing to print on says so at the stage's foot. From there two zones and
/// a library pick reach a machine and a resin, each put under the plate unasked (ADR 0299).
#[test]
fn a_plate_with_nothing_to_print_on_is_set_up_from_its_notice() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable as _;

    let dir = std::env::temp_dir().join(format!("encrust-setup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = SlicerApp::default();
    app.view.options.started = true;
    app.machine.slicing.catalogue =
        printer_profiles::Catalogue::with_root(&dir).expect("a missing directory reads as empty");
    let mut dressed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut SlicerApp| {
                if dressed {
                    app.window().show(ui);
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            app,
        );
    harness.run_steps(3);
    assert!(
        !harness.state().machine.settings.machines,
        "nothing opens unasked"
    );
    harness.get_by_label("No printer or resin yet");

    for step in [
        "Set up",
        "Add a machine",
        "Elegoo",
        "Saturn 4 Ultra",
        "Add a resin",
    ] {
        harness.get_by_role_and_label(Role::Button, step).click();
        harness.run_steps(3);
    }
    let slicing = &harness.state().machine.slicing;
    assert_eq!(slicing.printer_id.as_deref(), Some("elegoo-saturn-4-ultra"));
    assert!(slicing.resin_id.is_some(), "the new resin is the plate's");

    let _ = std::fs::remove_dir_all(&dir);
}

/// A file dropped on Machine and resin is a profile rather than a model: a machine while
/// the library is in view, a resin once its machine's resins are, and either goes under
/// the plate that had none (ADR 0299).
#[test]
fn a_profile_dropped_on_machine_and_resin_is_imported_and_printed_on() {
    let dir = std::env::temp_dir().join(format!("encrust-drop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = SlicerApp::default();
    app.machine.slicing.catalogue =
        printer_profiles::Catalogue::with_root(&dir).expect("a missing directory reads as empty");
    let printer = app
        .machine
        .slicing
        .catalogue
        .export(printer_profiles::Kind::Printer, "elegoo-saturn-4-ultra")
        .expect("a shipped machine exports");
    let printer_file =
        std::env::temp_dir().join(format!("encrust-drop-{}.toml", std::process::id()));
    std::fs::write(&printer_file, printer).expect("the temp directory is writable");
    crate::panels::open_machines(&mut app.machine);

    app.open_dropped(Handed::Path(printer_file.clone()));
    assert!(
        app.machine.slicing.printer_id.is_some(),
        "the dropped machine is printed on"
    );
    assert!(app.doc.scene.is_empty(), "nothing went on the plate");

    let resin = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../printer-profiles/tests/fixtures/saturn-4-ultra.cfgx");
    app.open_dropped(Handed::Path(resin));
    assert!(
        app.machine.slicing.resin_id.is_some(),
        "the dropped resin is printed with"
    );

    let _ = std::fs::remove_file(&printer_file);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A machine the user removed is still in the library, and a run must not open on it.
#[test]
fn a_remembered_machine_that_is_no_longer_the_users_is_not_restored() {
    let read: crate::prefs::Preferences =
        serde_json::from_str(r#"{"printer":"elegoo-saturn-4-ultra","resin":"gone"}"#)
            .expect("every field of the file is optional");
    let mut app = SlicerApp::default();
    read.apply(&mut app.window());
    assert!(app.machine.slicing.printer_id.is_none());
    assert!(app.machine.slicing.resin_id.is_none());
}

/// Notices of different widths stand on one middle, the room between the side cards.
#[test]
fn every_notice_at_the_stages_foot_is_centred_on_one_line() {
    use egui_kittest::kittest::Queryable as _;

    let mut app = SlicerApp::default();
    app.view.options.started = true;
    app.machine.status = Status::Info("Opened /a/rather/long/path/to/Stand.obj".to_owned());
    let mut dressed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut SlicerApp| {
                if dressed {
                    app.window().show(ui);
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            app,
        );
    harness.run_steps(3);
    let unready = harness.get_by_label("No printer or resin yet").rect();
    let message = harness
        .get_by_label("Opened /a/rather/long/path/to/Stand.obj")
        .rect();
    let set_up = harness.get_by_label("Set up").rect();
    let unready_middle = (unready.left() + set_up.right()) / 2.0;
    assert!(
        (unready_middle - message.center().x).abs() < 16.0,
        "the notice centred at {unready_middle}, the message at {}",
        message.center().x
    );
}

/// A message with its figures longer than the room wraps inside it rather than running
/// under the view tools beside it.
#[test]
fn a_long_message_wraps_inside_the_room_between_the_side_cards() {
    use crate::status::{Change, Reading};
    use egui_kittest::kittest::Queryable as _;

    let mut app = SlicerApp::default();
    app.view.options.started = true;
    let figure = |text: &str| Reading {
        text: text.to_owned(),
        change: Change::Better,
    };
    let message = "1 already stands the way it prints best; 1 stands on the plate";
    app.machine.status = Status::Readings(
        message.to_owned(),
        vec![figure("0.04 ml of supports"), figure("4.8 N of peel")],
    );
    let mut dressed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1100.0, 800.0))
        .build_ui_state(
            move |ui, app: &mut SlicerApp| {
                if dressed {
                    app.window().show(ui);
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            app,
        );
    harness.run_steps(3);
    let tools = harness.get_by_label("Plate grid").rect();
    let last = harness.get_by_label("4.8 N of peel").rect();
    assert!(
        last.right() < tools.left(),
        "the message ends at {}, the view tools start at {}",
        last.right(),
        tools.left()
    );
}

/// A message under the pointer is being read, so it outlasts its moment, and goes once
/// the pointer has left it for as long again.
#[test]
fn a_message_under_the_pointer_stays_until_it_is_left() {
    use egui_kittest::kittest::Queryable as _;

    let mut app = SlicerApp::default();
    app.view.options.started = true;
    app.machine.status = Status::Info("Hollowed 1 model(s), saving 70.9 ml".to_owned());
    let mut dressed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .with_step_dt(0.5)
        .build_ui_state(
            move |ui, app: &mut SlicerApp| {
                if dressed {
                    app.window().show(ui);
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            app,
        );
    harness.run_steps(2);
    let at = harness
        .get_by_label("Hollowed 1 model(s), saving 70.9 ml")
        .rect()
        .center();
    harness.hover_at(at);
    harness.run_steps(16);
    assert!(
        harness
            .query_by_label("Hollowed 1 model(s), saving 70.9 ml")
            .is_some(),
        "eight seconds under the pointer"
    );

    harness.hover_at(egui::pos2(640.0, 300.0));
    harness.run_steps(12);
    assert!(
        harness
            .query_by_label("Hollowed 1 model(s), saving 70.9 ml")
            .is_none()
    );
}

/// A notice is as thin as its lines: the figures sit right under the message.
#[test]
fn a_message_and_its_figures_stand_close_together() {
    use crate::status::{Change, Reading};
    use egui_kittest::kittest::Queryable as _;

    let mut app = SlicerApp::default();
    app.view.options.started = true;
    let reading = Reading {
        text: "0.06 ml of supports".to_owned(),
        change: Change::Same,
    };
    app.machine.status = Status::Readings("Everything already stands".to_owned(), vec![reading]);
    let mut dressed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut SlicerApp| {
                if dressed {
                    app.window().show(ui);
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    dressed = true;
                }
            },
            app,
        );
    harness.run_steps(3);
    let title = harness.get_by_label("Everything already stands").rect();
    let figures = harness.get_by_label("0.06 ml of supports").rect();
    let gap = figures.top() - title.bottom();
    assert!(
        (0.0..=4.0).contains(&gap),
        "{gap} points between the two lines"
    );
}

/// The Settings window holds the keyboard, and Escape takes down the question over it
/// before the window itself.
#[test]
fn escape_closes_the_question_and_then_the_settings_window() {
    let mut app = SlicerApp::default();
    shortcuts::act(&mut app.window(), shortcuts::Action::Settings);
    assert!(app.machine.settings.open);

    let deleting = crate::settings::Deleting::Profile(printer_profiles::Kind::Support, "x".into());
    app.machine.settings.confirm = Some(deleting);
    shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
    assert!(app.machine.settings.confirm.is_none());
    assert!(app.machine.settings.open, "the window is still there");

    shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
    assert!(!app.machine.settings.open);
}

/// The Machine and resin window holds the keyboard, and Escape takes down the question
/// over it before the window itself.
#[test]
fn escape_closes_the_question_and_then_the_machine_and_resin_window() {
    let mut app = SlicerApp::default();
    crate::panels::open_machines(&mut app.machine);
    assert!(app.machine.settings.machines);

    app.machine.settings.confirm = Some(crate::settings::Deleting::Resin("grey".to_owned()));
    shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
    assert!(app.machine.settings.confirm.is_none());
    assert!(app.machine.settings.machines, "the window is still there");

    shortcuts::act(&mut app.window(), shortcuts::Action::SelectAll);
    assert!(
        app.machine.settings.machines,
        "a key meant for the plate is held"
    );

    shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
    assert!(!app.machine.settings.machines);
}

#[test]
fn a_switch_back_to_prepare_without_a_stack_keeps_its_own_height() {
    let mut app = SlicerApp::default();
    app.view.section.height_mm = Some(3.5);
    app.mode = Mode::Preview;
    app.carry_height();
    app.mode = Mode::Prepare;
    app.carry_height();
    assert_eq!(app.view.section.height_mm, Some(3.5));
}

/// Check and Export read their figures off the stack, so opening either cuts it
/// without the user having to look at a layer first.
#[test]
fn opening_check_or_export_cuts_the_stack_in_the_model_view() {
    for tool in [Tool::Check, Tool::Export] {
        let fixture =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.stl");
        let mut app = SlicerApp::default();
        let profile = app
            .machine
            .slicing
            .catalogue
            .printer("elegoo-saturn-4-ultra")
            .expect("the printer ships")
            .profile
            .clone();
        let id = Some("elegoo-saturn-4-ultra".to_owned());
        crate::profiles::apply_printer(&mut app.window(), profile, id);
        app.open_model(Handed::Path(fixture));
        while app.doc.imports.poll(
            &mut app.doc.scene,
            &app.doc.plate,
            &mut app.view.camera,
            &mut app.machine.status,
        ) {
            std::thread::yield_now();
        }

        app.refresh_preview();
        assert!(
            !app.machine.preview.is_building(),
            "the model view alone cuts nothing"
        );
        app.tool = tool;
        app.refresh_preview();
        assert!(
            app.machine.preview.is_building(),
            "{} cuts the stack",
            tool.label()
        );
        assert_eq!(app.mode, Mode::Prepare, "and the view stays on the model");
    }
}

#[test]
fn the_preview_is_not_built_without_a_printer_profile() {
    let mut app = SlicerApp {
        mode: Mode::Preview,
        ..SlicerApp::default()
    };
    app.refresh_preview();
    assert!(!app.machine.preview.is_building());
    assert!(
        !app.machine.status.is_error(),
        "an empty plate is not a failure"
    );
}

const IMPORTED_ON: &str = "elegoo-saturn-4-ultra";

/// The app over a profile directory of the test's own, which every import lands in.
fn with_profile_dir(name: &str) -> SlicerApp {
    let dir = std::env::temp_dir().join(format!("encrust-profiles-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = SlicerApp::default();
    app.machine.slicing.catalogue =
        printer_profiles::Catalogue::with_root(&dir).expect("a missing directory reads as empty");
    app
}

fn handed<T>(name: &str, text: Result<String, T>) -> Handed
where
    T: std::fmt::Debug,
{
    let text = text.expect("the profile serialises");
    Handed::bytes(name, std::sync::Arc::from(text.into_bytes()))
}

#[test]
fn a_machine_from_the_file_menu_is_kept_and_put_under_the_plate() {
    let mut app = with_profile_dir("printer");
    let catalogue = &app.machine.slicing.catalogue;
    let file = handed(
        "saturn.toml",
        catalogue.export(printer_profiles::Kind::Printer, IMPORTED_ON),
    );

    crate::profiles::import_printer(&mut app.window(), &file);
    let id = app
        .machine
        .slicing
        .printer_id
        .clone()
        .expect("a machine in hand");
    let entry = app.machine.slicing.catalogue.printer(&id).expect("kept");
    assert!(
        crate::settings::installed(&entry.source),
        "one of the user's machines, with an id to remember"
    );
}

#[test]
fn a_resin_from_the_file_menu_goes_on_the_machine_in_hand() {
    let mut app = with_profile_dir("resin");
    let profile = app
        .machine
        .slicing
        .catalogue
        .printer(IMPORTED_ON)
        .expect("ships")
        .profile
        .clone();
    crate::profiles::apply_printer(&mut app.window(), profile, Some(IMPORTED_ON.to_owned()));
    let resin = printer_profiles::MaterialProfile {
        name: "Bench grey".to_owned(),
        ..printer_profiles::MaterialProfile::default()
    };
    let file = handed(
        "grey.toml",
        resin.to_toml_string(std::path::Path::new("grey")),
    );

    crate::profiles::import_material(&mut app.machine, &file);
    let id = app
        .machine
        .slicing
        .resin_id
        .clone()
        .expect("a resin in hand");
    let kept = &app
        .machine
        .slicing
        .catalogue
        .resin(&id)
        .expect("kept")
        .profile;
    assert!(kept.is_tuned_for(IMPORTED_ON));
}

#[test]
fn a_sliced_file_handed_in_as_a_resin_goes_on_the_machine_in_hand() {
    let mut app = with_profile_dir("sliced-resin");
    app.machine.slicing.printer_id = Some(IMPORTED_ON.to_owned());
    let path = crate::preview::tests::written_goo("sliced-resin", 3);

    crate::profiles::import_material(&mut app.machine, &Handed::Path(path));
    let id = app
        .machine
        .slicing
        .resin_id
        .clone()
        .expect("the file's resin is in hand");
    let kept = &app
        .machine
        .slicing
        .catalogue
        .resin(&id)
        .expect("kept")
        .profile;
    assert!(kept.is_tuned_for(IMPORTED_ON));
    assert_eq!(kept.for_printer(IMPORTED_ON).layer_height_mm, 0.05);
}

#[test]
fn a_sliced_file_for_no_known_machine_waits_for_one_in_hand() {
    let mut app = with_profile_dir("sliced-resin-unknown");
    app.machine.slicing.printer_id = None;
    let path = crate::preview::tests::written_goo("sliced-resin-unknown", 3);

    crate::profiles::import_material(&mut app.machine, &Handed::Path(path));
    assert!(matches!(app.machine.status, Status::Error(_)));
    assert_eq!(app.machine.slicing.catalogue.resins().count(), 0);
}

#[test]
fn a_resin_another_program_exported_goes_on_the_machine_it_names() {
    let mut app = with_profile_dir("foreign-resin");
    app.machine.slicing.printer_id = None;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../printer-profiles/tests/fixtures/saturn-4-ultra.cfgx");

    crate::profiles::import_material(&mut app.machine, &Handed::Path(path));
    let id = app
        .machine
        .slicing
        .resin_id
        .clone()
        .expect("the exported resin is in hand");
    let kept = &app
        .machine
        .slicing
        .catalogue
        .resin(&id)
        .expect("kept")
        .profile;
    assert!(kept.is_tuned_for(IMPORTED_ON));
    assert_eq!(kept.for_printer(IMPORTED_ON).light_pwm, 211);
}
