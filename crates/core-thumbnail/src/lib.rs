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
pub use render::{Look, Part, Rendered, render, render_view};
pub use settings::{
    RESIN, RGB15_BITS, RGB565_BITS, Rgb, SUPPORT, ThumbnailSettings, rgb_of_565, rgb565, widen,
};
pub use view::View;
