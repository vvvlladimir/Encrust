//! Turning layer contours into the greyscale masks an MSLA display shows.

mod area;
mod blur;
mod error;
mod mask;
mod pixels;
mod preview;
mod rastered;
mod rasterizer;
mod runs;
mod scanline;
mod settings;

pub use error::RasterError;
pub use mask::LayerMask;
pub use preview::{crop, downsample, shrink_factor};
pub use rastered::Rastered;
pub use rasterizer::{Rasterizer, ScanlineRasterizer};
pub use runs::{LayerRuns, Run, RunsBuilder};
pub use settings::{Grey, PixelPitch, RasterSettings, Shading};
