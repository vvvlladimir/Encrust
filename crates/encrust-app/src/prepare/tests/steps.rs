//! The steps that work on the plate as a whole: check and its retry, arrange, save, pause.

use std::path::Path;

use printer_profiles::{MaterialProfile, PrinterProfile};

use super::recipes::plan;
use super::*;

/// A panel coarse enough to cut and measure in a moment, wide enough for the large cube.
const PANEL: &str = r#"
name = "Test panel"
manufacturer = "Test"

[display]
width_px = 240
height_px = 240
width_mm = 120.0
height_mm = 120.0

[build_volume]
x = 120.0
y = 120.0
z = 100.0
"#;

impl Bench {
    fn with_printer(mut self) -> Self {
        let printer = PrinterProfile::from_toml_str(PANEL, Path::new("inline.toml"))
            .expect("the inline profile is valid");
        let slicing = &mut self.machine.slicing;
        slicing.set_printer(printer, None);
        // Nothing ships an exposure, so a cut needs a resin in hand as much as a panel.
        slicing.set_material(MaterialProfile {
            layer_height_mm: LAYER_MM,
            ..MaterialProfile::default()
        });
        self
    }

    fn run(&mut self, steps: &str) {
        let (doc, tools, machine) = (&mut self.doc, &mut self.tools, &mut self.machine);
        start_with(plan(steps), doc, tools, machine).expect("a model to prepare");
    }

    fn cancel(&mut self) {
        cancel(&mut self.doc, &mut self.tools, &mut self.machine);
    }
}

/// A 10 mm cube held 5 mm over the plate on nothing, beside a 2 mm peg that stands on it so
/// that the stack starts at the plate: the cube's first layer is an island.
fn floating() -> (Bench, ObjectId) {
    let (bench, id) = Bench::with(cuboid(Vec3::ZERO, Vec3::splat(10.0)));
    let mut bench = bench.with_printer();
    let object = bench.doc.scene.get_mut(id).expect("on the plate");
    object.transform.translation.z = 5.0;
    let peg = cuboid(Vec3::new(40.0, 0.0, 0.0), Vec3::new(42.0, 2.0, 2.0));
    let summary = ImportSummary::made(diagnose(&peg));
    let peg = Imported::new(
        "peg".to_owned(),
        Arc::new(peg),
        Transform::default(),
        summary,
    );
    bench.doc.scene.insert(peg);
    bench.doc.scene.select(None);
    bench.frame();
    (bench, id)
}

#[test]
fn a_check_counts_the_islands_a_gate_after_it_stops_on() {
    let (mut bench, id) = floating();
    let before = standing(bench.object(id));
    bench.run(
        "[[step]]\ndo = \"check\"\n\
         [[step]]\ndo = \"gate\"\nstop_if = \"islands > 0\"\nmessage = \"islands\"\n",
    );
    bench.run_to_end();
    assert_eq!(bench.waits(), Some("islands"));
    assert_eq!(standing(bench.object(id)), before, "as before Prepare");
}

#[test]
fn a_check_on_a_model_standing_on_the_plate_finds_no_island() {
    let (bench, _) = Bench::with(cuboid(Vec3::ZERO, Vec3::splat(10.0)));
    let mut bench = bench.with_printer();
    bench.run(
        "[[step]]\ndo = \"check\"\n\
         [[step]]\ndo = \"gate\"\nstop_if = \"islands > 0 || support_ml > 0\"\n",
    );
    bench.run_to_end();
    let said = Status::Info(tr!("prepare-done", count = 1));
    assert_eq!(bench.machine.status, said);
}

#[test]
fn a_retry_supports_the_islands_again_at_most_max_times() {
    let (mut bench, id) = floating();
    bench.run("[[step]]\ndo = \"check\"\nretry = { while = \"islands >= 0\", max = 2 }\n");
    let mut most = 0;
    while bench.frame() || bench.doc.drilling.is_running() || bench.doc.surfaces.is_running() {
        most = most.max(bench.tools.prepare.attempt().unwrap_or(0));
        std::thread::yield_now();
    }
    assert_eq!(most, 2, "measured again after each of the two supportings");
    let said = Status::Info(tr!("prepare-done", count = 2));
    assert_eq!(bench.machine.status, said, "and then it ends");
    assert!(
        !bench.object(id).supports.is_empty(),
        "the island stands on a support"
    );
}

#[test]
fn a_retry_with_no_supports_step_stands_islands_on_the_automatic_profile_not_the_panels() {
    let (mut bench, _) = floating();
    bench.tools.supports.profile.name = "the panel's own".to_owned();
    bench.run("[[step]]\ndo = \"check\"\nretry = { while = \"islands >= 0\", max = 1 }\n");
    bench.run_to_end();
    let groups = &bench.tools.supports.groups;
    let islands = groups
        .iter()
        .find(|group| group.name == crate::supports::ISLANDS_GROUP);
    let islands = islands.expect("the island was supported in its own group");
    assert_ne!(
        islands.profile.name, "the panel's own",
        "the run builds to its own profile, `auto` when no supports step ran"
    );
}

