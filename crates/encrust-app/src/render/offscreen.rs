//! Frames drawn offscreen and looked at, for what only the picture can answer: a cap
//! painted where the cutting plane misses the model (ADR 0074), and an exposure band
//! washed over the height it does not cover (ADR 0090).

use core_geometry::Scalar;

mod cavity;
mod holes;
mod marks;
mod paint;
mod plate;
mod texture;

const SIZE: u32 = 768;
const SECTION_MM: Scalar = 50.0;
