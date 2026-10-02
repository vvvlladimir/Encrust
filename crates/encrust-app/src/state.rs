use crate::camera::OrbitCamera;
use crate::cut::CutTool;
use crate::drain::DrainTool;
use crate::gizmo::TransformGizmo;
use crate::hollow::HollowTool;
use crate::import::Imports;
use crate::measure::Measure;
use crate::network::Network;
use crate::orient::OrientTool;
use crate::panels::Overlays;
use crate::plate::Plate;
use crate::preview::Preview;
use crate::project::Opened;
use crate::relief::ReliefTool;
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
    pub plate: Plate,
    pub history: History,
    pub project: Opened,
    pub imports: Imports,
}

/// How the plate is looked at: none of it changes what is printed.
pub struct View {
    pub options: ViewOptions,
    pub camera: OrbitCamera,
    pub gizmo: TransformGizmo,
    pub section: Section,
    pub input: ViewportInput,
    pub overlays: Overlays,
}

impl View {
    /// A view framing `plate` from the default angle.
    pub fn framing(plate: &Plate) -> Self {
        Self {
            options: ViewOptions::default(),
            camera: OrbitCamera::framing_plate(plate),
            gizmo: TransformGizmo::default(),
            section: Section::default(),
            input: ViewportInput::default(),
            overlays: Overlays::default(),
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
    pub orient: OrientTool,
    pub measure: Measure,
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
}
