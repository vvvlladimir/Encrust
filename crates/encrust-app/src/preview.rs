use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use std::ops::Range;

use anyhow::{Context as _, Result, bail};
use core_analysis::{Measured, erase, island_runs};
use core_format::{OpenFile, ReadSeek, SlicedFile};
use core_geometry::{Mesh, Scalar};
use core_raster::{
    Grey, LayerMask, LayerRuns, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer,
    Shading, crop, downsample, shrink_factor,
};
use core_slicer::{Contour, LayerPlan, Sliced, Windows};

use core_engine::{Cutting, bake};
use core_pipeline::{Opened, Tolerance, open};

use crate::files::Handed;
use crate::job::{MeasureJob, MeasureOutcome, PreviewJob, PreviewOutcome, models_of};
use crate::scene::Scene;
use crate::status::Status;
use crate::ui::theme;

/// Widest preview mask handed to egui, pixels. A 5760 x 3600 panel is 83 MB once it has
/// become RGBA, which no slider drag can upload a frame of; see
/// `docs/decisions/0023-preview-masks-are-downsampled.md`.
pub const MAX_PREVIEW_PX: u32 = 2048;

/// The sliced stack the preview panel is showing, and the build that produces it.
#[derive(Default)]
pub struct Preview {
    source: Option<Source>,
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
    pub tolerance: Tolerance,
}

impl Fold {
    fn start(self) -> Measured {
        let fold = Measured::new(self.held_layers);
        if self.remove_islands {
            fold.removing_islands()
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
    mesh: Arc<Mesh>,
    windows: Windows,
    fingerprint: u64,
    /// The window last cut and the contours it came to, so that stepping through a
    /// window costs nothing; see `docs/decisions/0068-the-window-holds-no-stack.md`.
    cut: Option<(Range<usize>, Sliced)>,
}

/// Where the layers the panel is showing come from.
///
/// Both answer the same question — the runs of layer `n` — and neither holds a stack: a
/// plate cuts the layer it is asked for, a file decodes it. See `docs/decisions/0151`.
enum Source {
    Cut(Stack),
    Read(Box<ReadFile>),
}

/// A sliced file open in the window, and what it says about itself.
struct ReadFile {
    path: PathBuf,
    facts: SlicedFile,
    open: Opened<Box<dyn ReadSeek>>,
    fingerprint: u64,
    /// What stood on the plate when the file was opened. The plate moving on is what ends
    /// the file's turn under the slider, so that no reading comes from the other source.
    plate: u64,
}

impl Source {
    fn stack(&self) -> Option<&Stack> {
        match self {
            Self::Cut(stack) => Some(stack),
            Self::Read(_) => None,
        }
    }

    fn fingerprint(&self) -> u64 {
        match self {
            Self::Cut(stack) => stack.fingerprint,
            Self::Read(read) => read.fingerprint,
        }
    }

    fn layer_count(&self) -> usize {
        match self {
            Self::Cut(stack) => stack.windows.layer_count(),
            Self::Read(read) => read.facts.layer_count() as usize,
        }
    }
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

/// A rectangle of panel pixels, `x0..x1` by `y0..y1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl PixelRect {
    pub fn width(self) -> u32 {
        self.x1 - self.x0
    }

    pub fn height(self) -> u32 {
        self.y1 - self.y0
    }

    fn contains(self, other: Self) -> bool {
        self.x0 <= other.x0 && self.y0 <= other.y0 && self.x1 >= other.x1 && self.y1 >= other.y1
    }

    /// Grown by up to half its size each way, no wider than `most` and inside the panel.
    fn padded(self, most: u32, width_px: u32, height_px: u32) -> Self {
        let grow = |from: u32, to: u32, extent: u32| {
            let length = to - from;
            let pad = (length / 2).min(most.saturating_sub(length) / 2);
            (from.saturating_sub(pad), (to + pad).min(extent))
        };
        let (x0, x1) = grow(self.x0, self.x1, width_px);
        let (y0, y1) = grow(self.y0, self.y1, height_px);
        Self { x0, y0, x1, y1 }
    }
}

/// How far into the mask the pane is zoomed, and the panel pixel at the middle of it.
///
/// Kept across layers, so stepping through the stack looks at the same spot of each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaskView {
    /// 1 shows the whole panel.
    pub zoom: f32,
    /// Panel pixel at the middle of the pane, or `None` for the middle of the panel.
    centre: Option<egui::Vec2>,
}

impl Default for MaskView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            centre: None,
        }
    }
}

/// Deepest zoom: a panel pixel this many points across at the least.
const MOST_POINTS_A_PIXEL: f32 = 48.0;

impl MaskView {
    /// Where the whole panel lands on screen, given the rectangle it fills at zoom 1.
    pub fn placement(&self, fit: egui::Rect, panel_px: egui::Vec2) -> egui::Rect {
        let scale = fit.size() / panel_px * self.zoom;
        let centre = self.centre.unwrap_or(panel_px / 2.0);
        egui::Rect::from_min_size(fit.center() - centre * scale, panel_px * scale)
    }

    /// The panel pixel, fractional, at a point on screen.
    pub fn pixel_at(&self, at: egui::Pos2, fit: egui::Rect, panel_px: egui::Vec2) -> egui::Vec2 {
        let placed = self.placement(fit, panel_px);
        (at - placed.min) / placed.size() * panel_px
    }

    /// Zooms by `factor`, keeping the pixel under `at` where it is on screen.
    pub fn zoom_at(&mut self, factor: f32, at: egui::Pos2, fit: egui::Rect, panel_px: egui::Vec2) {
        let most = (MOST_POINTS_A_PIXEL * panel_px.x / fit.width()).max(1.0);
        let zoom = (self.zoom * factor).clamp(1.0, most);
        let held = self.pixel_at(at, fit, panel_px);
        let scale = fit.size() / panel_px * zoom;
        self.zoom = zoom;
        self.centre = Some(held - (at - fit.center()) / scale);
        self.clamp(panel_px);
    }

