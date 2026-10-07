//! Turning a mesh into a stack of closed 2D contours.

mod adaptive;
mod bins;
mod contour;
mod engine;
mod error;
mod layer;
mod offset;
mod plan;
mod plane;
mod settings;
mod sliced;
mod stitch;
mod windows;

pub use adaptive::{AdaptiveSettings, plan as adaptive_plan, plan_under as adaptive_plan_under};
pub use contour::{Contour, Winding};
pub use engine::{PlaneSliceEngine, SliceEngine, layer_heights, layer_heights_under};
pub use error::SliceError;
pub use layer::Layer;
pub use offset::offset_contours;
pub use plan::LayerPlan;
pub use settings::{ONE_SAMPLE, SliceSettings};
pub use sliced::Sliced;
pub use windows::{WINDOW_LAYERS, Windows};
