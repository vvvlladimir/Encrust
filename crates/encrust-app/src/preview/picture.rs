//! The picture of one layer: what is rasterised, what is cached, and the part of the
//! panel the view is zoomed into (ADR 0023).

use anyhow::{Result, bail};
use core_analysis::{Measured, erase, island_runs};
use core_format::OpenFile;
use core_raster::{
    LayerMask, LayerRuns, RasterSettings, Rasterizer, ScanlineRasterizer, crop, downsample,
    shrink_factor,
};

use crate::ui::theme;

use super::*;

/// Widest preview mask handed to egui, pixels. A 5760 x 3600 panel is 83 MB once it has
/// become RGBA, which no slider drag can upload a frame of; see
/// `docs/decisions/0023-preview-masks-are-downsampled.md`.
pub const MAX_PREVIEW_PX: u32 = 2048;

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

    pub(super) fn contains(self, other: Self) -> bool {
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
pub(super) const MOST_POINTS_A_PIXEL: f32 = 48.0;

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

/// The mask with the pixels that stand on nothing picked out.
pub(super) fn tinted(mask: &LayerMask, islands: &LayerMask) -> egui::ColorImage {
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
pub(super) fn as_seen(settings: &RasterSettings) -> RasterSettings {
    RasterSettings {
        mirror_x: false,
        mirror_y: true,
        ..*settings
    }
}

/// Where on the plate a risk measured on this panel stands, `at_mm` being millimetres from
/// the first pixel of the picture `as_seen` lays out.
pub fn on_plate(settings: &RasterSettings, at_mm: [f32; 2]) -> core_geometry::Vec2 {
    let seen = as_seen(settings);
    let across = |mirrored: bool, at: f32, px: u32, pitch: f32| {
        if mirrored { px as f32 * pitch - at } else { at }
    };
    core_geometry::Vec2::new(
        across(seen.mirror_x, at_mm[0], seen.width_px, seen.pitch.x),
        across(seen.mirror_y, at_mm[1], seen.height_px, seen.pitch.y),
    )
}

impl Preview {
    /// Rasterises the layer being shown, cutting its window first if that has not
    /// happened yet, and shrinks it to something a screen can hold.
    #[cfg(test)]
    pub fn mask(&mut self, settings: &RasterSettings) -> Result<LayerMask> {
        let shown = self.full(settings)?;
        Ok(downsample(&shown.runs, MAX_PREVIEW_PX, MAX_PREVIEW_PX))
    }

    /// The layer being shown at the panel's own resolution, rasterised once for each
    /// layer, scene and panel, with the islands on it against the layer below.
    pub(super) fn full(&mut self, settings: &RasterSettings) -> Result<&Shown> {
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
    pub(super) fn runs_of(&mut self, index: usize, settings: &RasterSettings) -> Result<LayerRuns> {
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
    pub(super) fn rasterised(&self, index: usize, settings: &RasterSettings) -> Result<LayerRuns> {
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
                    .cut(stack.solid.as_ref(), index..index + 1)
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

    pub(super) fn key(&self, settings: &RasterSettings) -> Option<TextureKey> {
        Some(TextureKey {
            fingerprint: self.source.as_ref()?.fingerprint(),
            layer: self.layer,
            settings: *settings,
            cleaned: self.cleaned(settings).is_some(),
        })
    }

    /// The measurement that took islands out of this stack, once there is one.
    pub(super) fn cleaned(&self, settings: &RasterSettings) -> Option<&Measured> {
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

    pub(super) fn current(&self) -> Option<&core_slicer::Layer> {
        let stack = self.source.as_ref()?.stack()?;
        let (window, sliced) = stack.cut.as_ref()?;
        sliced.layers.get(self.layer.checked_sub(window.start)?)
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
}

#[cfg(test)]
mod tests {
    use core_analysis::RiskKind;
    use core_geometry::{Mesh, Vec2, Vec3};
    use core_raster::{Grey, PixelPitch, Shading};
    use core_slicer::{ONE_SAMPLE, SliceSettings, WINDOW_LAYERS};

    use super::*;
    use crate::job::{MeasureJob, MeasureOutcome};

    /// Twelve facets of the box from `min` to `max`, appended to `vertices` and `faces`.
    fn add_box(vertices: &mut Vec<Vec3>, faces: &mut Vec<[u32; 3]>, min: Vec3, max: Vec3) {
        let base = vertices.len() as u32;
        for index in 0..8 {
            let pick =
                |bit: usize, low: f32, high: f32| if index >> bit & 1 == 0 { low } else { high };
            vertices.push(Vec3::new(
                pick(0, min.x, max.x),
                pick(1, min.y, max.y),
                pick(2, min.z, max.z),
            ));
        }
        let quads = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        for [a, b, c, d] in quads {
            faces.push([base + a, base + b, base + c]);
            faces.push([base + a, base + c, base + d]);
        }
    }

    /// BUG: Support every island took the island back to the plate through the machine's
    /// mirrors rather than the picture's, so every island landed across the plate.
    #[test]
    fn an_island_measured_on_a_mirrored_panel_is_found_where_it_stands() {
        let (mut vertices, mut faces) = (Vec::new(), Vec::new());
        add_box(
            &mut vertices,
            &mut faces,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 2.0),
        );
        // Off the panel's middle on both axes, so a flip on either shows.
        add_box(
            &mut vertices,
            &mut faces,
            Vec3::new(2.0, 2.6, 1.0),
            Vec3::new(3.0, 3.6, 2.0),
        );
        let mesh = Mesh::new(vertices, faces);
        let slicing = SliceSettings {
            layer_height: 0.1,
            samples: ONE_SAMPLE,
        };
        let windows = Windows::new(&mesh, slicing, WINDOW_LAYERS).expect("two boxes slice");
        let settings = RasterSettings {
            width_px: 200,
            height_px: 200,
            pitch: PixelPitch { x: 0.02, y: 0.02 },
            mirror_x: true,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        };
        let mut job = MeasureJob::spawn(
            Arc::new(mesh.into()),
            windows,
            as_seen(&settings),
            Tolerance::default(),
            Measured::new(1),
        );
        let measured = loop {
            match job.poll() {
                Some(MeasureOutcome::Measured(measured)) => break measured,
                Some(other) => panic!("two boxes measure: {other:?}"),
                None => std::thread::yield_now(),
            }
        };
        let island = measured
            .risks()
            .into_iter()
            .find(|risk| matches!(risk.kind, RiskKind::Island { .. }))
            .expect("the box standing on nothing is an island");
        let found = on_plate(&settings, island.at_mm);
        assert!(
            (found - Vec2::new(2.5, 3.1)).length() < 0.05,
            "the floating box is centred at (2.5, 3.1), found {found}"
        );
    }
}