    /// Moves the picture by `delta` points, as a drag does.
    pub fn pan(&mut self, delta: egui::Vec2, fit: egui::Rect, panel_px: egui::Vec2) {
        let scale = fit.size() / panel_px * self.zoom;
        self.centre = Some(self.centre.unwrap_or(panel_px / 2.0) - delta / scale);
        self.clamp(panel_px);
    }

    fn clamp(&mut self, panel_px: egui::Vec2) {
        if let Some(centre) = self.centre.as_mut() {
            *centre = centre.clamp(egui::Vec2::ZERO, panel_px);
        }
    }
}

impl Preview {
    /// Opens a sliced file and shows it in place of the plate.
    ///
    /// Only its tables are read: a layer is decoded when the slider lands on it, so opening
    /// a stack of thousands costs the same as opening one of ten.
    pub fn read_file(&mut self, file: &Handed, plate: u64) -> Result<()> {
        let path = file.path();
        let source: Box<dyn ReadSeek> = file
            .reader()
            .with_context(|| format!("cannot read {}", path.display()))?;
        let open = open(path, source).with_context(|| format!("cannot open {}", path.display()))?;
        let facts = open.facts().clone();
        if facts.layer_count() == 0 {
            bail!("{} has no layers", path.display());
        }
        self.job = None;
        self.full = None;
        self.detail = None;
        self.texture = None;
        self.measured = None;
        self.measuring = None;
        self.layer = facts.layer_count() as usize - 1;
        self.view = MaskView::default();
        self.source = Some(Source::Read(Box::new(ReadFile {
            fingerprint: file_fingerprint(file),
            path: path.to_owned(),
            facts,
            open,
            plate,
        })));
        Ok(())
    }

    /// What the file being shown says about itself, or `None` while a plate is being shown.
    pub fn read_facts(&self) -> Option<&SlicedFile> {
        match self.source.as_ref()? {
            Source::Read(read) => Some(&read.facts),
            Source::Cut(_) => None,
        }
    }

    /// The file being shown, by the name it was opened under.
    pub fn read_path(&self) -> Option<&Path> {
        match self.source.as_ref()? {
            Source::Read(read) => Some(&read.path),
            Source::Cut(_) => None,
        }
    }

    /// The panel the file was written for, which is what its layers have to be drawn on: a
    /// profile loaded in the window says nothing about a file another slicer wrote.
    pub fn read_panel(&self) -> Option<RasterSettings> {
        let facts = self.read_facts()?;
        let (width_mm, height_mm) = facts.display_mm?;
        Some(RasterSettings {
            width_px: facts.width_px,
            height_px: facts.height_px,
            pitch: PixelPitch {
                x: Scalar::from(width_mm) / facts.width_px as Scalar,
                y: Scalar::from(height_mm) / facts.height_px as Scalar,
            },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        })
    }

    /// Closes whatever file is open and leaves the panel empty.
    pub fn close_file(&mut self) {
        if matches!(self.source, Some(Source::Read(_))) {
            *self = Self::default();
        }
    }

    /// Starts cutting everything visible on the plate. Fails when there is nothing there.
    pub fn build(&mut self, scene: &Scene, cutting: Cutting) -> Result<()> {
        let baked = bake(
            &models_of(scene, scene.active_plate()),
            &cutting.compensation,
        )
        .context("nothing visible on the plate to preview")?;
        self.job = Some(Build {
            job: PreviewJob::spawn(baked, cutting),
            fingerprint: stack_fingerprint(scene, cutting),
        });
        Ok(())
    }

    /// Drains a running build and measurement. Returns whether one is still going, which
    /// is what tells the window to keep repainting.
    pub fn poll(&mut self, status: &mut Status) -> bool {
        let building = self.poll_build(status);
        self.poll_measure(status) || building
    }

    /// Starts measuring the stack as `settings` would write it, unless that measurement is
    /// already held or under way.
    pub fn measure(&mut self, settings: &RasterSettings, fold: Fold) {
        self.fold = fold;
        let Some(key) = self.measure_key(settings, fold) else {
            return;
        };
        let known = |held: Option<MeasureKey>| held == Some(key);
        if known(self.measured.as_ref().map(|(key, _)| *key))
            || known(self.measuring.as_ref().map(|(key, _)| *key))
        {
            return;
        }
        // Only a plate being cut is measured this way. A file's masks are already
        // written, so what they cure is read off them instead; see `read_measured`.
        let Some(stack) = self.source.as_ref().and_then(Source::stack) else {
            return;
        };
        // Measured as the picture shows it, so a risk's position lands on the picture.
        let job = MeasureJob::spawn(
            Arc::clone(&stack.mesh),
            stack.windows.clone(),
            as_seen(settings),
            fold.tolerance,
            fold.start(),
        );
        self.measuring = Some((key, job));
    }

    /// What the stack cures, once it has been measured for the panel as it now is.
    pub fn measured(&self, settings: &RasterSettings, fold: Fold) -> Option<&Measured> {
        let key = self.measure_key(settings, fold)?;
        let (measured_key, measured) = self.measured.as_ref()?;
        (*measured_key == key).then_some(measured)
    }

    pub fn is_measuring(&self) -> bool {
        self.measuring.is_some()
    }

    fn measure_key(&self, settings: &RasterSettings, fold: Fold) -> Option<MeasureKey> {
        Some(MeasureKey {
            fingerprint: self.source.as_ref()?.fingerprint(),
            settings: *settings,
            fold,
        })
    }

