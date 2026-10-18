use crate::camera::{CameraTurn, OrbitCamera};
use crate::cut::CutTool;
use crate::drain::DrainTool;
use crate::gizmo::TransformGizmo;
use crate::hollow::HollowTool;
use crate::import::Imports;
use crate::measure::Measure;
use crate::network::Network;
use crate::orient::OrientTool;
use crate::plate::BuildPlate;
use crate::preview::Preview;
use crate::project::Opened;
use crate::recent::Recent;
use crate::relief::ReliefTool;
use crate::repair::Repairs;
use crate::report::Report;
use crate::scene::Scene;
use crate::settings::Settings;
use crate::slicing::Slicing;
use crate::status::Status;
use crate::supports::SupportTool;
use crate::undo::History;
use crate::updates::Updates;
use crate::viewport_input::ViewportInput;
use crate::workspace::{Array, Section, ViewOptions};

/// What is being printed: the models, the plate they stand on, and the file they are
/// saved in. Everything an undo or a save reaches.
#[derive(Default)]
pub struct Doc {
    pub scene: Scene,
    pub plate: BuildPlate,
    pub history: History,
    pub project: Opened,
    pub imports: Imports,
    pub repairs: Repairs,
}

/// How the plate is looked at: none of it changes what is printed.
pub struct View {
    pub options: ViewOptions,
    pub camera: OrbitCamera,
    pub gizmo: TransformGizmo,
    pub section: Section,
    pub input: ViewportInput,
    /// The swing the view cube asked for, while it is still under way.
    pub turn: Option<CameraTurn>,
    /// How long the window's last pass over its panels took, milliseconds of CPU: the
    /// viewport writes it to the log beside its own frame.
    pub ui_ms: f32,
    /// The camera as the last frame drew it, and when it last moved, seconds of egui's
    /// clock: heavy meshes are drawn light while it moves.
    pub drawn_camera: OrbitCamera,
    pub moved_s: f64,
    pub lods: crate::render::Lods,
    /// What this machine draws and keeps up while the camera moves, and what the last
    /// frame drew, faces: the next frame's time is what that cost.
    pub budget: crate::render::Budget,
    pub drawn_faces: Option<usize>,
    /// The level each model was last drawn at, 0 for its own mesh.
    pub levels: core_geometry::FastMap<crate::scene::ObjectId, usize>,
}

impl View {
    /// A view framing `plate` from the default angle.
    pub fn framing(plate: &BuildPlate) -> Self {
        Self {
            options: ViewOptions::default(),
            camera: OrbitCamera::framing_plate(plate),
            gizmo: TransformGizmo::default(),
            section: Section::default(),
            input: ViewportInput::default(),
            turn: None,
            ui_ms: 0.0,
            drawn_camera: OrbitCamera::framing_plate(plate),
            moved_s: f64::NEG_INFINITY,
            lods: crate::render::Lods::default(),
            budget: crate::render::Budget::default(),
            drawn_faces: None,
            levels: core_geometry::FastMap::default(),
        }
    }
}

/// The state each tool of the rail keeps between frames.
#[derive(Default)]
pub struct Tools {
    pub supports: SupportTool,
    pub hollow: HollowTool,
    pub drain: DrainTool,
    pub cut: CutTool,
    pub relief: ReliefTool,
    pub boolean: crate::boolean::BooleanTool,
    pub orient: OrientTool,
    pub measure: Measure,
    pub measure_edges: crate::measure::EdgeCache,
    pub array: Array,
}

/// The printer, what it will be sent, and what the last job said.
#[derive(Default)]
pub struct Machine {
    pub slicing: Slicing,
    pub network: Network,
    pub settings: Settings,
    pub preview: Preview,
    pub status: Status,
    pub updates: Updates,
    /// The bug report being written, while the sheet for it is open.
    pub report: Report,
    /// The files opened lately, for the start page.
    pub recent: Recent,
    /// How the window itself behaves, from the Settings window's General page.
    pub general: crate::prefs::General,
}
