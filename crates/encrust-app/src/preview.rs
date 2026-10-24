use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use std::ops::Range;

use anyhow::Context as _;
use core_analysis::Measured;
use core_geometry::Scalar;
use core_raster::{LayerRuns, RasterSettings};
use core_slicer::{Contour, LayerPlan, Sliced, Solid, Windows};

use core_engine::Cutting;
use core_pipeline::Tolerance;

use crate::job::{MeasureJob, PreviewJob};
use crate::scene::Scene;

mod job;
mod picture;

pub use picture::{MaskView, PixelRect, on_plate};

/// The sliced stack the preview panel is showing, and the build that produces it.
#[derive(Default)]
pub struct Preview {
    /// The plate being cut, and the layers it cuts into.
    source: Option<Stack>,
    /// Index into the stack, not a height: the slider counts layers.
    layer: usize,
    job: Option<Build>,
    texture: Option<(TextureKey, egui::TextureHandle)>,
    /// The layer being shown at the panel's own resolution, which the magnifier reads.
    full: Option<(TextureKey, Shown)>,
    /// A stretch of the layer at full resolution, for a view zoomed past what the
    /// shrunk texture holds.
    detail: Option<((TextureKey, PixelRect), egui::TextureHandle)>,
    /// How far into the mask the pane is zoomed, and where.
    pub view: MaskView,
    /// A height to park the slider at once the stack being built arrives; `None` inside
    /// is the top.
    parked: Option<Option<Scalar>>,
    /// The transport is stepping through the stack on its own.
    playing: bool,
    /// What the stack cures on the panel it was measured for, and the pass measuring it.
    measured: Option<(MeasureKey, Measured)>,
    measuring: Option<(MeasureKey, MeasureJob)>,
    /// How the stack was last asked to be folded, which is what the picture follows.
    fold: Fold,
}

/// How a stack is folded: the layers the plate holds, whether islands are taken out, and
/// how far the resin moves each layer's walls.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fold {
    pub held_layers: usize,
    pub remove_islands: bool,
    pub hanging_passes: u32,
    pub tolerance: Tolerance,
}

impl Fold {
    fn start(self) -> Measured {
        let fold = Measured::new(self.held_layers);
        if self.remove_islands {
            fold.removing_hanging(self.hanging_passes)
        } else {
            fold
        }
    }
}

/// The layer being shown, and the pixels of it that stand on nothing in the layer below,
/// both at the panel's own resolution.
struct Shown {
    runs: LayerRuns,
    islands: LayerRuns,
}

/// What a measurement was taken of: the stack, the panel and grey it was rasterised with,
/// and how many layers the plate holds.
#[derive(Clone, Copy, PartialEq)]
struct MeasureKey {
    fingerprint: u64,
    settings: RasterSettings,
    fold: Fold,
}

/// The plate to preview, the layers it cuts into, and the one window of it that has
/// actually been cut.
struct Stack {
    solid: Arc<Solid>,
    windows: Windows,
    fingerprint: u64,
    /// The window last cut and the contours it came to, so that stepping through a
    /// window costs nothing; see `docs/decisions/0068-the-window-holds-no-stack.md`.
    cut: Option<(Range<usize>, Sliced)>,
}

/// A running build and the fingerprint its stack will carry.
struct Build {
    job: PreviewJob,
    fingerprint: u64,
}

/// What a cached picture was drawn from. Anything here changing means drawing it again.
#[derive(Clone, Copy, PartialEq)]
struct TextureKey {
    fingerprint: u64,
    layer: usize,
    settings: RasterSettings,
    /// Whether the islands a measurement took out are erased from the picture.
    cleaned: bool,
}

/// Identifies the stack a scene would slice into.
///
/// Placement, visibility, which meshes are loaded, the cuts in them and the layer height
/// all change the contours; selecting an object or moving the camera does not, and must not cost a
/// rebuild.
pub fn stack_fingerprint(scene: &Scene, cutting: Cutting) -> u64 {
    let mut hasher = DefaultHasher::new();
    cutting.layer_height_mm.to_bits().hash(&mut hasher);
    for factor in cutting.compensation.scale() {
        factor.to_bits().hash(&mut hasher);
    }
    if let Some(adaptive) = cutting.adaptive {
        for value in [
            adaptive.cusp_mm,
            adaptive.min_height_mm,
            adaptive.max_height_mm,
        ] {
            value.to_bits().hash(&mut hasher);
        }
    }
    plate_fingerprint(scene).hash(&mut hasher);
    hasher.finish()
}

