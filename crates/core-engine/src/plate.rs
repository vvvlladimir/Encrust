use std::sync::Arc;

use core_format::ExposurePlan;
use core_geometry::{Mesh, Transform};
use core_pipeline::{PanelOverrides, SlicedFormat};
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::cut::Cutting;

/// One model on the plate, as it will be printed: geometry, and where it stands.
///
/// What produced that geometry — the wall of a cavity, the profile of a support — stays
/// with whoever placed it. A run cuts the result (ADR 0129).
#[derive(Debug, Clone)]
pub struct Model {
    /// The model as it is sliced: its shell when it is hollowed, otherwise the mesh it
    /// was imported as, in the model's own space.
    pub mesh: Arc<Mesh>,
    /// Where the model stands on the plate.
    pub transform: Transform,
    /// Drain holes and channels, wound inward, in the model's own space: the winding is
    /// what takes them out of the model, cavity or no cavity (ADR 0075).
    pub cuts: Option<Arc<Mesh>>,
    /// The support trees standing under it, already in plate coordinates.
    pub supports: Vec<Arc<Mesh>>,
}

impl Model {
    /// A model with nothing cut out of it and nothing standing under it.
    pub fn placed(mesh: Arc<Mesh>, transform: Transform) -> Self {
        Self {
            mesh,
            transform,
            cuts: None,
            supports: Vec::new(),
        }
    }
}

/// Everything one run needs: what is on the plate, the machine, the resin, and how the
/// stack is cut and drawn.
#[derive(Debug, Clone)]
pub struct Plate {
    pub models: Vec<Model>,
    pub printer: PrinterProfile,
    pub material: MaterialProfile,
    /// What the masks are drawn with, over what the printer profile states.
    pub panel: PanelOverrides,
    pub cutting: Cutting,
    /// Exposure bands over the resin's own; empty means the resin's throughout.
    pub exposure: ExposurePlan,
    /// Whether every island is taken out of the written file.
    pub remove_islands: bool,
    /// Which container is written. The output name has the last word on it (ADR 0047),
    /// so whoever holds the name resolves it before the run starts.
    pub format: SlicedFormat,
    /// Layers rasterised at once. Peak memory is this many masks (ADR 0010).
    pub raster_window: usize,
    /// When the file is made, seconds since the Unix epoch, read off the caller's clock.
    pub created_unix_s: u64,
}