    fn poll_measure(&mut self, status: &mut Status) -> bool {
        let Some((key, job)) = self.measuring.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };
        let key = *key;
        self.measuring = None;
        match outcome {
            MeasureOutcome::Measured(measured) => self.measured = Some((key, *measured)),
            MeasureOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    fn poll_build(&mut self, status: &mut Status) -> bool {
        let Some(build) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = build.job.poll() else {
            return true;
        };

        let fingerprint = build.fingerprint;
        self.job = None;
        match outcome {
            PreviewOutcome::Built(mesh, windows) => {
                self.layer = self.layer.min(windows.layer_count().saturating_sub(1));
                self.source = Some(Source::Cut(Stack {
                    mesh,
                    windows,
                    fingerprint,
                    cut: None,
                }));
                if let Some(height_mm) = self.parked.take() {
                    self.show_height(height_mm);
                }
            }
            PreviewOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    /// Stops waiting for the build. The thread it left behind finishes into a channel
    /// nobody reads; a slicing run has no point inside it to stop at.
    pub fn cancel(&mut self) {
        self.job = None;
    }

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
        let count = source.layer_count();
        let Some(height_mm) = height_mm else {
            self.layer = count.saturating_sub(1);
            return;
        };
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
        self.layer = nearer.min(count.saturating_sub(1));
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

    pub fn is_building(&self) -> bool {
        self.job.is_some()
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

    /// How many panel pixels one preview pixel stands for, which is what the mask panel
    /// puts in its heading. See `docs/decisions/0023-preview-masks-are-downsampled.md`.
    pub fn downsample_factor(&self, settings: &RasterSettings) -> u32 {
        shrink_factor(
            settings.width_px,
            settings.height_px,
            MAX_PREVIEW_PX,
            MAX_PREVIEW_PX,
        )
    }

    pub fn layer_count(&self) -> usize {
        self.source.as_ref().map_or(0, Source::layer_count)
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
        Some(self.source.as_ref()?.stack()?.windows.plan())
    }

    /// Height the plate stands at for the layer being shown: the top of that layer, as
    /// the printer and the written file state it.
    pub fn layer_z(&self) -> Option<Scalar> {
        self.top_of(self.layer)
    }

    /// Top of layer `index` above the plate, millimetres: the plan's for a plate being cut,
    /// and the layer table's own for a file being read.
    fn top_of(&self, index: usize) -> Option<Scalar> {
        match self.source.as_ref()? {
            Source::Cut(stack) => stack.windows.plan().top_of(index),
            Source::Read(read) => read
                .facts
                .layers
                .get(index)
                .map(|entry| Scalar::from(entry.z_mm)),
        }
    }

    /// Area the layer exposes, square millimetres, taken from the contours rather than
    /// from the preview mask: the mask is shrunk with a maximum, which inflates area.
    ///
    /// `None` until the window holding the layer has been cut, which drawing it does.
    pub fn layer_area_mm2(&self) -> Option<Scalar> {
        if let Some((_, shown)) = self.full.as_ref().filter(|_| self.read_facts().is_some()) {
            // A file has no contours; its area is the mask's, which is exact because the
            // runs held here are the panel's own resolution rather than the shrunk picture.
            let panel = self.read_panel()?;
            let lit: u64 = shown
                .runs
                .runs()
                .iter()
                .filter(|run| run.value > 0)
                .map(|run| u64::from(run.length))
                .sum();
            return Some(lit as Scalar * panel.pitch.x * panel.pitch.y);
        }
        let layer = self.current()?;
        let double_area: Scalar = layer.contours.iter().map(Contour::signed_double_area).sum();
        Some((double_area / 2.0).max(0.0))
    }

    /// The file being shown was opened over another plate than the one standing now, so
    /// what the window states about the plate and what the file states are two sources at
    /// once; see `docs/decisions/0151`.
    pub fn file_is_over_an_old_plate(&self, plate: u64) -> bool {
        match self.source.as_ref() {
            Some(Source::Read(read)) => read.plate != plate,
            Some(Source::Cut(_)) | None => false,
        }
    }

    /// Exposure of the layer being shown as the file's own table states it, seconds.
    pub fn read_layer_exposure_s(&self) -> Option<f32> {
        let facts = self.read_facts()?;
        Some(facts.layers.get(self.layer)?.exposure_s)
    }

    /// The stack no longer matches the scene it was cut from.
    pub fn is_stale(&self, fingerprint: u64) -> bool {
        self.source
            .as_ref()
            .and_then(Source::stack)
            .is_some_and(|stack| stack.fingerprint != fingerprint)
    }

    /// Rasterises the layer being shown, cutting its window first if that has not
    /// happened yet, and shrinks it to something a screen can hold.
    #[cfg(test)]
    pub fn mask(&mut self, settings: &RasterSettings) -> Result<LayerMask> {
        let shown = self.full(settings)?;
        Ok(downsample(&shown.runs, MAX_PREVIEW_PX, MAX_PREVIEW_PX))
    }

    /// The layer being shown at the panel's own resolution, rasterised once for each
    /// layer, scene and panel, with the islands on it against the layer below.
    fn full(&mut self, settings: &RasterSettings) -> Result<&Shown> {
        let Some(key) = self.key(settings) else {
            bail!("no sliced stack to preview");
        };
        if self.full.as_ref().is_none_or(|(cached, _)| *cached != key) {
            self.cut_window()?;
            let seen = as_seen(settings);
            let layer = self.layer;
            let under = layer.checked_sub(1);

            // Taken before the layers, because decoding one needs the source mutably.
            let taken = |preview: &Self, at: usize| {
                preview
                    .cleaned(settings)
                    .map_or(Vec::new(), |measured| measured.removed_from(at).to_vec())
            };
            let (taken_here, taken_under) = (taken(self, layer), under.map(|at| taken(self, at)));

            let runs = erase(&self.runs_of(layer, &seen)?, &taken_here);
            let islands = match (under, taken_under) {
                (Some(under), Some(taken_under)) => {
                    let below = erase(&self.runs_of(under, &seen)?, &taken_under);
                    island_runs(&runs, &below, seen.pitch)
                }
                _ => LayerRuns::builder(runs.width(), runs.height()).finish(),
            };
            self.full = Some((key, Shown { runs, islands }));
        }
        match &self.full {
            Some((_, shown)) => Ok(shown),
            None => bail!("no sliced stack to preview"),
        }
    }

    /// The runs of layer `index` at the panel's own resolution.
    ///
    /// A plate is rasterised from the window already cut when it is there and cut on its own
    /// when it is not: the layer under a window's first is in the one before. A file is
    /// decoded, which needs no rasteriser at all — the runs are what it holds.
    fn runs_of(&mut self, index: usize, settings: &RasterSettings) -> Result<LayerRuns> {
        match self.source.as_mut() {
            Some(Source::Read(read)) => {
                let runs = read
                    .open
                    .layer(index as u32)
                    .with_context(|| format!("cannot decode layer {index}"))?;
                let mut builder = LayerRuns::builder(read.facts.width_px, read.facts.height_px);
                for run in runs {
                    builder.push(run.length, run.value);
                }
                Ok(builder.finish())
            }
            _ => self.rasterised(index, settings),
        }
    }

    /// Layer `index` rasterised off the plate being cut.
    fn rasterised(&self, index: usize, settings: &RasterSettings) -> Result<LayerRuns> {
        let Some(stack) = self.source.as_ref().and_then(Source::stack) else {
            bail!("no plate to preview");
        };
        let held = stack
            .cut
            .as_ref()
            .and_then(|(window, sliced)| sliced.layers.get(index.checked_sub(window.start)?));
        let alone;
        let layer = match held {
            Some(layer) => layer,
            None => {
                alone = stack
                    .windows
                    .cut(&stack.mesh, index..index + 1)
                    .context("cannot cut the layer under the one being previewed")?;
                let Some(layer) = alone.layers.first() else {
                    bail!("layer {index} is not in the stack");
                };
                layer
            }
        };
        let rastered = ScanlineRasterizer
            .rasterize(layer, settings)
            .with_context(|| format!("cannot rasterise the layer at z = {:.3} mm", layer.z))?;
        Ok(rastered.runs)
    }

    fn key(&self, settings: &RasterSettings) -> Option<TextureKey> {
        Some(TextureKey {
            fingerprint: self.source.as_ref()?.fingerprint(),
            layer: self.layer,
            settings: *settings,
            cleaned: self.cleaned(settings).is_some(),
        })
    }

    /// The measurement that took islands out of this stack, once there is one.
    fn cleaned(&self, settings: &RasterSettings) -> Option<&Measured> {
        self.measured(settings, self.fold)
            .filter(|_| self.fold.remove_islands)
    }

    /// The layer at full resolution over at least `visible`, or `None` when that is more
    /// of the panel than a texture is allowed to hold, and the shrunk one has to do.
    ///
    /// Cut with room around it, so a small pan reuses the texture instead of cutting again.
    pub fn detail(
        &mut self,
        ctx: &egui::Context,
        settings: &RasterSettings,
        visible: PixelRect,
    ) -> Result<Option<(PixelRect, egui::TextureHandle)>> {
        let fits = |length: u32| (1..=MAX_PREVIEW_PX).contains(&length);
        if !fits(visible.width()) || !fits(visible.height()) {
            return Ok(None);
        }
        let Some(key) = self.key(settings) else {
            bail!("no sliced stack to preview");
        };
        if let Some(((cached, region), handle)) = self.detail.as_ref()
            && *cached == key
            && region.contains(visible)
        {
            return Ok(Some((*region, handle.clone())));
        }

        let region = visible.padded(MAX_PREVIEW_PX, settings.width_px, settings.height_px);
        let (width, height) = (region.width(), region.height());
        let shown = self.full(settings)?;
        let image = tinted(
            &crop(&shown.runs, region.x0, region.y0, width, height),
            &crop(&shown.islands, region.x0, region.y0, width, height),
        );
        let handle = ctx.load_texture("layer-detail", image, egui::TextureOptions::NEAREST);
        self.detail = Some(((key, region), handle.clone()));
        Ok(Some((region, handle)))
    }

    /// Cuts the window the layer being shown falls in, unless it is the one already cut.
    /// A file being read has nothing to cut.
    fn cut_window(&mut self) -> Result<()> {
        let layer = self.layer;
        let stack = match self.source.as_mut() {
            Some(Source::Cut(stack)) => stack,
            Some(Source::Read(_)) => return Ok(()),
            None => bail!("no plate to preview"),
        };
        let wanted = stack.windows.window_of(layer);
        if stack
            .cut
            .as_ref()
            .is_some_and(|(cached, _)| *cached == wanted)
        {
            return Ok(());
        }

        let sliced = stack
            .windows
            .cut(&stack.mesh, wanted.clone())
            .context("cannot cut the layers being previewed")?;
        stack.cut = Some((wanted, sliced));
        Ok(())
    }

    /// The layer being shown as a texture, rasterised only when the layer, the scene or
    /// the panel has changed since the last frame.
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        settings: &RasterSettings,
    ) -> Result<egui::TextureHandle> {
        let Some(key) = self.key(settings) else {
            bail!("no sliced stack to preview");
        };
        if let Some((cached, handle)) = self.texture.as_ref()
            && *cached == key
        {
            return Ok(handle.clone());
        }

        let shown = self.full(settings)?;
        let image = tinted(
            &downsample(&shown.runs, MAX_PREVIEW_PX, MAX_PREVIEW_PX),
            &downsample(&shown.islands, MAX_PREVIEW_PX, MAX_PREVIEW_PX),
        );
        // Nearest, or a wall one pixel wide would be blurred away by the magnification
        // the panel is drawn at.
        let handle = ctx.load_texture("layer-preview", image, egui::TextureOptions::NEAREST);
        self.texture = Some((key, handle.clone()));
        Ok(handle)
    }

    fn current(&self) -> Option<&core_slicer::Layer> {
        let stack = self.source.as_ref()?.stack()?;
        let (window, sliced) = stack.cut.as_ref()?;
        sliced.layers.get(self.layer.checked_sub(window.start)?)
    }
}

/// A file's own fingerprint for the texture cache: its name, and what it was last written —
/// or, for bytes handed over, which bytes they are.
///
/// A file opened twice over an edit must not show the first one's cached picture.
fn file_fingerprint(file: &Handed) -> u64 {
    let mut hasher = DefaultHasher::new();
    let path = file.path();
    path.hash(&mut hasher);
    if let Handed::Bytes { bytes, .. } = file {
        bytes.as_ptr().hash(&mut hasher);
        bytes.len().hash(&mut hasher);
    } else if let Ok(meta) = std::fs::metadata(path) {
        meta.len().hash(&mut hasher);
        if let Ok(modified) = meta.modified() {
            modified.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// The mask with the pixels that stand on nothing picked out.
fn tinted(mask: &LayerMask, islands: &LayerMask) -> egui::ColorImage {
    let pixels = mask
        .pixels()
        .iter()
        .zip(islands.pixels())
        .map(|(&grey, &island)| theme::mask_pixel(grey, island > 0))
        .collect();
    egui::ColorImage::new([mask.width() as usize, mask.height() as usize], pixels)
}

/// The same panel read against the plate rather than against the machine: no panel
/// mirroring (ADR 0109), and the rows flipped, because a file starts at the near edge of
/// the plate while a picture starts at the far one (ADR 0134).
fn as_seen(settings: &RasterSettings) -> RasterSettings {
    RasterSettings {
        mirror_x: false,
        mirror_y: true,
        ..*settings
    }
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
        ] {
            value.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
    use core_raster::Grey;
    use core_raster::{PixelPitch, Shading};

    use crate::scene::{ImportSummary, Imported};

    /// Panel of 64 x 32 pixels at a 0.2 mm pitch: 12.8 x 6.4 mm of build area.
    fn settings() -> RasterSettings {
        RasterSettings {
            width_px: 64,
            height_px: 32,
            pitch: PixelPitch { x: 0.2, y: 0.2 },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        }
    }

    /// A preview of a 4 x 4 mm box `count` half-millimetre layers tall, with its first
    /// window already cut.
    fn preview_with_stack(count: usize) -> Preview {
        let mesh = Arc::new(box_mesh(count as Scalar * 0.5));
        let windows = Windows::new(
            &mesh,
            core_slicer::SliceSettings {
                layer_height: 0.5,
                ..core_slicer::SliceSettings::default()
            },
            core_slicer::WINDOW_LAYERS,
        )
        .expect("a box slices");
        let mut preview = Preview {
            source: Some(Source::Cut(Stack {
                mesh,
                windows,
                fingerprint: 7,
                cut: None,
            })),
            ..Preview::default()
        };
        preview.cut_window().expect("a box slices");
        preview
    }

    /// A 4 x 4 mm box `height` millimetres tall, standing on the plate.
    fn box_mesh(height: Scalar) -> Mesh {
        let corners = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(4.0, 4.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
            Vec3::new(0.0, 0.0, height),
            Vec3::new(4.0, 0.0, height),
            Vec3::new(4.0, 4.0, height),
            Vec3::new(0.0, 4.0, height),
        ];
        Mesh::new(
            corners.to_vec(),
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
        )
    }

    #[test]
    fn a_box_starting_in_mid_air_is_an_island_on_its_first_layer_only() {
        // One box on the plate, and one beside it from 1 mm up: layer 2 is its first.
        let mut mesh = box_mesh(2.0);
        let floating = box_mesh(1.0);
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend(
            floating
                .vertices
                .iter()
                .map(|v| Vec3::new(v.x + 6.0, v.y, v.z + 1.0)),
        );
        mesh.faces.extend(
            floating
                .faces
                .iter()
                .map(|face| face.map(|index| index + base)),
        );
        let windows = Windows::new(
            &mesh,
            core_slicer::SliceSettings {
                layer_height: 0.5,
                ..core_slicer::SliceSettings::default()
            },
            core_slicer::WINDOW_LAYERS,
        )
        .expect("two boxes slice");
        let mut preview = Preview {
            source: Some(Source::Cut(Stack {
                mesh: Arc::new(mesh),
                windows,
                fingerprint: 7,
                cut: None,
            })),
            ..Preview::default()
        };

        let mut island_pixels = |layer: usize| {
            preview.set_layer(layer);
            let shown = preview.full(&settings()).expect("the stack has layers");
            shown
                .islands
                .to_mask()
                .pixels()
                .iter()
                .filter(|p| **p > 0)
                .count()
        };
        assert_eq!(island_pixels(0), 0, "the plate holds the first layer");
        assert_eq!(island_pixels(1), 0, "the floating box has not started");
        // 4 x 4 mm at 0.2 mm a pixel.
        assert_eq!(
            island_pixels(2),
            400,
            "the floating box starts over nothing"
        );
        assert_eq!(island_pixels(3), 0, "it then stands on itself");
    }

    fn tetrahedron() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        )
    }

    fn scene_with_a_model() -> Scene {
        let mesh = tetrahedron();
        let mut scene = Scene::default();
        scene.insert(crate::scene::Imported::new(
            "tetra".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
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
        scene
    }

    #[test]
    fn stepping_inside_one_window_cuts_nothing_again() {
        // Cutting is what a preview must not do on every frame; the window it is showing
        // is kept. See ADR 0068.
        let mut preview = preview_with_stack(4);
        let before = preview.layer_z();
        preview.set_layer(2);
        preview.cut_window().expect("a box slices");

        assert_eq!(preview.layer_count(), 4);
        assert_ne!(preview.layer_z(), before);
        assert!(preview.layer_area_mm2().is_some_and(|area| area > 15.0));
    }

    #[test]
    fn an_empty_preview_has_no_layers_to_show() {
        let mut preview = Preview::default();
        assert_eq!(preview.layer_count(), 0);
        assert_eq!(preview.layer_z(), None);
        assert!(!preview.is_building());
        assert!(preview.mask(&settings()).is_err());
    }

    #[test]
    fn the_slider_never_leaves_the_stack() {
        let mut preview = preview_with_stack(4);
        preview.set_layer(99);
        assert_eq!(preview.layer(), 3, "the last layer of four");
        assert_eq!(
            preview.layer_z(),
            Some(2.0),
            "the top of the fourth half-millimetre layer"
        );
        // Every layer of the stack is the same 4 x 4 mm square.
        let area = preview.layer_area_mm2().expect("the stack has layers");
        assert!((area - 16.0).abs() < 1e-3, "expected 16 mm^2, got {area}");
    }

    #[test]
    fn a_shorter_stack_pulls_the_slider_back() {
        let mut preview = preview_with_stack(8);
        preview.set_layer(7);

        let scene = scene_with_a_model();
        preview
            .build(&scene, Cutting::uniform(0.5))
            .expect("the plate has a model on it");
        let mut status = Status::default();
        while preview.poll(&mut status) {
            std::thread::yield_now();
        }

        // One millimetre of tetrahedron at half a millimetre a layer: two layers.
        assert_eq!(preview.layer_count(), 2);
        assert_eq!(preview.layer(), 1);
        assert_eq!(status, Status::Idle, "a build that worked says nothing");
    }

    #[test]
    fn a_layer_is_rasterised_at_the_panels_aspect_and_shrunk_to_fit() {
        let mut preview = preview_with_stack(2);
        let mask = preview.mask(&settings()).expect("the stack has layers");

        // The panel is smaller than the preview limit, so it is shown pixel for pixel.
        assert_eq!((mask.width(), mask.height()), (64, 32));
        // A 4 x 4 mm square at a 0.2 mm pitch: 20 x 20 pixels.
        assert!(
            (mask.coverage() - 400.0).abs() < 1.0,
            "expected about 400 lit pixels, got {}",
            mask.coverage()
        );
    }

    #[test]
    fn a_height_from_the_prepare_mode_parks_the_slider_on_the_nearest_layer() {
        // Four layers of half a millimetre, topping out at 0.5, 1.0, 1.5 and 2.0.
        let mut preview = preview_with_stack(4);
        preview.show_height(Some(1.3));
        assert_eq!(preview.layer(), 2);
        preview.show_height(Some(-5.0));
        assert_eq!(preview.layer(), 0, "below the stack is its first layer");
        preview.show_height(Some(40.0));
        assert_eq!(preview.layer(), 3, "above it, its last");
        preview.show_height(None);
        assert_eq!(preview.layer(), 3, "an uncut model is the top of the stack");
    }

    #[test]
    fn the_layer_shown_goes_back_to_the_prepare_mode_as_its_height() {
        let mut preview = preview_with_stack(4);
        preview.set_layer(1);
        assert_eq!(preview.prepare_height(), Some(Some(1.0)));
        preview.set_layer(3);
        assert_eq!(
            preview.prepare_height(),
            Some(None),
            "the top layer cuts nothing"
        );
        assert_eq!(
            Preview::default().prepare_height(),
            None,
            "no stack, no say"
        );
    }

    #[test]
    fn a_view_zoomed_in_far_enough_is_drawn_from_the_panels_own_pixels() {
        let mut preview = preview_with_stack(2);
        let ctx = egui::Context::default();
        let visible = PixelRect {
            x0: 10,
            y0: 5,
            x1: 30,
            y1: 20,
        };
        let (region, texture) = preview
            .detail(&ctx, &settings(), visible)
            .expect("the stack has layers")
            .expect("twenty pixels fit a texture");
        assert!(region.contains(visible));
        assert_eq!(
            region,
            PixelRect {
                x0: 0,
                y0: 0,
                x1: 40,
                y1: 27
            },
            "grown by half each way, and cut at the panel's edge"
        );
        assert_eq!(texture.size(), [40, 27]);

        let too_much = PixelRect {
            x0: 0,
            y0: 0,
            x1: MAX_PREVIEW_PX + 1,
            y1: 1,
        };
        assert!(
            preview
                .detail(&ctx, &settings(), too_much)
                .expect("no error")
                .is_none()
        );
    }

    #[test]
    fn zooming_keeps_the_pixel_under_the_cursor_where_it_was() {
        let fit = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 320.0));
        let panel = egui::vec2(64.0, 32.0);
        let mut view = MaskView::default();
        let cursor = egui::pos2(100.0, 50.0);
        let before = view.pixel_at(cursor, fit, panel);
        view.zoom_at(4.0, cursor, fit, panel);
        assert!((view.pixel_at(cursor, fit, panel) - before).length() < 1e-3);
        assert_eq!(view.zoom, 4.0);
    }

    #[test]
    fn a_view_cannot_be_zoomed_out_past_the_whole_panel_or_dragged_off_it() {
        let fit = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 320.0));
        let panel = egui::vec2(64.0, 32.0);
        let mut view = MaskView::default();
        view.zoom_at(0.1, egui::pos2(10.0, 10.0), fit, panel);
        assert_eq!(view.zoom, 1.0);
        view.zoom_at(2.0, egui::pos2(320.0, 160.0), fit, panel);
        view.pan(egui::vec2(1.0e6, 1.0e6), fit, panel);
        let corner = view.pixel_at(fit.center(), fit, panel);
        assert_eq!(
            (corner.x, corner.y),
            (0.0, 0.0),
            "the middle stops at the panel's corner"
        );
    }

    #[test]
    fn a_mirrored_panel_leaves_the_preview_the_way_round_the_plate_is() {
        let mut preview = preview_with_stack(2);
        let mask = preview
            .mask(&RasterSettings {
                mirror_x: true,
                ..settings()
            })
            .expect("the stack has layers");

        let half = mask.width() as usize / 2;
        let (left, right) = mask.pixels().chunks(mask.width() as usize).fold(
            (0usize, 0usize),
            |(left, right), row| {
                let lit = |part: &[u8]| part.iter().filter(|&&pixel| pixel > 0).count();
                (left + lit(&row[..half]), right + lit(&row[half..]))
            },
        );
        // The box stands over x = 0..4 mm of a 12.8 mm panel, so it is left of centre.
        assert!(left > 0, "the layer is drawn");
        assert_eq!(right, 0, "the machine's mirroring stays out of the picture");
    }

    #[test]
    fn the_near_edge_of_the_plate_is_drawn_at_the_bottom_of_the_picture() {
        let mut preview = preview_with_stack(2);
        let mask = preview.mask(&settings()).expect("the stack has layers");

        let width = mask.width() as usize;
        let lit = |row: &[u8]| row.iter().any(|&pixel| pixel > 0);
        let rows: Vec<usize> = mask
            .pixels()
            .chunks(width)
            .enumerate()
            .filter(|(_, row)| lit(row))
            .map(|(y, _)| y)
            .collect();

        // The box covers y = 0..4 mm of a 6.4 mm panel, 32 rows at a 0.2 mm pitch: the
        // near edge of the plate, which the viewport draws nearest the viewer.
        assert_eq!(rows.first().copied(), Some(12), "the far end of the box");
        assert_eq!(
            rows.last().copied(),
            Some(31),
            "and the near edge of the plate"
        );
    }

    #[test]
    fn a_stack_cut_from_another_scene_is_stale() {
        let preview = preview_with_stack(2);
        assert!(!preview.is_stale(7));
        assert!(preview.is_stale(8));
        assert!(
            !Preview::default().is_stale(8),
            "nothing sliced yet is not stale, it is empty"
        );
    }

    #[test]
    fn previewing_an_empty_plate_starts_nothing() {
        let mut preview = Preview::default();
        assert!(
            preview
                .build(&Scene::default(), Cutting::uniform(0.5))
                .is_err()
        );
        assert!(!preview.is_building());
    }

    #[test]
    fn moving_or_hiding_a_model_changes_the_fingerprint() {
        let mut scene = scene_with_a_model();
        let placed = stack_fingerprint(&scene, Cutting::uniform(0.05));

        scene.select(None);
        assert_eq!(
            stack_fingerprint(&scene, Cutting::uniform(0.05)),
            placed,
            "selection does not change the stack"
        );

        assert_ne!(
            stack_fingerprint(&scene, Cutting::uniform(0.1)),
            placed,
            "the layer height changes the stack"
        );

        scene.objects_mut()[0].transform.translation = Vec3::new(1.0, 0.0, 0.0);
        let moved = stack_fingerprint(&scene, Cutting::uniform(0.05));
        assert_ne!(moved, placed, "placement changes the stack");

        scene.objects_mut()[0].visible = false;
        assert_ne!(
            stack_fingerprint(&scene, Cutting::uniform(0.05)),
            moved,
            "hiding a model changes the stack"
        );
    }

    #[test]
    fn drilling_a_hole_or_clearing_it_changes_the_fingerprint() {
        let mut scene = scene_with_a_model();
        let solid = stack_fingerprint(&scene, Cutting::uniform(0.05));

        let object = &mut scene.objects_mut()[0];
        let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
        let size = core_volume::HoleSize {
            diameter_mm: 0.2,
            depth_mm: 0.2,
            taper: 1.0,
        };
        let point = mesh.vertices[0];
        let transform = object.transform;
        object
            .hollow
            .add_drain(&mesh, &bvh, point, Vec3::NEG_Z, size, transform);
        let drilled = stack_fingerprint(&scene, Cutting::uniform(0.05));
        assert_ne!(drilled, solid, "a hole changes what the layers hold");

        scene.objects_mut()[0].hollow.clear_drains();
        assert_ne!(
            stack_fingerprint(&scene, Cutting::uniform(0.05)),
            drilled,
            "and so does taking it back out"
        );
    }

    #[test]
    fn a_texture_is_drawn_once_per_layer() {
        let ctx = egui::Context::default();
        let mut preview = preview_with_stack(4);

        let first = preview
            .texture(&ctx, &settings())
            .expect("the stack has layers");
        let again = preview
            .texture(&ctx, &settings())
            .expect("the stack has layers");
        assert_eq!(first.id(), again.id(), "the same layer is not redrawn");

        preview.set_layer(2);
        let moved = preview
            .texture(&ctx, &settings())
            .expect("the stack has layers");
        assert_ne!(moved.id(), first.id(), "another layer is another texture");
        assert_eq!(moved.size(), [64, 32]);
    }

    #[test]
    fn a_cancelled_build_leaves_the_preview_alone() {
        let mut preview = preview_with_stack(2);
        preview
            .build(&scene_with_a_model(), Cutting::uniform(0.5))
            .expect("the plate has a model on it");
        assert!(preview.is_building());

        preview.cancel();
        assert!(!preview.is_building());
        assert_eq!(preview.layer_count(), 2, "the old stack is still shown");
    }

    /// A `.goo` of `count` layers on a 16 x 8 panel with a lit block in the middle, written
    /// to a temporary file so it can be opened by path the way the window opens one.
    pub(crate) fn written_goo(name: &str, count: u32) -> PathBuf {
        use core_format::{LayerSink, PrintJob, SlicedFileWriter};
        use printer_profiles::{MaterialProfile, PrinterProfile};

        let printer = PrinterProfile::from_toml_str(
            r#"
name = "Read Back"
manufacturer = "Test"

[display]
width_px = 16
height_px = 8
width_mm = 1.6
height_mm = 0.8

[build_volume]
x = 1.6
y = 0.8
z = 10.0
"#,
            Path::new("inline.toml"),
        )
        .expect("the inline profile is valid");

        let raster = RasterSettings {
            width_px: 16,
            height_px: 8,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            ..settings()
        };
        let job = PrintJob {
            printer,
            material: MaterialProfile {
                layer_height_mm: 0.05,
                ..MaterialProfile::default()
            },
            raster,
            plan: LayerPlan::of_count(0.05, count as usize),
            volume_mm3: 0.0,
            exposure: core_format::ExposurePlan::default(),
            thumbnail: None,
            created_unix_s: 0,
        };

        let mut mask = LayerMask::new(16, 8);
        let pixels = mask.pixels_mut();
        for y in 2..6u32 {
            for x in 4..12u32 {
                pixels[(y * 16 + x) as usize] = 255;
            }
        }
        let runs = LayerRuns::from_mask(&mask);

        let path = std::env::temp_dir().join(format!("encrust-preview-{name}.goo"));
        let file = std::fs::File::create(&path).expect("a temporary file");
        let mut buffered = std::io::BufWriter::new(file);
        {
            let mut sink = format_goo::GooWriter
                .begin(&job, &mut buffered)
                .expect("the job matches the panel");
            for _ in 0..count {
                sink.push(format_goo::GooSink::encode(&runs))
                    .expect("a layer");
            }
            sink.finish(1234.0).expect("every promised layer arrived");
        }
        path
    }

    #[test]
    fn an_opened_file_reports_what_it_states_and_not_what_a_profile_would() {
        let path = written_goo("states", 4);
        let mut preview = Preview::default();
        preview
            .read_file(&Handed::Path(path.clone()), 0)
            .expect("a .goo we wrote opens");

        let facts = preview.read_facts().expect("a file is open");
        assert_eq!(facts.format, "goo");
        assert_eq!(facts.layer_count(), 4);
        assert_eq!((facts.width_px, facts.height_px), (16, 8));
        assert_eq!(preview.layer_count(), 4);
        assert_eq!(
            preview.layer(),
            3,
            "a file opens at its top layer, as a cut stack does"
        );

        // The panel is the file's, whatever profile the window holds.
        let panel = preview.read_panel().expect("a .goo records its panel");
        assert_eq!((panel.width_px, panel.height_px), (16, 8));
        assert!((panel.pitch.x - 0.1).abs() < 1e-6);
    }

    #[test]
    fn a_layer_of_an_opened_file_is_decoded_rather_than_cut() {
        let path = written_goo("decode", 3);
        let mut preview = Preview::default();
        preview
            .read_file(&Handed::Path(path.clone()), 0)
            .expect("a .goo we wrote opens");
        let panel = preview.read_panel().expect("a .goo records its panel");

        preview.set_layer(1);
        let mask = preview.mask(&panel).expect("the layer decodes");
        assert_eq!((mask.width(), mask.height()), (16, 8));

        let lit = mask.pixels().iter().filter(|grey| **grey > 0).count();
        assert_eq!(lit, 8 * 4, "the block written into every layer comes back");

        // Area is the mask's own, because a file has no contours to take it from.
        let area = preview
            .layer_area_mm2()
            .expect("a decoded layer has an area");
        assert!(
            (area - 32.0 * 0.01).abs() < 1e-4,
            "32 lit pixels of 0.1 by 0.1 mm, got {area}"
        );
    }

    #[test]
    fn an_opened_file_is_never_stale_against_the_scene_and_states_its_own_heights() {
        let path = written_goo("heights", 4);
        let mut preview = Preview::default();
        preview
            .read_file(&Handed::Path(path.clone()), 0)
            .expect("a .goo we wrote opens");

        assert!(
            !preview.is_stale(0),
            "nothing about the plate makes an opened file out of date"
        );
        assert!(
            preview.plan().is_none(),
            "a container carries a layer table, not a plan we cut"
        );

        preview.set_layer(0);
        let first = preview
            .layer_z()
            .expect("the table states every layer's top");
        preview.set_layer(3);
        let last = preview
            .layer_z()
            .expect("the table states every layer's top");
        assert!((first - 0.05).abs() < 1e-4, "got {first}");
        assert!((last - 0.2).abs() < 1e-4, "got {last}");
    }

    /// BUG-51: importing a model while a file is open left the panel, the slider and the
    /// footer stating two stacks at once.
    #[test]
    fn a_file_is_over_an_old_plate_once_what_stands_on_it_changes() {
        let path = written_goo("plate", 2);
        let mut scene = Scene::default();
        let mut preview = Preview::default();
        preview
            .read_file(&Handed::Path(path.clone()), plate_fingerprint(&scene))
            .expect("a .goo we wrote opens");
        assert!(
            !preview.file_is_over_an_old_plate(plate_fingerprint(&scene)),
            "the plate it was opened over is the plate it is shown over"
        );

        scene.insert(Imported::new(
            "box".to_owned(),
            Arc::new(box_mesh(2.0)),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&box_mesh(2.0)),
            },
        ));
        assert!(preview.file_is_over_an_old_plate(plate_fingerprint(&scene)));
    }

