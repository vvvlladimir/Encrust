//! Pictures of each tool's panel on a picked model.

use core_volume::InfillPattern;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotResults};
use printer_profiles::{Bracing, BracingPreset};

use super::super::states::{SETTLE_FRAMES, cut, drawn};
use super::{WINDOW, picture, pictured};
use crate::workspace::{Mode, Tool};

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

#[test]
fn the_supports_tool_opening_its_groups() {
    let mut app = pictured();
    app.tool = Tool::Supports;
    let catalogue = app.machine.slicing.catalogue.clone();
    app.tools.supports.set_automatic(true, &catalogue);
    let mut harness = drawn(Harness::builder().with_size(WINDOW).wgpu(), app);
    let pickers = harness.query_all_by_role(egui::accesskit::Role::ComboBox);
    let left = pickers
        .min_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
        .expect("the supports card holds the group picker");
    left.click();
    harness.run_steps(SETTLE_FRAMES);
    harness.snapshot("tool_supports_groups");
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
