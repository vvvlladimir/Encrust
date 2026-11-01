//! Pictures of the window's panels, compared with the ones in `tests/snapshots/`. The
//! viewport draws nothing here: its GPU resources are never installed, so these pin the
//! chrome alone. Run by hand, see docs/decisions/0215.

use std::path::PathBuf;

use core_volume::InfillPattern;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotResults};
use printer_profiles::{Bracing, BracingPreset};

use super::SlicerApp;
use super::states::{SETTLE_FRAMES, app, cut, drawn, with_machines, with_nothing};
use crate::workspace::{Mode, Tool};

mod network;

use network::{board, listed, on_the_network};

/// The size the window opens at, `lib::viewport`.
const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// Draws `app` as the window would and compares it with the picture called `name`. The
/// results fail the test when dropped, so a test of several pictures gathers them first.
fn picture(name: &str, app: SlicerApp) -> SnapshotResults {
    let mut harness = drawn(Harness::builder().with_size(WINDOW).wgpu(), app);
    harness.snapshot(name);
    harness.take_snapshot_results()
}

/// The window as the mockup draws it: a project of two plates open, a cube picked on the
/// first, a priced resin, and Bench B ready to take the plate.
fn pictured() -> SlicerApp {
    let mut app = listed(app(true), &[("Bench B", 140, "bb")]);
    app.doc.project.path = Some(PathBuf::from("/plates/Bench project.encrust"));
    let scene = &mut app.doc.scene;
    let cube = scene.selection().first().copied();
    scene.add_plate();
    scene.show_plate(0);
    scene.select(cube);
    app.machine.slicing.material.details.price = 35.0;
    app
}

#[test]
fn the_start_page() {
    let mut app = app(false);
    let day_s = 86_400;
    let now_s = crate::updates::now_s();
    for (name, ago_s) in [
        ("Calibration.goo", 8 * day_s),
        ("Chess rook.3mf", 7 * day_s),
        ("Hex tray.stl", 2 * day_s + 3_600),
        ("Dragon bust.encrust", day_s + 7_200),
        ("Bench.encrust", day_s + 3_600),
    ] {
        app.machine
            .recent
            .note(&PathBuf::from("/plates").join(name), now_s - ago_s);
    }
    picture("start", app);
}

#[test]
fn an_empty_plate() {
    let mut app = app(false);
    app.view.options.started = true;
    picture("empty_plate", app);
}

#[test]
fn a_picked_model() {
    picture("picked_model", pictured());
}

#[test]
fn every_tool_on_a_picked_model() {
    let mut results = SnapshotResults::new();
    for tool in Tool::ALL.into_iter().filter(|tool| *tool != Tool::Check) {
        let mut app = pictured();
        app.tool = tool;
        if tool == Tool::Measure {
            across_the_top(&mut app.tools.measure);
        }
        results.extend(picture(&format!("tool_{tool:?}").to_lowercase(), app));
    }
}

/// A span across the top of the pictured cube, corner to opposite corner: 28.28 mm.
fn across_the_top(measure: &mut crate::measure::Measure) {
    use crate::measure::Snapped;
    use core_geometry::Vec3;

    measure.pick(Vec3::new(99.44, 51.44, 20.0), Snapped::Corner);
    measure.pick(Vec3::new(119.44, 71.44, 20.0), Snapped::Corner);
}

/// Check over a cube with a block beside it 10 mm up on nothing: that block an island.
#[test]
fn the_check_finding_an_island() {
    use core_geometry::{Vec3, cuboid};

    let mut app = pictured();
    app.tool = Tool::Check;
    let mut two = cuboid(Vec3::ZERO, Vec3::splat(20.0));
    two.append(&cuboid(
        Vec3::new(22.0, 0.0, 10.0),
        Vec3::new(26.0, 4.0, 12.0),
    ));
    let shaped = crate::scene::Shaped::of(std::sync::Arc::new(two));
    app.doc.scene.objects_mut()[0].reshape(shaped);
    picture("tool_check", cut(app));
}

#[test]
fn the_supports_tool_sizing_automatically() {
    let mut app = pictured();
    app.tool = Tool::Supports;
    let catalogue = app.machine.slicing.catalogue.clone();
    app.tools.supports.set_automatic(true, &catalogue);
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_supports_auto", app));
}

/// Tall enough for the Shape tool to show the bracing's drawing and every number under it.
const TALL_WINDOW: egui::Vec2 = egui::vec2(1280.0, 1200.0);