/// Identifies what stands on the plate, without the numbers it would be cut with: the
/// half of [`stack_fingerprint`] a sliced file open in the window is held against.
pub fn plate_fingerprint(scene: &Scene) -> u64 {
    let mut hasher = DefaultHasher::new();
    scene.active_plate().hash(&mut hasher);
    for object in scene.printable(scene.active_plate()) {
        object.id.hash(&mut hasher);
        Arc::as_ptr(&object.mesh).hash(&mut hasher);
        object.hollow.shell().map(Arc::as_ptr).hash(&mut hasher);
        object
            .hollow
            .cut_bodies()
            .map(Arc::as_ptr)
            .hash(&mut hasher);
        for supports in object.supports.meshes().unwrap_or_default() {
            Arc::as_ptr(supports).hash(&mut hasher);
        }
        let transform = object.transform;
        for value in [
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
            transform.rotation.w,
            transform.scale.x,
            transform.scale.y,
            transform.scale.z,
        ]
        .into_iter()
        .chain(transform.stretch.to_cols_array())
        {
            value.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

impl Preview {
    /// Moves the slider to the layer nearest `height_mm`, or to the top for `None`: the
    /// height the Prepare mode was cut at. Waits for a stack still being built.
    pub fn show_height(&mut self, height_mm: Option<Scalar>) {
        if self.is_building() {
            self.parked = Some(height_mm);
            return;
        }
        let Some(source) = self.source.as_ref() else {
            return;
        };
        let count = source.windows.layer_count();
        self.layer = match height_mm {
            Some(height_mm) => self.layer_topping_near(height_mm),
            None => count.saturating_sub(1),
        };
    }

    /// The layer whose top stands nearest `height_mm`, clamped to the stack.
    pub fn layer_topping_near(&self, height_mm: Scalar) -> usize {
        let count = self.layer_count();
        let height = |layer: usize| self.top_of(layer).unwrap_or(Scalar::MAX);
        let (mut below, mut above) = (0, count);
        while below < above {
            let middle = (below + above) / 2;
            if height(middle) < height_mm {
                below = middle + 1;
            } else {
                above = middle;
            }
        }
        let under = above.saturating_sub(1);
        let nearer = if above < count && height(above) - height_mm < height_mm - height(under) {
            above
        } else {
            under
        };
        nearer.min(count.saturating_sub(1))
    }

    /// The height to hand the Prepare mode: the layer being shown, `None` at the top layer
    /// where nothing is cut away, and no answer at all without a stack.
    pub fn prepare_height(&self) -> Option<Option<Scalar>> {
        let count = self.layer_count();
        if count == 0 {
            return None;
        }
        Some(self.layer_z().filter(|_| self.layer + 1 < count))
    }

    /// Whether a stack is being cut or measured, either of which slices the whole plate.
    pub fn is_slicing(&self) -> bool {
        self.job.is_some() || self.measuring.is_some()
    }

    pub fn is_playing(&self) -> bool {
        self.playing && self.layer_count() > 0
    }

    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
    }

    /// Moves the slider by `layers`, stopping at either end of the stack.
    pub fn step(&mut self, layers: i64) {
        let last = self.layer_count().saturating_sub(1) as i64;
        let target = (self.layer as i64 + layers).clamp(0, last.max(0));
        self.layer = target as usize;
    }

    pub fn layer_count(&self) -> usize {
        self.source
            .as_ref()
            .map_or(0, |stack| stack.windows.layer_count())
    }

    pub fn layer(&self) -> usize {
        self.layer
    }

    /// Moves the slider, clamped to the stack so a shorter one never leaves it past the
    /// end.
    pub fn set_layer(&mut self, index: usize) {
        self.layer = index.min(self.layer_count().saturating_sub(1));
    }

    /// Where every layer of the stack starts and stops.
    pub fn plan(&self) -> Option<&LayerPlan> {
        Some(self.source.as_ref()?.windows.plan())
    }

    /// Height the plate stands at for the layer being shown: the top of that layer, as
    /// the printer and the written file state it.
    pub fn layer_z(&self) -> Option<Scalar> {
        self.top_of(self.layer)
    }

    /// Height the viewport cuts the plate at for the layer being shown: the plane its
    /// mask is sampled at rather than its top, so the section shows what the mask does.
    pub fn section_z(&self) -> Option<Scalar> {
        self.source.as_ref()?.windows.section_plane_of(self.layer)
    }

    /// Bottom of layer `index` above the plate, millimetres, or `None` for the first,
    /// which stands on the plate.
    pub fn bottom_of(&self, index: usize) -> Option<Scalar> {
        self.top_of(index.checked_sub(1)?)
    }

    /// Top of the last layer above the plate, millimetres: how tall the stack stands.
    pub fn stack_top_mm(&self) -> Option<Scalar> {
        self.top_of(self.layer_count().checked_sub(1)?)
    }

    /// Top of layer `index` above the plate, millimetres, as the plan cuts it.
    fn top_of(&self, index: usize) -> Option<Scalar> {
        self.source.as_ref()?.windows.plan().top_of(index)
    }

    /// Area the layer exposes, square millimetres, taken from the contours rather than
    /// from the preview mask: the mask is shrunk with a maximum, which inflates area.
    ///
    /// `None` until the window holding the layer has been cut, which drawing it does.
    pub fn layer_area_mm2(&self) -> Option<Scalar> {
        let layer = self.current()?;
        let double_area: Scalar = layer.contours.iter().map(Contour::signed_double_area).sum();
        Some((double_area / 2.0).max(0.0))
    }

    /// The stack no longer matches the scene it was cut from.
    pub fn is_stale(&self, fingerprint: u64) -> bool {
        self.source
            .as_ref()
            .is_some_and(|stack| stack.fingerprint != fingerprint)
    }
}

#[cfg(test)]
pub(crate) mod tests;
