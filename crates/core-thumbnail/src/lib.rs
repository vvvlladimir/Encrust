//! The shaded picture of a plate that a sliced file carries as its thumbnail, and the same plate
//! from any side with points marked. Rendered in software so the CLI and window agree and no core
//! crate knows a graphics API (ADR 0048).

mod image;
mod marker;
mod render;
mod settings;
mod view;

pub use image::Thumbnail;
pub use marker::{Marker, MarkerSpot};
pub use render::{Part, Rendered, render, render_view};
pub use settings::{Rgb, ThumbnailSettings, rgb565};
pub use view::View;
