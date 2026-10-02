use std::num::NonZeroU8;

use anyhow::{Context, Result};
use core_geometry::{Mesh, Scalar};
use core_slicer::{AdaptiveSettings, LayerPlan, SliceSettings, Sliced, Windows, adaptive_plan};

pub use core_slicer::WINDOW_LAYERS as SLICE_WINDOW_LAYERS;

/// A slicing run over one mesh: the windows to cut, and the mesh to cut them from.
pub struct Plan<'m> {
    mesh: &'m Mesh,
    windows: Windows,
}

impl<'m> Plan<'m> {
    pub fn new(
        mesh: &'m Mesh,
        layer_height: Scalar,
        samples: NonZeroU8,
        window: usize,
    ) -> Result<Self> {
        let settings = SliceSettings {
            layer_height,
            samples,
        };
        let windows = Windows::new(mesh, settings, window).context("cannot slice the model")?;
        Ok(Self { mesh, windows })
    }

    /// A run whose layers are as thick as the surface running through them allows.
    pub fn adaptive(
        mesh: &'m Mesh,
        settings: &AdaptiveSettings,
        samples: NonZeroU8,
        window: usize,
    ) -> Result<Self> {
        let plan = adaptive_plan(mesh, settings).context("cannot plan the layers")?;
        Ok(Self {
            mesh,
            windows: Windows::planned(plan, samples, window),
        })
    }

    pub fn settings(&self) -> SliceSettings {
        self.windows.settings()
    }

    /// The mesh this run cuts, standing in plate coordinates.
    pub fn mesh(&self) -> &Mesh {
        self.mesh
    }

    pub fn windows(&self) -> &Windows {
        &self.windows
    }

    /// Where every layer of this run starts and stops.
    pub fn layers(&self) -> &LayerPlan {
        self.windows.plan()
    }

    /// Slices window by window in print order, handing each window over before the next
    /// one is cut.
    pub fn stream(&self, mut each: impl FnMut(&Sliced) -> Result<()>) -> Result<()> {
        self.windows.stream(self.mesh, |sliced| each(sliced))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;
    use core_slicer::ONE_SAMPLE;

    /// A unit cube standing on the plate, as twelve triangles.
    fn cube() -> Mesh {
        let corners = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
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
    fn every_layer_comes_through_exactly_once_whatever_the_window() {
        let mesh = cube();
        let whole = Plan::new(&mesh, 0.1, ONE_SAMPLE, 1024).expect("a cube slices");
        let chopped = Plan::new(&mesh, 0.1, ONE_SAMPLE, 3).expect("a cube slices");

        let collect = |plan: &Plan| {
            let mut heights = Vec::new();
            plan.stream(|sliced| {
                heights.extend(sliced.layers.iter().map(|layer| layer.z));
                Ok(())
            })
            .expect("a cube slices");
            heights
        };

        let one = collect(&whole);
        assert_eq!(one.len(), 10, "a 1 mm cube at 0.1 mm is ten layers");
        assert_eq!(one, collect(&chopped));
    }

    #[test]
    fn a_window_holds_at_most_its_own_layers() {
        let mesh = cube();
        let plan = Plan::new(&mesh, 0.1, ONE_SAMPLE, 3).expect("a cube slices");

        let mut widest = 0;
        plan.stream(|sliced| {
            widest = widest.max(sliced.layers.len());
            Ok(())
        })
        .expect("a cube slices");

        assert_eq!(widest, 3);
    }
}
