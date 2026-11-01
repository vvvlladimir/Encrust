//! The models' pipelines, each built opaque and clipped under the light it is drawn in; see
//! `docs/design/viewport.md`.

use super::*;

/// A model pipeline in the two forms a draw picks between: opaque, for a surface nothing cuts
/// away or shows through, and clipped, which blends and may discard.
pub(in super::super) struct Shaded {
    pub(in super::super) opaque: Facing,
    pub(in super::super) clipped: Facing,
}

impl Shaded {
    pub(in super::super) fn of(&self, opaque: bool) -> &Facing {
        match opaque {
            true => &self.opaque,
            false => &self.clipped,
        }
    }
}

/// A pipeline under the studio's light, built with its soft shadows for a camera at rest and
/// with a single tap of the key's map for one moving.
pub(in super::super) struct Studio<T> {
    pub(in super::super) resting: T,
    pub(in super::super) moving: T,
}

impl<T> Studio<T> {
    pub(in super::super) fn of(&self, moving: bool) -> &T {
        match moving {
            true => &self.moving,
            false => &self.resting,
        }
    }
}

/// The models' plain pipelines: under the working view's light, and under the studio's.
pub(in super::super) struct Models {
    pub(in super::super) working: Shaded,
    pub(in super::super) studio: Studio<Shaded>,
}

impl Models {
    /// Under the studio when `staged`, its shadows hard while the camera is `moving`.
    pub(in super::super) fn under(&self, staged: bool, moving: bool) -> &Shaded {
        match staged {
            true => self.studio.of(moving),
            false => &self.working,
        }
    }
}

/// The values of the shader's `LOOK`.
const WORKING_LIGHT: f64 = 1.0;
pub(super) const STUDIO_LIGHT: f64 = 2.0;
pub(super) const STUDIO_MOVING: f64 = 3.0;

impl Builder<'_> {
    pub(super) fn models(&self) -> Models {
        let buffers = [Some(ModelVertex::layout()), Some(ModelInstance::layout())];
        let kind = |label| PipelineKind {
            label,
            vertex_entry: "model_vertex",
            fragment_entry: "model_fragment",
            buffers: &buffers,
            ..PipelineKind::default()
        };
        Models {
            working: self.shaded(&self.layout, kind("viewport_models"), WORKING_LIGHT),
            studio: Studio {
                resting: self.shaded(&self.layout, kind("viewport_studio_models"), STUDIO_LIGHT),
                moving: self.shaded(&self.layout, kind("viewport_moving_models"), STUDIO_MOVING),
            },
        }
    }

    /// `kind` built opaque and clipped, under the light `look` names.
    pub(super) fn shaded(
        &self,
        layout: &wgpu::PipelineLayout,
        kind: PipelineKind<'_>,
        look: f64,
    ) -> Shaded {
        let opaque = [("CLIPPED", 0.0), ("LOOK", look)];
        let clipped = [("CLIPPED", 1.0), ("LOOK", look)];
        Shaded {
            opaque: self.facing_on(
                layout,
                PipelineKind {
                    opaque: true,
                    constants: &opaque,
                    ..kind.clone()
                },
            ),
            clipped: self.facing_on(
                layout,
                PipelineKind {
                    constants: &clipped,
                    ..kind
                },
            ),
        }
    }
}
