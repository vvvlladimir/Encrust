//! The shaded picture of a plate that a sliced file carries as its thumbnail. Rendered in software
//! so the CLI and window agree and no core crate knows a graphics API (ADR 0048).

mod image;
mod render;
mod settings;

pub use image::Thumbnail;
pub use render::{Look, Part, render};
pub use settings::{
    RESIN, RGB15_BITS, RGB565_BITS, Rgb, SUPPORT, ThumbnailSettings, rgb_of_565, rgb565, widen,
};