#[test]
fn the_shape_tool_on_bracing_from_a_preset_and_by_the_numbers() {
    let mut results = SnapshotResults::new();
    let own = Bracing {
        rise_mm: 6.0,
        ..BracingPreset::Tall.bracing()
    };
    for (name, bracing) in [
        ("tool_shape_brace_preset", BracingPreset::Standard.bracing()),
        ("tool_shape_brace_numbers", own),
    ] {
        let mut app = pictured();
        app.tool = Tool::Shape;
        app.tools.supports.profile.bracing = bracing;
        let mut harness = drawn(Harness::builder().with_size(TALL_WINDOW).wgpu(), app);
        harness.get_by_label("Brace").click();
        harness.run_steps(SETTLE_FRAMES);
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}

#[test]
fn the_hollow_tool_with_each_kind_of_infill() {
    let mut results = SnapshotResults::new();
    for (name, pattern) in [
        ("tool_hollow_hive", InfillPattern::Hive),
        ("tool_hollow_scaffold", InfillPattern::Scaffold),
    ] {
        let mut app = pictured();
        app.tool = Tool::Hollow;
        app.tools.hollow.state.infill_on = true;
        app.tools.hollow.state.infill.pattern = pattern;
        results.extend(picture(name, app));
    }
}

#[test]
fn the_repair_tool_wrapping() {
    let mut app = pictured();
    app.tool = Tool::Repair;
    app.tools.repair.state.method = core_volume::RepairMethod::Wrap;
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_repair_wrap", app));
}

#[test]
fn the_repair_tool_picking_faces_and_edges_by_hand() {
    let mut results = SnapshotResults::new();
    for (name, picking) in [
        ("tool_repair_faces", crate::repair::Picking::Faces),
        ("tool_repair_edges", crate::repair::Picking::Edges),
    ] {
        let mut app = pictured();
        app.tool = Tool::Repair;
        app.tools.repair.mode = crate::repair::RepairMode::Faces;
        app.tools.repair.picking = picking;
        results.extend(picture(name, app));
    }
}

#[test]
fn the_repair_tool_listing_two_shells_one_picked() {
    let mut app = pictured();
    app.tool = Tool::Repair;
    let mut two =
        core_geometry::cuboid(core_geometry::Vec3::ZERO, core_geometry::Vec3::splat(20.0));
    let crumb = (
        core_geometry::Vec3::splat(25.0),
        core_geometry::Vec3::splat(26.0),
    );
    two.append(&core_geometry::cuboid(crumb.0, crumb.1));
    let shaped = crate::scene::Shaped::of(std::sync::Arc::new(two));
    app.doc.scene.objects_mut()[0].reshape(shaped);
    let object = &app.doc.scene.objects()[0];
    let shells = &mut app.doc.repairs.shells;
    shells.list(object);
    while shells.poll(&app.doc.scene) {
        std::thread::yield_now();
    }
    shells.toggle(object, 1);
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_repair_shells", app));
}

#[test]
fn the_repair_tool_marking_what_it_found() {
    let mut app = pictured();
    app.tool = Tool::Repair;
    let found = &mut app.doc.scene.objects_mut()[0].summary.diagnostics;
    (found.holes, found.boundary_edges, found.duplicate_faces) = (2, 12, 4);
    let scene = &app.doc.scene;
    app.doc
        .repairs
        .found
        .press(scene, crate::repair::Finding::OpenEdges);
    let mut results = SnapshotResults::new();
    results.extend(picture("tool_repair_found", app));
}

#[test]
fn the_relief_tool_in_each_mode() {
    let mut results = SnapshotResults::new();
    for (name, mode) in [
        ("tool_relief_text", crate::relief::ReliefMode::Text),
        ("tool_relief_combine", crate::relief::ReliefMode::Combine),
    ] {
        let mut app = pictured();
        app.tool = Tool::Relief;
        app.tools.relief.mode = mode;
        results.extend(picture(name, app));
    }
    let mut app = pictured();
    app.tool = Tool::Relief;
    crate::relief::choose_builtin(&mut app.tools.relief, &mut app.machine.status, 2);
    results.extend(picture("tool_relief_picture", app));
}

#[test]
fn the_cut_tool_joining_its_halves() {
    let mut results = SnapshotResults::new();
    for (name, kind) in [
        ("tool_cut_dowels", core_joint::JointKind::Dowels),
        ("tool_cut_key", core_joint::JointKind::Key),
    ] {
        let mut app = pictured();
        app.tool = Tool::Cut;
        app.tools.cut.state.joints.kind = Some(kind);
        results.extend(picture(name, app));
    }
}

#[test]
fn the_preview_of_a_cut_cube() {
    let mut app = pictured();
    app.mode = Mode::Preview;
    let mut app = cut(app);
    app.machine.preview.set_layer(211);
    picture("preview", app);
}

/// The app with the Settings window open, on the first support profile.
fn with_settings() -> SlicerApp {
    let mut app = app(false);
    app.view.options.started = true;
    crate::panels::toggle_settings(&mut app.machine);
    app
}

#[test]
fn the_settings_window() {
    let mut results = SnapshotResults::new();
    results.extend(picture("settings", with_settings()));
    let mut form = with_settings();
    let settings = &mut form.machine.settings;
    settings.pick_support(&form.machine.slicing.catalogue, "heavy");
    settings.editing_support = true;
    results.extend(picture("settings_support_form", form));
    let mut auto = with_settings();
    let settings = &mut auto.machine.settings;
    settings.pick_auto(&auto.machine.slicing.catalogue, "default");
    settings.editing_support = true;
    results.extend(picture("settings_auto_form", auto));
    // Not General: it names the profile folder, which is a different path on every desk.
    let mut hollow = with_settings();
    hollow
        .machine
        .settings
        .turn_to(crate::settings::Section::Hollow);
    results.extend(picture("settings_hollow", hollow));
}

#[test]
fn the_sheet_of_keys() {
    let mut app = app(false);
    app.view.options.started = true;
    app.view.options.sheet = true;
    picture("sheet", app);
}

/// Find over a picked model, nothing typed: every tool, action and model by group.
#[test]
fn the_find_palette() {
    let mut app = pictured();
    app.view.options.find = true;
    picture("find", app);
}

#[test]
fn the_machine_and_resin_window() {
    let mut results = SnapshotResults::new();
    results.extend(picture(
        "machines_resins",
        with_machines(app(false), "resins"),
    ));
    let mut machine = with_machines(app(false), "machine");
    machine.machine.settings.tab = crate::settings::Tab::Machine;
    results.extend(picture("machines_machine", machine));
    results.extend(picture("machines_none", with_nothing(app(false), false)));
    results.extend(picture("machines_no_resin", with_nothing(app(false), true)));
}

/// This plate's card with two boards on the network: a profile listing one names it on a
/// plain row, one listing both offers the other, an idle one puts Slice and send over the
/// press and Start print once the file landed, and the ring opens the first's sheet.
#[test]
fn the_printers_on_the_network() {
    let (bench_a, bench_b) = (("Bench A", 133, "aa"), ("Bench B", 140, "bb"));
    let mut results = SnapshotResults::new();
    results.extend(picture("plate_send_bound", on_the_network(&[bench_a])));
    results.extend(picture(
        "plate_send_list",
        on_the_network(&[bench_a, bench_b]),
    ));
    results.extend(picture("plate_send_idle", on_the_network(&[bench_b])));
    let mut landed = on_the_network(&[bench_b]);
    landed.machine.network.sent = Some(crate::network::Sent {
        printer: "Bench B".to_owned(),
        filename: "Cube.goo".to_owned(),
    });
    results.extend(picture("plate_send_landed", landed));
    let mut sheet = on_the_network(&[bench_a]);
    sheet.machine.network.open = Some("sdcp:aa".to_owned());
    results.extend(picture("printer_sheet", sheet));
}

/// A preview being cut, a file waiting on a printer, a model that came in broken, and
/// what happened last, stacked at the foot of the stage.
#[test]
fn the_notices_at_the_foot_of_the_stage() {
    let mut app = pictured();
    let cube = app.doc.scene.objects_mut().first_mut();
    cube.expect("the pictured plate holds a cube")
        .summary
        .diagnostics
        .boundary_edges = 3;
    app.machine.network.sent = Some(crate::network::Sent {
        printer: "Workshop".to_owned(),
        filename: "Dragon bust.goo".to_owned(),
    });
    app.machine.status = crate::status::Status::Info("Dragon bust.goo is on Workshop".to_owned());
    // Never polled, so the cut stays under way while the window is drawn.
    let cutting = app.machine.slicing.cutting();
    app.machine
        .preview
        .build(&app.doc.scene, cutting)
        .expect("the plate holds a cube");
    app.machine.slicing.resin_id = None;
    picture("notices", app);
}

/// The printer's Network tab with two boards of its model listed and named, and a third
/// found on the network still to tick.
#[test]
fn the_printers_a_profile_sends_to() {
    let mut app = with_machines(app(false), "network");
    app.machine.settings.tab = crate::settings::Tab::Network;
    app.machine.network.printers = vec![
        board("3D Printer", "aa", 133),
        board("3D Printer", "bb", 140),
        board("3D Printer", "cc", 151),
    ];
    if let Some(draft) = app.machine.settings.printer.as_mut() {
        draft.values.connection = printer_profiles::Connection::Sdcp;
        draft.values.machines = vec![
            printer_profiles::NetworkMachine {
                name: "Bench A".to_owned(),
                address: "192.168.1.133".to_owned(),
                board: Some("aa".to_owned()),
            },
            printer_profiles::NetworkMachine {
                name: "Bench B".to_owned(),
                address: "192.168.1.140".to_owned(),
                board: Some("bb".to_owned()),
            },
        ];
    }
    picture("machines_network", app);
}

#[test]
fn the_chip_opening_its_machines_and_its_resins() {
    let mut results = SnapshotResults::new();
    for (name, part) in [("chip_machines", "Machine, "), ("chip_resins", "Resin, ")] {
        let mut harness = drawn(Harness::builder().with_size(WINDOW).wgpu(), pictured());
        harness.get_by_label_contains(part).click();
        harness.run_steps(SETTLE_FRAMES);
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}
