//! Screenshots for the website: the whole window, viewport and all, on the Crustling, at
//! twice the size the window opens at. Unlike `snapshots`, these install the viewport's GPU
//! resources, so the scene is drawn, and they compare nothing: each run writes fresh PNGs for
//! the site to pick up (`pnpm sync:screens` there). Run by hand on a machine with a GPU:
//!
//! ```sh
//! cargo test -p encrust-app --features showcase --lib showcase -- --test-threads=1
//! ```
//!
//! The pictures land in `target/showcase/`, or in `ENCRUST_SHOWCASE_DIR` when it is set.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui_kittest::Harness;

use super::SlicerApp;
use super::states::app;
use crate::files::Handed;
use crate::i18n::Language;
use crate::prefs::Appearance;
use crate::scene::SceneObject;
use crate::ui::theme;
use crate::workspace::Tool;

/// The size the window opens at, `lib::viewport`.
const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// Physical pixels per point: a Retina capture, which the site scales down.
const SCALE: f32 = 2.0;

/// Frames a background job gets before the picture is given up on.
const MOST_FRAMES: usize = 6_000;

/// Quiet frames after the last job lands, for folds, fades and the camera to come to rest.
const QUIET_FRAMES: usize = 40;

/// Angles that show the model's depth: a turn off straight-on and how far the camera looks
/// down. Each picture takes one by its name, shifted by `ENCRUST_SHOWCASE_SEED`, so a run
/// repeats itself and a new seed gives the site a fresh set.
const ANGLES: [(f32, f32); 6] = [
    (0.6, 0.5),
    (-0.6, 0.5),
    (0.9, 0.35),
    (-0.35, 0.65),
    (0.3, 0.4),
    (-0.9, 0.45),
];

/// How far above the stage's middle the model is drawn, as a share of the camera's distance.
const RAISE: f32 = 0.1;

/// How much further back than a tight frame, so the model sits in the plate rather than fills it.
const STEP_BACK: f32 = 1.3;

/// The window with what a frame of `SlicerApp::ui` does to draw it, short of what reaches past
/// the test: no preferences written, no update checks, no printers on the network.
struct Shown {
    app: SlicerApp,
    busy: bool,
}

impl eframe::App for Shown {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let app = &mut self.app;
        crate::i18n::choose(app.machine.general.language);
        let ctx = ui.ctx().clone();
        let working = app.poll_jobs(&ctx);
        app.let_go_of_other_tools();
        app.refresh_preview();
        app.carry_height();
        app.refresh_supports(false);
        app.window().show(ui);
        let growing = app
            .doc
            .scene
            .objects()
            .iter()
            .any(SceneObject::supports_pending);
        self.busy = working || growing;
    }
}

/// The Crustling, from the test prints at the root of the repository.
fn crustling() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-prints/crustling.stl")
}

/// Where the pictures go.
fn out_dir() -> PathBuf {
    std::env::var_os("ENCRUST_SHOWCASE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/showcase"))
}

