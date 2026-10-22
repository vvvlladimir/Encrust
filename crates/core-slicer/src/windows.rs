use std::num::NonZeroU8;

use core_geometry::{Mesh, Scalar};

use crate::engine::{layer_heights_under, on_the_plate};
use crate::{Layer, LayerPlan, SliceError, SliceSettings, Sliced, Source};

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
        Self::under(mesh, settings, window, Scalar::INFINITY)
    }

    /// A run that plans nothing above `ceiling_mm`, for a mesh carrying bodies that only
    /// subtract; see [`crate::layer_heights_under`].
    pub fn under(
        mesh: &Mesh,
        settings: SliceSettings,
        window: usize,
        ceiling_mm: Scalar,
    ) -> Result<Self, SliceError> {
        // Asking for the heights first is what rejects a zero layer height and an empty
        // mesh, which a plan built from bounds alone could not.
        let heights = layer_heights_under(mesh, &settings, ceiling_mm)?;
        let (z_min, z_max) = on_the_plate(mesh, ceiling_mm)?;
        Ok(Self {
            settings,
            plan: LayerPlan::uniform(&settings, z_min, z_max),
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

    /// The highest plane sampled inside layer `layer`'s band: a section drawn there shows
    /// nothing the layer's mask lacks, and with one sample it is the mask's own plane.
    pub fn section_plane_of(&self, layer: usize) -> Option<Scalar> {
        self.planes_of(layer).into_iter().reduce(Scalar::max)
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
    pub fn cut(
        &self,
        source: &(impl Source + ?Sized),
        layers: std::ops::Range<usize>,
    ) -> Result<Sliced, SliceError> {
        if self.settings.samples == crate::ONE_SAMPLE {
            let heights = self.heights.get(layers).unwrap_or(&[]);
            return source.slice_at(heights);
        }

        let layers = layers.start..layers.end.min(self.heights.len());
        let planes: Vec<Scalar> = layers.clone().flat_map(|l| self.planes_of(l)).collect();
        let sliced = source.slice_at(&planes)?;
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
        source: &(impl Source + ?Sized),
        mut each: impl FnMut(&Sliced) -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<SliceError>,
    {
        let mut first = 0;
        while first < self.heights.len() {
            let last = (first + self.window).min(self.heights.len());
            each(&self.cut(source, first..last)?)?;
            first = last;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(samples: u8) -> Windows {
        let plan = LayerPlan::of_count(0.05, 10);
        let samples = NonZeroU8::new(samples).expect("a run samples at least once");
        Windows::planned(plan, samples, WINDOW_LAYERS)
    }

    #[test]
    fn one_sample_is_sectioned_at_the_middle_of_its_layer() {
        let plane = run(1).section_plane_of(3).expect("layer 3 is planned");
        assert!(
            (plane - 0.175).abs() < 1e-6,
            "layer 3 spans 0.15..0.2, so its one plane is 0.175, got {plane}"
        );
    }

    #[test]
    fn several_samples_are_sectioned_at_the_highest() {
        let plane = run(4).section_plane_of(3).expect("layer 3 is planned");
        assert!(
            (plane - 0.19375).abs() < 1e-6,
            "four parts of 0.15..0.2 are sampled at their middles, the highest 0.19375, \
             got {plane}"
        );
    }
}
