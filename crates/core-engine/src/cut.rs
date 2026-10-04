use std::num::NonZeroU8;

use core_geometry::{Mesh, Scalar};
use core_slicer::{
    AdaptiveSettings, ONE_SAMPLE, SliceSettings, WINDOW_LAYERS, Windows, adaptive_plan,
};
use printer_profiles::Compensation;

use crate::error::EngineError;

/// How a stack is cut: one thickness throughout, or as thick as the surface allows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cutting {
    /// The thickest layer the stack may use, millimetres.
    pub layer_height_mm: Scalar,
    /// The rules an adaptive stack follows, or `None` for one thickness throughout.
    pub adaptive: Option<AdaptiveSettings>,
    /// How many planes are sampled inside each layer's band.
    pub samples: NonZeroU8,
    /// The resin's corrections, of which only the shrinkage changes what is cut.
    pub compensation: Compensation,
    /// Layers cut at once. Fewer holds less of the stack and rebuilds the face index
    /// more often, which trades memory for time.
    pub slice_window: usize,
}

impl Cutting {
    /// One thickness throughout, sampled once a layer, with the resin correcting nothing.
    pub fn uniform(layer_height_mm: Scalar) -> Self {
        Self {
            layer_height_mm,
            adaptive: None,
            samples: ONE_SAMPLE,
            compensation: Compensation::default(),
            slice_window: WINDOW_LAYERS,
        }
    }
}

/// The windows `mesh`, standing in plate coordinates, will be cut in.
pub fn cut(mesh: &Mesh, cutting: &Cutting) -> Result<Windows, EngineError> {
    let Some(adaptive) = cutting.adaptive else {
        let settings = SliceSettings {
            layer_height: cutting.layer_height_mm,
            samples: cutting.samples,
        };
        return Ok(Windows::new(mesh, settings, cutting.slice_window)?);
    };
    let plan = adaptive_plan(
        mesh,
        &AdaptiveSettings {
            max_height_mm: cutting.layer_height_mm,
            ..adaptive
        },
    )?;
    Ok(Windows::planned(
        plan,
        cutting.samples,
        cutting.slice_window,
    ))
}