    /// The exposure of a file's layer is the file's own, not the window resin's ramp.
    #[test]
    fn an_opened_file_states_its_own_exposure_for_the_layer_being_shown() {
        let path = written_goo("exposure", 3);
        let mut preview = Preview::default();
        preview
            .read_file(&Handed::Path(path.clone()), 0)
            .expect("a .goo we wrote opens");
        let facts = preview.read_facts().expect("a file is open").clone();

        preview.set_layer(2);
        let exposure_s = preview
            .read_layer_exposure_s()
            .expect("the table states every layer's exposure");
        assert!(
            (exposure_s - facts.layers[2].exposure_s).abs() < 1e-6,
            "the row of the layer being shown, got {exposure_s}"
        );
        assert_eq!(
            Preview::default().read_layer_exposure_s(),
            None,
            "a plate under the slider has no file to read"
        );
    }

    #[test]
    fn closing_a_file_leaves_nothing_of_it_behind() {
        let path = written_goo("close", 2);
        let mut preview = Preview::default();
        preview
            .read_file(&Handed::Path(path.clone()), 0)
            .expect("a .goo we wrote opens");
        preview.close_file();

        assert!(preview.read_facts().is_none());
        assert!(preview.read_path().is_none());
        assert_eq!(preview.layer_count(), 0);
    }
}