/// The angle the picture called `name` is taken from.
fn angle_of(name: &str) -> (f32, f32) {
    let seed: usize = std::env::var("ENCRUST_SHOWCASE_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or(0);
    // FNV-1a: the same name lands on the same angle on every machine and every run.
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    ANGLES[(hash as usize).wrapping_add(seed) % ANGLES.len()]
}

/// The pictured machine and resin with the Crustling picked on the plate, and the camera on
/// it from the angle the picture called `name` takes.
fn with_crustling(name: &str) -> SlicerApp {
    let mut app = app(false);
    app.view.options.started = true;
    let bytes = std::fs::read(crustling()).expect("test-prints/crustling.stl is in the repository");
    let file = Handed::Bytes {
        name: PathBuf::from("Crustling.stl"),
        bytes: Arc::from(bytes),
        beside: Arc::from([]),
    };
    let imported =
        crate::import::prepare(&file, &app.doc.plate, &mut |_| {}).expect("the Crustling imports");
    let id = app.doc.scene.insert(imported);
    app.doc.scene.select(Some(id));
    crate::panels::frame_view(&app.doc.scene, &app.doc.plate, &mut app.view.camera);
    let (turn_rad, pitch_rad) = angle_of(name);
    let camera = &mut app.view.camera;
    camera.yaw_rad += turn_rad;
    camera.pitch_rad = pitch_rad;
    camera.distance_mm *= STEP_BACK;
    // The tool's panel covers the left of the stage and the site crops the top-left of the
    // window, so the model is drawn a little above the stage's middle rather than on it.
    camera.target.z -= RAISE * camera.distance_mm;
    app
}

/// Draws `app` until every job has landed and the window is still, then writes it as
/// `<name>.png`.
fn shoot(name: &str, app: SlicerApp) {
    let mut harness = Harness::builder()
        .with_size(WINDOW)
        .with_pixels_per_point(SCALE)
        .wgpu()
        .build_eframe(|cc| {
            let render_state = cc
                .wgpu_render_state
                .as_ref()
                .expect("the wgpu harness hands over its render state");
            crate::render::install(render_state);
            theme::apply(&cc.egui_ctx);
            Shown { app, busy: true }
        });
    let mut quiet = 0;
    for _ in 0..MOST_FRAMES {
        harness.step();
        if harness.state().busy {
            quiet = 0;
            std::thread::sleep(std::time::Duration::from_millis(5));
        } else {
            quiet += 1;
            if quiet == QUIET_FRAMES {
                break;
            }
        }
    }
    assert!(!harness.state().busy, "{name}: a job was still running");
    let image = harness.render().expect("the window renders");
    let dir = out_dir();
    std::fs::create_dir_all(&dir).expect("the output directory can be made");
    let path = dir.join(format!("{name}.png"));
    image.save(&path).expect("the picture is written");
    eprintln!("wrote {}", path.display());
}

/// Each picture in each language the window speaks and in both appearances: `<name>`,
/// `<name>-ru`, `<name>-light` and `<name>-ru-light`, the site showing the one that matches
/// its own theme.
fn in_every_look(name: &str, make: impl Fn(&str) -> SlicerApp) {
    for (language_suffix, language) in [("", Language::English), ("-ru", Language::Russian)] {
        for (look_suffix, appearance) in [("", Appearance::Dark), ("-light", Appearance::Light)] {
            // Some text is named when the state is built, a plate's name among it.
            crate::i18n::choose(Some(language));
            let mut app = make(name);
            app.machine.general.language = Some(language);
            app.machine.general.appearance = appearance;
            shoot(&format!("{name}{language_suffix}{look_suffix}"), app);
        }
    }
}

#[test]
fn showcase_select() {
    in_every_look("select", |name| {
        let mut app = with_crustling(name);
        app.tool = Tool::Position;
        app
    });
}

/// How far the Crustling stands off the plate on its supports.
const LIFT_MM: f32 = 4.0;

/// The Crustling on grown supports.
fn on_supports(name: &str) -> SlicerApp {
    let mut app = with_crustling(name);
    app.tool = Tool::Supports;
    // Lifted off the plate, as a model on supports usually is, so every support has room to
    // stand under it.
    if let Some(crustling) = app.doc.scene.objects_mut().first_mut() {
        crustling.lift_to(LIFT_MM);
    }
    let window = &mut app.window();
    let started = window.tools.supports.start(
        &mut window.doc.scene,
        window.machine.slicing.layer_height_mm(),
        &window.machine.slicing.catalogue,
        crate::supports::Side::Outside,
    );
    started.expect("supports start on the Crustling");
    // Picked, the whole model is tinted, which hides where the supports meet it.
    app.doc.scene.select(None);
    app
}

/// The plate as it will print, on its own: the render view with the tool's panel folded away,
/// the Crustling in the pictured grey resin, seen low and close, as a person leaning over the
/// desk sees it, with the things beside the plate for scale.
#[test]
fn showcase_render() {
    in_every_look("render", |name| {
        let mut app = with_crustling(name);
        app.doc.scene.select(None);
        // The grey resin the pictured plate saves, opaque, rather than the first shipped one.
        let slicing = &mut app.machine.slicing;
        let mut grey = slicing
            .catalogue
            .resin(RENDER_RESIN)
            .expect("the pictured plate saves the grey resin")
            .profile
            .clone();
        grey.details.clarity = 0.0;
        slicing.resin_id = Some(RENDER_RESIN.to_owned());
        slicing.set_material(grey);
        app.view.options.render = true;
        app.view.options.inspector = false;
        crate::panels::frame_view(&app.doc.scene, &app.doc.plate, &mut app.view.camera);
        let camera = &mut app.view.camera;
        camera.yaw_rad += RENDER_TURN_RAD;
        camera.pitch_rad = RENDER_PITCH_RAD;
        camera.distance_mm *= RENDER_DISTANCE;
        app
    });
}

/// The resin the render picture prints in: `states::app` saves it.
const RENDER_RESIN: &str = "standard-grey";

/// The render picture's camera: a little round from the front, low over the mat, and close.
const RENDER_TURN_RAD: f32 = -0.1;
const RENDER_PITCH_RAD: f32 = 0.42;
const RENDER_DISTANCE: f32 = 1.4;

#[test]
fn showcase_supports() {
    in_every_look("supports", on_supports);
}

/// How far up the Crustling, 29 mm tall, the layer slider cuts it to show what is inside.
const HOLLOW_CUT_MM: f32 = 13.0;

/// The least the camera looks down on the hollow picture.
const HOLLOW_PITCH_RAD: f32 = 0.75;

/// The Crustling hollowed with the shipped wall, unpicked.
fn hollowed(name: &str) -> SlicerApp {
    let mut app = with_crustling(name);
    app.tool = Tool::Hollow;
    let started = app.tools.hollow.start(&app.doc.scene);
    started.expect("the Crustling hollows");
    app.doc.scene.select(None);
    app
}

#[test]
fn showcase_hollow() {
    in_every_look("hollow", |name| {
        let mut app = hollowed(name);
        // Whole, a hollow model looks like a solid one: cut it on the layer slider, as a
        // person checking the wall would, and leave it unpicked so the cut is not tinted.
        app.view.section.height_mm = Some(HOLLOW_CUT_MM);
        // High enough to look into the cut, whichever turn the name gave.
        let camera = &mut app.view.camera;
        camera.pitch_rad = camera.pitch_rad.max(HOLLOW_PITCH_RAD);
        app
    });
}

#[test]
fn showcase_check() {
    in_every_look("check", |name| {
        let mut app = with_crustling(name);
        app.tool = Tool::Check;
        // Picked, the whole model is tinted, which hides what the check marks on it.
        app.doc.scene.select(None);
        app
    });
}

/// The supports card drawing one support part by part, to scale, over the grown trees.
#[test]
fn showcase_shape() {
    in_every_look("shape", |name| {
        let mut app = on_supports(name);
        app.tool = Tool::Shape;
        app
    });
}

/// The supports card's hand tools over the grown trees.
#[test]
fn showcase_paint() {
    in_every_look("paint", |name| {
        let mut app = on_supports(name);
        app.tool = Tool::Paint;
        app
    });
}

/// The hollowed Crustling seen through, where drain holes go.
#[test]
fn showcase_drain() {
    in_every_look("drain", |name| {
        let mut app = hollowed(name);
        app.tool = Tool::Drain;
        app.view.options.xray = true;
        app
    });
}

/// The cut tool on the picked Crustling, joining its halves with dowels.
#[test]
fn showcase_cut() {
    in_every_look("cut", |name| {
        let mut app = with_crustling(name);
        app.tool = Tool::Cut;
        app.tools.cut.state.joints.kind = Some(core_joint::JointKind::Dowels);
        app
    });
}

/// The relief tool with one of its shipped pictures chosen for the picked Crustling.
#[test]
fn showcase_relief() {
    in_every_look("relief", |name| {
        let mut app = with_crustling(name);
        app.tool = Tool::Relief;
        crate::relief::choose_builtin(&mut app.tools.relief, &mut app.machine.status, 2);
        app.doc.scene.select(None);
        app
    });
}

/// The Crustling measured from its lowest point to its highest.
#[test]
fn showcase_measure() {
    in_every_look("measure", |name| {
        let mut app = with_crustling(name);
        app.tool = Tool::Measure;
        app.doc.scene.select(None);
        let crustling = &app.doc.scene.objects()[0];
        let world = crustling.transform.to_matrix();
        let points = crustling
            .mesh
            .vertices
            .iter()
            .map(|v| world.transform_point3(*v));
        let lowest = points.clone().min_by(|a, b| a.z.total_cmp(&b.z));
        let highest = points.max_by(|a, b| a.z.total_cmp(&b.z));
        let (Some(lowest), Some(highest)) = (lowest, highest) else {
            panic!("the Crustling has vertices");
        };
        let measure = &mut app.tools.measure;
        measure.pick(lowest, crate::measure::Snapped::Corner);
        measure.pick(highest, crate::measure::Snapped::Corner);
        app
    });
}

#[test]
fn showcase_repair() {
    in_every_look("repair", |name| {
        let mut app = with_crustling(name);
        app.tool = Tool::Repair;
        app.doc.scene.select(None);
        app
    });
}