#[test]
fn arrange_moves_models_standing_on_each_other_apart() {
    let (mut bench, one) = Bench::with(large_cube());
    let summary = ImportSummary::made(diagnose(&large_cube()));
    let model = Imported::new(
        "other".to_owned(),
        Arc::new(large_cube()),
        Transform::default(),
        summary,
    );
    let other = bench.doc.scene.insert(model);
    bench.doc.scene.select(None);
    bench.frame();
    bench.run("[[step]]\ndo = \"arrange\"\nset = { spacing_mm = 5.0 }\n");
    bench.run_to_end();

    let bounds = |id| bench.object(id).world_bounds().expect("a mesh");
    let (one, other) = (bounds(one), bounds(other));
    let gap_x = (other.mins.x - one.maxs.x).max(one.mins.x - other.maxs.x);
    let gap_y = (other.mins.y - one.maxs.y).max(one.mins.y - other.maxs.y);
    let gap_mm = gap_x.max(gap_y);
    assert!(gap_mm >= 4.0, "5 mm apart, give a cell; got {gap_mm}");
}

#[test]
fn save_writes_the_plate_into_the_folder_by_the_template() {
    let (bench, id) = Bench::with(cuboid(Vec3::ZERO, Vec3::splat(10.0)));
    let mut bench = bench.with_printer();
    let folder = std::env::temp_dir().join(format!("encrust-prepare-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a temporary folder");
    let before = standing(bench.object(id));
    let folder_text = folder.display().to_string().replace('\\', "/");
    bench.run(&format!(
        "[[step]]\ndo = \"save\"\nset = {{ folder = \"{folder_text}\", name = \"{{model}}-saved\" }}\n"
    ));
    bench.run_to_end();

    let extension = bench.machine.slicing.format.extension();
    let path = folder.join(format!("model-saved.{extension}"));
    let written = path.exists();
    std::fs::remove_dir_all(&folder).ok();
    assert!(
        written,
        "{} written; said {:?}",
        path.display(),
        bench.machine.status
    );
    assert_eq!(standing(bench.object(id)), before, "the plate unchanged");
    assert!(!bench.doc.history.is_held(), "the run ended");
}

#[test]
fn save_to_send_with_no_printer_to_send_to_waits_saying_why() {
    let (bench, _) = Bench::with(cuboid(Vec3::ZERO, Vec3::splat(10.0)));
    let mut bench = bench.with_printer();
    bench.run("[[step]]\ndo = \"save\"\nset = { send = true }\n");
    bench.run_to_end();
    let said = tr!("prepare-save-no-printer-to-send");
    assert_eq!(bench.waits(), Some(said.as_str()));
    assert!(bench.machine.slicing.job.is_none(), "nothing is sliced");
}

const HOLLOW_PAUSE_DRAIN: &str = "[[step]]\ndo = \"hollow\"\n\
    [[step]]\ndo = \"pause\"\nmessage = \"Look at the wall\"\n\
    [[step]]\ndo = \"drain\"\nset = { at = \"points\", points = [[0.5, 0.5, 0.0]] }\n";

#[test]
fn a_pause_waits_for_continue_with_the_plate_free() {
    let (mut bench, id) = Bench::with(large_cube());
    bench.run(HOLLOW_PAUSE_DRAIN);
    bench.run_to_end();
    let wait = bench.tools.prepare.waiting().expect("waits at the pause");
    assert_eq!(wait.why, "Look at the wall");
    assert!(!wait.failed);
    assert!(bench.object(id).hollow.is_hollow(), "hollowed before it");
    assert!(
        bench.object(id).hollow.drains().is_empty(),
        "not drained yet"
    );
    assert!(holding(&bench.tools).is_none(), "the tools are the user's");

    bench.resume();
    bench.run_to_end();
    assert!(!bench.tools.prepare.is_running());
    assert_eq!(bench.object(id).hollow.drains().len(), 1, "drained after");
}

#[test]
fn ending_a_pause_keeps_the_plate_as_it_stands() {
    let (mut bench, id) = Bench::with(large_cube());
    bench.run(HOLLOW_PAUSE_DRAIN);
    bench.run_to_end();
    assert!(bench.tools.prepare.waiting().is_some());
    bench.cancel();
    bench.run_to_end();
    assert!(!bench.tools.prepare.is_running());
    assert!(bench.object(id).hollow.is_hollow(), "kept");
    assert!(bench.object(id).hollow.drains().is_empty(), "not drained");
}

#[test]
fn a_pause_stops_even_with_every_step_before_it_skipped() {
    let (mut bench, _) = Bench::with(large_cube());
    let steps = "[[step]]\ndo = \"hollow\"\nwhen = \"hollow\"\n\
        [[step]]\ndo = \"pause\"\n\
        [[step]]\ndo = \"drain\"\nset = { at = \"points\", points = [[0.5, 0.5, 0.0]] }\n";
    bench.run(steps);
    bench.run_to_end();
    let wait = bench.tools.prepare.waiting().expect("the recipe's pause");
    assert_eq!(
        wait.step, 2,
        "the hollow step skipped, the model being solid"
    );
    bench.resume();
    bench.run_to_end();
    assert!(!bench.tools.prepare.is_running());
}

#[test]
fn an_arrange_step_names_itself_on_the_bar() {
    let (mut bench, _) = Bench::with(large_cube());
    bench.run("[[step]]\ndo = \"arrange\"\n[[step]]\ndo = \"hollow\"\n");
    let (step, _) = bench.tools.prepare.progress().expect("running");
    assert_eq!(step, Step::Arrange, "arranged in the frame it started");
}
