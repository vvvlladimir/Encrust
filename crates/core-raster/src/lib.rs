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
mod trace;

pub use error::RasterError;
pub use mask::LayerMask;
pub use preview::{crop, downsample, shrink_factor};
pub use rastered::Rastered;
pub use rasterizer::{Rasterizer, ScanlineRasterizer};
pub use runs::{LayerRuns, Run, RunsBuilder, lit_px};
pub use settings::{Grey, PixelPitch, RasterSettings, Shading};
pub use trace::{Ring, area_mm2, double_area_px, length_px, rings_of};
