use std::num::NonZeroU8;

use core_geometry::{Mesh, Scalar};

use crate::engine::{PlaneSliceEngine, SliceEngine, layer_heights};
use crate::{Layer, LayerPlan, SliceError, SliceSettings, Sliced};

/// Layers cut in one go before their contours are handed on and dropped.
///
/// Shorter holds less and rebuilds the face index more often; see
/// `docs/decisions/0066-a-stack-is-sliced-a-window-at-a-time.md`.
pub const WINDOW_LAYERS: usize = 64;

/// A slicing run that knows its layer heights but has cut none of them, so a caller can
/// take the stack a window at a time and hold none of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Windows {
    settings: SliceSettings,
    plan: LayerPlan,
    heights: Vec<Scalar>,
    window: usize,
}

impl Windows {
    pub fn new(mesh: &Mesh, settings: SliceSettings, window: usize) -> Result<Self, SliceError> {
        // Asking for the heights first is what rejects a zero layer height and an empty
        // mesh, which a plan built from bounds alone could not.
        let heights = layer_heights(mesh, &settings)?;
        let aabb = mesh.aabb().ok_or(SliceError::EmptyMesh)?;
        Ok(Self {
            settings,
            plan: LayerPlan::uniform(&settings, aabb.mins.z, aabb.maxs.z),
            heights,
            window: window.max(1),
        })
    }

    /// A run over a plan whose layers need not be the same thickness.
    pub fn planned(plan: LayerPlan, samples: NonZeroU8, window: usize) -> Self {
        Self {
            settings: SliceSettings {
                layer_height: plan.nominal_thickness(),
                samples,
            },
            heights: plan.planes(),
            plan,
            window: window.max(1),
        }
    }

    pub fn settings(&self) -> SliceSettings {
        self.settings
    }

    /// Where every layer of this run starts and stops.
    pub fn plan(&self) -> &LayerPlan {
        &self.plan
    }

    pub fn layer_count(&self) -> usize {
        self.heights.len()
    }

    /// Height of one layer, millimetres above the plate.
    pub fn height_of(&self, layer: usize) -> Option<Scalar> {
        self.heights.get(layer).copied()
    }

    /// The window `layer` falls in, as the range of layer indices it covers.
    pub fn window_of(&self, layer: usize) -> std::ops::Range<usize> {
        let first = layer / self.window * self.window;
        first..(first + self.window).min(self.heights.len())
    }

    /// Every plane sampled inside layer `layer`'s band, the one nearest the middle first.
    ///
    /// `samples` planes split the band into that many equal parts and take the middle of
    /// each. The one nearest the band's own middle leads, because that is the cross-section
    /// everything but the rasteriser reads; see `docs/design/slicing.md`.
    fn planes_of(&self, layer: usize) -> Vec<Scalar> {
        let samples = usize::from(self.settings.samples.get());
        let Some((bottom, top)) = self.plan.band_of(layer) else {
            return self.height_of(layer).into_iter().collect();
        };
        if samples == 1 {
            return self.height_of(layer).into_iter().collect();
        }

        let middle = Scalar::midpoint(bottom, top);
        let mut planes: Vec<Scalar> = (0..samples)
            .map(|part| {
                let fraction = (part as Scalar + 0.5) / samples as Scalar;
                (top - bottom).mul_add(fraction, bottom)
            })
            .collect();
        planes.sort_unstable_by(|a, b| {
            (a - middle)
                .abs()
                .total_cmp(&(b - middle).abs())
                .then(a.total_cmp(b))
        });
        planes
    }

    /// Cuts one range of layers, which need not be a whole window.
    pub fn cut(&self, mesh: &Mesh, layers: std::ops::Range<usize>) -> Result<Sliced, SliceError> {
        if self.settings.samples == crate::ONE_SAMPLE {
            let heights = self.heights.get(layers).unwrap_or(&[]);
            return PlaneSliceEngine.slice_at(mesh, heights);
        }

        let layers = layers.start..layers.end.min(self.heights.len());
        let planes: Vec<Scalar> = layers.clone().flat_map(|l| self.planes_of(l)).collect();
        let sliced = PlaneSliceEngine.slice_at(mesh, &planes)?;
        Ok(self.fold(sliced, layers))
    }

    /// Collapses one group of sampled planes per layer into one layer, at the height the
    /// plate will actually stand at.
    fn fold(&self, sliced: Sliced, layers: std::ops::Range<usize>) -> Sliced {
        let samples = usize::from(self.settings.samples.get());
        let mut folded = Sliced {
            layers: Vec::with_capacity(layers.len()),
            open_contours: sliced.open_contours,
            degenerate_contours: sliced.degenerate_contours,
            unlinked_segments: sliced.unlinked_segments,
        };
        let mut planes = sliced.layers.into_iter();
        for layer in layers {
            let mut group: Vec<Layer> = planes.by_ref().take(samples).collect();
            if group.is_empty() {
                break;
            }
            let first = group.remove(0);
            folded.layers.push(Layer {
                z: self.height_of(layer).unwrap_or(first.z),
                contours: first.contours,
                extra: group.into_iter().map(|plane| plane.contours).collect(),
            });
        }
        folded
    }

    /// Cuts window by window in print order, handing each one over before the next.
    pub fn stream<E>(
        &self,
        mesh: &Mesh,
        mut each: impl FnMut(&Sliced) -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<SliceError>,
    {
        let mut first = 0;
        while first < self.heights.len() {
            let last = (first + self.window).min(self.heights.len());
            each(&self.cut(mesh, first..last)?)?;
            first = last;
        }
        Ok(())
    }
}
